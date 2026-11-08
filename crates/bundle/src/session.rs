//! Making a session from a ROM and building its machine (design §1). Naming a
//! session creates it: [`Bundle::open_session`] holds that decision so every
//! client applies the same rule.

use std::fs;
use std::path::{Path, PathBuf};

use gbcli_core::flash::FlashProfile;
use gbcli_core::{BootRom, CoreConfig, Model};
use gbcli_engine::{Backend, Machine, SetupError};
use gbcli_protocol::{RenderConfig, Rev, SessionFile, model_name};

use crate::bundle::Bundle;
use crate::error::{Setting, StoreError};
use crate::io::ensure_dir;
use crate::lock::LOCK_WAIT;
use crate::store::SessionStore;

/// How to build a new session. Relative paths are relative to the project
/// directory ([`Bundle::project`]); a CLI makes its arguments absolute first.
#[derive(Clone, Debug)]
pub struct RomSource {
    pub rom: PathBuf,
    /// The `.sym`; by default the ROM's basename with `.sym` beside it.
    pub sym: Option<PathBuf>,
    /// `None`: [`DEFAULT_MODEL`] for a new session, and whatever an existing
    /// one has. A value an existing session doesn't have is refused.
    pub model: Option<Model>,
    /// `None`: 0 for a new session, and whatever an existing one has.
    pub seed: Option<u64>,
    /// The cart's flash board, by profile name (`--flash`). `None`: mask ROM
    /// for a new session, and whatever an existing one has.
    pub flash: Option<String>,
}

/// The model a session is created with when none is given.
pub const DEFAULT_MODEL: Model = Model::DmgB;

impl Bundle {
    /// Opens a session by name, creating it from `rom` when it doesn't exist.
    /// Without `rom` a missing session is `no_session`. With `rom`, an existing
    /// session over another ROM file is refused: `rom` would be ignored.
    pub fn open_session(
        &self,
        name: &str,
        rom: Option<&RomSource>,
    ) -> Result<SessionStore, StoreError> {
        let store = self.session(name)?;
        let Some(source) = rom else {
            return if store.exists() {
                Ok(store)
            } else {
                Err(store.no_session())
            };
        };
        if !store.exists() {
            match store.create(source) {
                Ok(_) => return Ok(store),
                // Another process created it first: check its ROM as below.
                Err(StoreError::SessionExists { .. }) => {}
                Err(e) => return Err(e),
            }
        }
        let meta = store.read_meta()?;
        let given = store.resolve(&source.rom);
        if !same_file(&store.resolve(Path::new(&meta.rom)), &given) {
            return Err(StoreError::RomMismatch {
                name: name.to_string(),
                stored: meta.rom,
                given,
            });
        }
        check_settings(&store, &meta, source)?;
        Ok(store)
    }
}

/// Refuses a model, seed or `.sym` given for an existing session that has
/// another: it would otherwise be silently ignored.
fn check_settings(
    store: &SessionStore,
    meta: &SessionFile,
    source: &RomSource,
) -> Result<(), StoreError> {
    let differs = |setting: Setting, session: String, given: String| StoreError::SettingDiffers {
        name: store.name().to_string(),
        setting,
        session,
        given,
    };
    if let Some(model) = source.model.filter(|m| *m != meta.model) {
        return Err(differs(
            Setting::Model,
            model_name(meta.model).into(),
            model_name(model).into(),
        ));
    }
    if let Some(seed) = source.seed.filter(|s| *s != meta.seed) {
        return Err(differs(
            Setting::Seed,
            meta.seed.to_string(),
            seed.to_string(),
        ));
    }
    if let Some(sym) = &source.sym {
        let given = store.resolve(sym);
        let same = meta
            .sym
            .as_ref()
            .is_some_and(|s| same_file(&store.resolve(Path::new(s)), &given));
        if !same {
            let session = meta.sym.clone().unwrap_or_else(|| "no .sym".into());
            return Err(differs(Setting::Sym, session, given.display().to_string()));
        }
    }
    if let Some(board) = source
        .flash
        .as_ref()
        .filter(|b| meta.flash.as_ref() != Some(*b))
    {
        let session = meta
            .flash
            .clone()
            .unwrap_or_else(|| "none (mask ROM)".into());
        return Err(differs(Setting::Flash, session, board.clone()));
    }
    Ok(())
}

