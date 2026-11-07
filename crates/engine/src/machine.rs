//! One emulated machine: a core plus everything needed to rebuild it exactly
//! (ROM, boot ROM, seed, config, patches) and to check a checkpoint against
//! it (symbols, breakpoints). Restoring and cloning rebuild the core from the
//! unpatched ROM, so a machine always holds exactly its recorded patches.

use std::fmt;
use std::sync::Arc;

use gbcli_core::{BootRom, Core, CoreConfig, CoreError, Keys, Model, Region};
use gbcli_protocol::{
    CheckpointHeader, CoreId, DebuggerConfig, Patch, RenderConfig, Sha256, TimeMark,
};

use crate::checkpoint::{Checkpoint, LoadOptions, SaveMeta};
use crate::checkpoint_error::{CheckpointError, LoadWarning};
use crate::romcheck;
use crate::sha256::sha256;
use crate::sym::{SymError, Symbols};

/// How to build cores of one backend.
#[derive(Clone, Copy)]
pub struct Backend {
    build: fn(CoreConfig) -> Result<Box<dyn Core>, CoreError>,
    builtin_boot_rom: fn(Model) -> &'static [u8],
}

impl Backend {
    pub fn new(
        build: fn(CoreConfig) -> Result<Box<dyn Core>, CoreError>,
        builtin_boot_rom: fn(Model) -> &'static [u8],
    ) -> Backend {
        Backend {
            build,
            builtin_boot_rom,
        }
    }

    /// The SameBoy backend.
    pub fn sameboy() -> Backend {
        Backend::new(
            |config| Ok(Box::new(gbcli_sameboy::SameBoy::new(config)?)),
            gbcli_sameboy::builtin_boot_rom,
        )
    }
}

impl fmt::Debug for Backend {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("Backend")
    }
}

/// Why a machine couldn't be built.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum SetupError {
    Core(CoreError),
    Symbols(SymError),
}

impl fmt::Display for SetupError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            SetupError::Core(e) => write!(f, "{e}"),
            SetupError::Symbols(e) => write!(f, "symbols unreadable: {e}"),
        }
    }
}

impl std::error::Error for SetupError {}

/// A loaded `.sym` and the hash of its text.
#[derive(Clone, Debug)]
struct LoadedSymbols {
    symbols: Arc<Symbols>,
    sha256: Sha256,
}

/// The settings a core is rebuilt from, besides the ROM.
#[derive(Clone, Debug)]
struct Setup {
    model: Model,
    seed: u64,
    config: RenderConfig,
    boot_rom: Arc<[u8]>,
    boot_sha256: Sha256,
}

pub struct Machine {
    backend: Backend,
    core: Box<dyn Core>,
    setup: Setup,
    /// The unpatched ROM file.
    rom: Arc<[u8]>,
    rom_sha256: Sha256,
    symbols: Option<LoadedSymbols>,
    patches: Vec<Patch>,
    keys: Keys,
    pub(crate) boot: Option<TimeMark>,
    debugger: DebuggerConfig,
}

impl Machine {
    /// Builds a machine at power-on. `sym` is the ROM's `.sym` text, if any.
    pub fn new(
        backend: Backend,
        config: CoreConfig,
        sym: Option<&str>,
    ) -> Result<Machine, SetupError> {
        let symbols = sym
            .map(|text| {
                Symbols::parse(text).map(|s| LoadedSymbols {
                    symbols: Arc::new(s),
                    sha256: sha256(text.as_bytes()),
                })
            })
            .transpose()
            .map_err(SetupError::Symbols)?;
        let boot_rom: Arc<[u8]> = match &config.boot_rom {
            BootRom::BuiltIn => (backend.builtin_boot_rom)(config.model).into(),
            BootRom::Custom(bytes) => bytes.as_slice().into(),
        };
        let setup = Setup {
            model: config.model,
            seed: config.seed,
            config: RenderConfig {
                rtc: config.rtc,
                rendering: config.rendering,
                joypad_bounce: config.joypad_bounce,
                write_tracking: config.write_tracking,
            },
            boot_sha256: sha256(&boot_rom),
            boot_rom,
        };
        let rom: Arc<[u8]> = config.rom.as_slice().into();
        let core = (backend.build)(core_config(&setup, &rom)).map_err(SetupError::Core)?;
        Ok(Machine {
            backend,
            core,
            rom_sha256: sha256(&rom),
            rom,
            setup,
            symbols,
            patches: Vec::new(),
            keys: Keys::NONE,
            boot: None,
            debugger: DebuggerConfig::default(),
        })
    }

    pub fn core(&self) -> &dyn Core {
        self.core.as_ref()
    }