/// A value no earlier creation of a session by this name holds: host time in
/// nanoseconds, mixed with the process id for two creations in one tick.
/// Host time is fine here; it never reaches emulation.
pub(crate) fn new_incarnation() -> u64 {
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| d.as_nanos() as u64);
    nanos ^ (u64::from(std::process::id()) << 48)
}

impl SessionStore {
    /// Creates the session from a ROM. Refuses an existing name. The ROM and
    /// symbols are read and the machine built before anything is written.
    pub fn create(&self, source: &RomSource) -> Result<Machine, StoreError> {
        if self.exists() {
            return Err(StoreError::SessionExists {
                name: self.name().to_string(),
            });
        }
        let rom_path = self.resolve(&source.rom);
        let rom = fs::read(&rom_path).map_err(|e| StoreError::RomUnreadable {
            path: rom_path.clone(),
            reason: e.to_string(),
        })?;
        let sym_path = source.sym.as_deref().map(|p| self.resolve(p)).or_else(|| {
            let sibling = rom_path.with_extension("sym");
            sibling.is_file().then_some(sibling)
        });
        let sym = sym_path.as_deref().map(read_symbols).transpose()?;
        let config = default_config();
        let meta = SessionFile {
            name: self.name().to_string(),
            rom: self.stored(&rom_path),
            sym: sym_path.as_deref().map(|p| self.stored(p)),
            model: source.model.unwrap_or(DEFAULT_MODEL),
            seed: source.seed.unwrap_or(0),
            flash: source.flash.clone(),
            rev: Rev(0),
            config,
            debugger: Default::default(),
            last_stop: None,
            incarnation: new_incarnation(),
            undo: None,
        };
        let machine = build(&meta, rom, sym.as_deref(), sym_path.as_deref())?;

        let lock = self.acquire(LOCK_WAIT)?;
        if self.exists() {
            return Err(StoreError::SessionExists {
                name: self.name().to_string(),
            });
        }
        let dir = self.dir();
        if dir.is_dir() {
            // Not a session: a create killed before its session.json, or the
            // lock-only directory older builds left for a mistyped name.
            let checkpoints = self.checkpoint_names()?;
            if !checkpoints.is_empty() {
                return Err(StoreError::CheckpointsPresent {
                    session: self.name().to_string(),
                    checkpoints,
                });
            }
            self.remove_locked()?;
        }
        ensure_dir(&self.bundle().sessions_dir())?;
        ensure_dir(&dir)?;
        ensure_dir(&self.history_dir())?;
        ensure_dir(&self.checkpoints_dir())?;
        // session.json last: until it is written, this is not a session.
        self.write_meta(&meta)?;
        lock.record_use(&self.lock_path())?;
        Ok(machine)
    }

    /// Builds a machine from the session's metadata at power-on.
    pub fn machine(&self, meta: &SessionFile) -> Result<Machine, StoreError> {
        let rom_path = self.resolve(Path::new(&meta.rom));
        let rom = fs::read(&rom_path).map_err(|e| StoreError::RomUnreadable {
            path: rom_path,
            reason: e.to_string(),
        })?;
        let sym_path = meta.sym.as_deref().map(|p| self.resolve(Path::new(p)));
        let sym = sym_path.as_deref().map(read_symbols).transpose()?;
        build(meta, rom, sym.as_deref(), sym_path.as_deref())
    }
}

fn build(
    meta: &SessionFile,
    rom: Vec<u8>,
    sym: Option<&str>,
    sym_path: Option<&Path>,
) -> Result<Machine, StoreError> {
    let core = CoreConfig {
        model: meta.model,
        rom,
        boot_rom: BootRom::BuiltIn,
        seed: meta.seed,
        rtc: meta.config.rtc,
        rendering: meta.config.rendering,
        joypad_bounce: meta.config.joypad_bounce,
        write_tracking: meta.config.write_tracking,
        flash: meta.flash.as_deref().map(flash_board).transpose()?,
    };
    Machine::new(Backend::sameboy(), core, sym).map_err(|e| match e {
        SetupError::Symbols(_) => StoreError::SymbolsUnreadable {
            path: sym_path.map(Path::to_path_buf).unwrap_or_default(),
            reason: e.to_string(),
        },
        SetupError::Core(_) => StoreError::Setup(e),
    })
}