    /// The core itself. Patches and keys set here bypass the machine's
    /// records; use [`Machine::patch`] and [`Machine::set_keys`].
    pub fn core_mut(&mut self) -> &mut dyn Core {
        self.core.as_mut()
    }

    pub fn symbols(&self) -> Option<&Symbols> {
        self.symbols.as_ref().map(|s| s.symbols.as_ref())
    }

    /// The ROM patches applied since power-on or the last restore, in order.
    pub fn patches(&self) -> &[Patch] {
        &self.patches
    }

    /// Patches the ROM at a file offset and records it, with the bytes it replaced.
    pub fn patch(&mut self, offset: u32, bytes: &[u8]) -> Result<(), CoreError> {
        let before = self.rom_bytes(offset, bytes.len())?;
        self.core.patch_rom(offset, bytes)?;
        self.patches.push(Patch {
            offset,
            bytes: bytes.to_vec(),
            before,
        });
        Ok(())
    }

    pub fn set_keys(&mut self, keys: Keys) {
        self.keys = keys;
        self.core.set_keys(keys);
    }

    pub fn keys(&self) -> Keys {
        self.keys
    }

    /// The boot handoff, once it has happened.
    pub fn boot(&self) -> Option<TimeMark> {
        self.boot
    }

    /// The moment `frame` and `t` in expressions and stop records count from
    /// (design §9). `Boot` before the handoff falls back to power-on.
    pub fn time_base(&self, since: &crate::debug::Since) -> TimeMark {
        match since {
            crate::debug::Since::PowerOn => TimeMark { ticks: 0, frame: 0 },
            crate::debug::Since::Boot => self.boot.unwrap_or(TimeMark { ticks: 0, frame: 0 }),
            crate::debug::Since::Checkpoint { mark, .. } => *mark,
        }
    }

    pub fn debugger_config(&self) -> &DebuggerConfig {
        &self.debugger
    }

    pub fn debugger_config_mut(&mut self) -> &mut DebuggerConfig {
        &mut self.debugger
    }

    /// The current state as a checkpoint. It takes `&mut self` because the
    /// core's save does (see [`Core::save`]).
    pub fn capture(&mut self, meta: SaveMeta) -> Checkpoint {
        Checkpoint {
            header: self.header(meta),
            blob: self.core.save(),
        }
    }

    /// The header [`Machine::capture`] would write, without saving the core.
    pub fn header(&self, meta: SaveMeta) -> CheckpointHeader {
        let info = self.core.info();
        CheckpointHeader {
            core: CoreId {
                backend: info.backend.into(),
                commit: info.version.into(),
            },
            model: self.setup.model,
            rom_sha256: self.rom_sha256,
            sym_sha256: self.symbols.as_ref().map(|s| s.sha256),
            boot_rom_sha256: self.setup.boot_sha256,
            seed: self.setup.seed,
            config: self.setup.config,
            ticks: self.core.now().0,
            frame: self.core.last_complete_frame().map_or(0, |f| f.tag.number),
            boot: self.boot,
            keys: self.keys.0,
            patches: self.patches.clone(),
            symbols_live: self
                .symbols()
                .map(|s| romcheck::live_symbols(self.core(), s))
                .unwrap_or_default(),
            debugger: self.debugger.clone(),
            created_by: meta.created_by,
            parent: meta.parent,
            rev: meta.rev,
        }
    }