/// The named flash board, or a refusal listing the names there are.
fn flash_board(name: &str) -> Result<FlashProfile, StoreError> {
    FlashProfile::named(name).ok_or_else(|| StoreError::UnknownFlashBoard {
        name: name.to_string(),
        known: FlashProfile::names().map(str::to_string).collect(),
    })
}

/// Whether two paths name one file. Paths that don't resolve compare as text.
fn same_file(a: &Path, b: &Path) -> bool {
    match (fs::canonicalize(a), fs::canonicalize(b)) {
        (Ok(a), Ok(b)) => a == b,
        _ => a == b,
    }
}

/// Reads a `.sym`, naming the path it read when it can't.
fn read_symbols(path: &Path) -> Result<String, StoreError> {
    fs::read_to_string(path).map_err(|e| StoreError::SymbolsUnreadable {
        path: path.to_path_buf(),
        reason: e.to_string(),
    })
}

fn default_config() -> RenderConfig {
    RenderConfig {
        rtc: gbcli_core::RtcMode::Accurate,
        rendering: gbcli_core::Rendering::On,
        joypad_bounce: gbcli_core::JoypadBounce::Off,
        write_tracking: gbcli_core::WriteTracking::On,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_fixture::{Fixture, SYM};

    #[test]
    fn creating_a_session_writes_metadata_and_holds_no_head() {
        let f = Fixture::new("create");
        let store = f.demo();
        assert!(store.exists());
        let meta = store.read_meta().unwrap();
        assert_eq!(meta.name, "demo");
        assert_eq!(meta.rev, Rev(0));
        assert_eq!(meta.rom, "demo.gb");
        assert!(store.read_head(meta.rev).unwrap().is_none());
        assert_eq!(f.bundle.session_names().unwrap(), vec!["demo"]);
    }

    #[test]
    fn a_flash_session_records_its_board_and_its_machine_is_on_it() {
        let f = Fixture::new("flash-create");
        let store = f.bundle.session("flashy").unwrap();
        let mut source = f.source();
        source.flash = Some("generic-wr-aaa-a9".into());
        let machine = store.create(&source).unwrap();
        assert_eq!(machine.flash_board(), Some("generic-wr-aaa-a9"));
        let meta = store.read_meta().unwrap();
        assert_eq!(meta.flash.as_deref(), Some("generic-wr-aaa-a9"));
        assert_eq!(
            store.machine(&meta).unwrap().flash_board(),
            Some("generic-wr-aaa-a9")
        );
        let mask = f.demo();
        let meta = mask.read_meta().unwrap();
        assert_eq!(mask.machine(&meta).unwrap().flash_board(), None);
    }

    #[test]
    fn an_unknown_flash_board_is_refused_naming_the_boards() {
        let f = Fixture::new("flash-unknown");
        let store = f.bundle.session("flashy").unwrap();
        let mut source = f.source();
        source.flash = Some("generic-wr-aaa-a8".into());
        let Err(err) = store.create(&source) else {
            panic!("an unknown board was accepted");
        };
        assert!(
            matches!(err, StoreError::UnknownFlashBoard { .. }),
            "{err:?}"
        );
        assert_eq!(err.code(), gbcli_protocol::ErrorCode::BadRequest);
        assert!(
            err.remedy().contains("generic-wr-aaa-a9"),
            "{}",
            err.remedy()
        );
        assert!(!store.dir().exists());
    }

    #[test]
    fn reopening_with_another_flash_board_is_refused() {
        let f = Fixture::new("flash-differs");
        f.demo();
        let mut source = f.source();
        source.flash = Some("generic-wr-aaa-a9".into());
        let err = f.bundle.open_session("demo", Some(&source)).unwrap_err();
        assert!(
            matches!(
                err,
                StoreError::SettingDiffers {
                    setting: Setting::Flash,
                    ..
                }
            ),
            "{err:?}"
        );
        assert!(err.to_string().contains("--flash"), "{err}");
    }

    #[test]
    fn a_missing_rom_is_refused_before_anything_is_written() {
        let f = Fixture::new("missing-rom");
        let store = f.bundle.session("gone").unwrap();
        let mut source = f.source();
        source.rom = f.root.join("nope.gb");
        assert!(matches!(
            store.create(&source),
            Err(StoreError::RomUnreadable { .. })
        ));
        assert!(!store.dir().exists());
    }

    #[test]
    fn relative_rom_and_sym_paths_both_resolve_against_the_project() {
        let f = Fixture::new("relative-paths");
        fs::create_dir_all(f.root.join("build")).unwrap();
        fs::write(
            f.root.join("build").join("game.gb"),
            crate::test_fixture::ROM,
        )
        .unwrap();
        fs::write(f.root.join("build").join("labels.sym"), SYM).unwrap();
        let store = f.bundle.session("demo").unwrap();
        store
            .create(&RomSource {
                rom: PathBuf::from("build/game.gb"),
                sym: Some(PathBuf::from("build/labels.sym")),
                model: Some(Model::DmgB),
                seed: Some(0),
                flash: None,
            })
            .unwrap();
        let meta = store.read_meta().unwrap();
        assert_eq!(meta.rom, "build/game.gb");
        assert_eq!(meta.sym.as_deref(), Some("build/labels.sym"));
    }

    #[test]
    fn an_unreadable_sym_names_the_path_it_tried() {
        let f = Fixture::new("sym-path");
        let store = f.bundle.session("demo").unwrap();
        let mut source = f.source();
        source.sym = Some(PathBuf::from("missing.sym"));
        let Err(err) = store.create(&source) else {
            panic!("a missing .sym was accepted");
        };
        let StoreError::SymbolsUnreadable { path, .. } = err else {
            panic!("{err:?}");
        };
        assert_eq!(path, f.bundle.project().join("missing.sym"));
    }

    #[test]
    fn creating_over_a_directory_with_no_session_file_works() {
        let f = Fixture::new("create-over-broken");
        let store = f.bundle.session("demo").unwrap();
        fs::create_dir_all(store.dir()).unwrap();
        fs::write(store.dir().join("lock"), b"").unwrap();
        store.create(&f.source()).unwrap();
        assert!(store.read_meta().is_ok());
    }

    #[test]
    fn opening_a_missing_session_without_a_rom_is_no_session_and_writes_nothing() {
        let f = Fixture::new("open-missing");
        let err = f.bundle.open_session("typo", None).unwrap_err();
        assert!(matches!(err, StoreError::NoSession { .. }), "{err:?}");
        assert!(!f.bundle.sessions_dir().join("typo").exists());
    }

    #[test]
    fn opening_a_missing_session_with_a_rom_creates_it_and_reopening_finds_it() {
        let f = Fixture::new("open-create");
        let source = f.source();
        let store = f.bundle.open_session("demo", Some(&source)).unwrap();
        assert_eq!(store.read_meta().unwrap().rom, "demo.gb");
        let again = f.bundle.open_session("demo", Some(&source)).unwrap();
        assert_eq!(again.name(), "demo");
        assert!(f.bundle.open_session("demo", None).is_ok());
    }

    #[test]
    fn opening_an_existing_session_with_another_rom_is_refused_with_a_remedy() {
        let f = Fixture::new("open-mismatch");
        f.demo();
        fs::write(f.root.join("other.gb"), crate::test_fixture::ROM).unwrap();
        let mut source = f.source();
        source.rom = f.root.join("other.gb");
        let err = f.bundle.open_session("demo", Some(&source)).unwrap_err();
        let StoreError::RomMismatch { stored, .. } = &err else {
            panic!("{err:?}");
        };
        assert_eq!(stored, "demo.gb");
        assert!(
            err.remedy().contains("sessions rm demo"),
            "{}",
            err.remedy()
        );
    }

    #[test]
    fn the_same_rom_named_another_way_is_not_a_mismatch() {
        let f = Fixture::new("open-same");
        f.demo();
        let mut source = f.source();
        source.rom = PathBuf::from("./demo.gb");
        assert!(f.bundle.open_session("demo", Some(&source)).is_ok());
    }
}