    /// Restores a checkpoint, verifying it first (design §6, steps 2–7; step 1
    /// is [`Checkpoint::decode`]). On any refusal the machine is unchanged.
    pub fn restore(
        &mut self,
        cp: &Checkpoint,
        opts: &LoadOptions,
    ) -> Result<Vec<LoadWarning>, CheckpointError> {
        let h = &cp.header;
        let mut warnings = Vec::new();
        let info = self.core.info();
        let ours = CoreId {
            backend: info.backend.into(),
            commit: info.version.into(),
        };
        // 2. Core.
        if h.core.backend != ours.backend {
            return Err(CheckpointError::OtherBackend {
                saved: h.core.backend.clone(),
                ours: ours.backend,
            });
        }
        // 3. Model: the core is rebuilt from the envelope's.
        if let Some(requested) = opts.model
            && requested != h.model
        {
            return Err(CheckpointError::ModelConflict {
                saved: h.model,
                requested,
            });
        }
        // 4. Boot ROM.
        let boot_rom = self.find_boot_rom(h, opts)?;
        // 5. ROM.
        let rom_changed = h.rom_sha256 != self.rom_sha256;
        if rom_changed {
            match romcheck::check(h, self.symbols(), &self.debugger.breakpoints) {
                Ok(check) => warnings.push(LoadWarning::RomChanged {
                    moved: Vec::new(),
                    checked: check.checked,
                    forced: false,
                }),
                Err(err) if opts.allow_rom_change => {
                    let moved = match err {
                        CheckpointError::RomChanged { moved } => moved,
                        _ => Vec::new(),
                    };
                    warnings.push(LoadWarning::RomChanged {
                        checked: h.symbols_live.len(),
                        moved,
                        forced: true,
                    });
                }
                Err(err) => return Err(err),
            }
        }
        // 6. Patches, onto a core rebuilt from the unpatched ROM.
        let setup = Setup {
            model: h.model,
            seed: h.seed,
            config: h.config,
            boot_sha256: h.boot_rom_sha256,
            boot_rom,
        };
        let rejected = |reason: String| CheckpointError::CoreRejected {
            saved: h.core.clone(),
            ours: ours.clone(),
            reason,
        };
        let mut core = (self.backend.build)(core_config(&setup, &self.rom))
            .map_err(|e| rejected(e.to_string()))?;
        for patch in &h.patches {
            let found = read_rom(core.as_ref(), patch.offset, patch.before.len());
            if found.as_ref() != Ok(&patch.before) {
                if !(rom_changed && opts.allow_rom_change) {
                    return Err(CheckpointError::PatchConflict {
                        offset: patch.offset,
                        expected: patch.before.clone(),
                        found: found.unwrap_or_default(),
                    });
                }
                warnings.push(LoadWarning::PatchOverChangedBytes {
                    offset: patch.offset,
                });
            }
            core.patch_rom(patch.offset, &patch.bytes).map_err(|e| {
                CheckpointError::PatchRefused {
                    offset: patch.offset,
                    reason: e.to_string(),
                }
            })?;
        }
        // 7. The rest: blob (counters, time, written bitmaps), keys, time bases.
        core.load(&cp.blob).map_err(|e| rejected(e.to_string()))?;
        core.set_keys(Keys(h.keys));
        if h.core.commit != ours.commit {
            warnings.push(LoadWarning::OtherCommit {
                saved: h.core.commit.clone(),
                ours: ours.commit,
            });
        }
        self.core = core;
        self.setup = setup;
        self.patches = h.patches.clone();
        self.keys = Keys(h.keys);
        self.boot = h.boot;
        Ok(warnings)
    }

    /// An independent copy in the same state, for A/B experiments. Copying
    /// saves this machine, hence `&mut self` (see `Core::save`).
    pub fn try_clone(&mut self) -> Result<Machine, CoreError> {
        let mut core = (self.backend.build)(core_config(&self.setup, &self.rom))?;
        for patch in &self.patches {
            core.patch_rom(patch.offset, &patch.bytes)?;
        }
        core.load(&self.core.save())?;
        core.set_keys(self.keys);
        Ok(Machine {
            backend: self.backend,
            core,
            setup: self.setup.clone(),
            rom: self.rom.clone(),
            rom_sha256: self.rom_sha256,
            symbols: self.symbols.clone(),
            patches: self.patches.clone(),
            keys: self.keys,
            boot: self.boot,
            debugger: self.debugger.clone(),
        })
    }

    fn find_boot_rom(
        &self,
        h: &CheckpointHeader,
        opts: &LoadOptions,
    ) -> Result<Arc<[u8]>, CheckpointError> {
        if h.boot_rom_sha256 == self.setup.boot_sha256 {
            return Ok(self.setup.boot_rom.clone());
        }
        let builtin = (self.backend.builtin_boot_rom)(h.model);
        std::iter::once(builtin)
            .chain(opts.boot_roms.iter().map(Vec::as_slice))
            .find(|bytes| sha256(bytes) == h.boot_rom_sha256)
            .map(Arc::from)
            .ok_or(CheckpointError::BootRomMissing {
                sha256: h.boot_rom_sha256,
            })
    }

    fn rom_bytes(&self, offset: u32, len: usize) -> Result<Vec<u8>, CoreError> {
        read_rom(self.core.as_ref(), offset, len)
    }
}

fn read_rom(core: &dyn Core, offset: u32, len: usize) -> Result<Vec<u8>, CoreError> {
    let rom = core.region(Region::Rom)?.bytes;
    let start = offset as usize;
    rom.get(start..start + len)
        .map(<[u8]>::to_vec)
        .ok_or(CoreError::OutOfRange {
            what: "ROM offset",
            value: u64::from(offset) + len as u64,
            limit: rom.len() as u64,
        })
}

fn core_config(setup: &Setup, rom: &[u8]) -> CoreConfig {
    CoreConfig {
        model: setup.model,
        rom: rom.to_vec(),
        boot_rom: BootRom::Custom(setup.boot_rom.to_vec()),
        seed: setup.seed,
        rtc: setup.config.rtc,
        rendering: setup.config.rendering,
        joypad_bounce: setup.config.joypad_bounce,
        write_tracking: setup.config.write_tracking,
    }
}
