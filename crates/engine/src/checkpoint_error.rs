//! Why a checkpoint didn't load, what to do about it, and what loaded with a
//! warning. Each refusal names its protocol error code (design §5) and a remedy.

use std::fmt;

use gbcli_core::Model;
use gbcli_protocol::{CoreId, Sha256, model_name};

/// A bank and address.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SymbolAt {
    pub bank: u16,
    pub addr: u16,
}

impl fmt::Display for SymbolAt {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{:02X}:{:04X}", self.bank, self.addr)
    }
}

/// Why a moved symbol matters to the saved state.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MovedRole {
    /// The ROM symbol containing PC.
    Pc,
    /// A WRAM, HRAM or cart RAM symbol.
    Ram,
    /// The ROM symbol containing a possible return address on the stack.
    Stack,
    /// A breakpoint's symbol.
    Breakpoint,
}

/// A symbol whose address differs between the saved and the current `.sym`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Moved {
    pub name: String,
    pub role: MovedRole,
    pub old: SymbolAt,
    /// `None` when the symbol is gone.
    pub new: Option<SymbolAt>,
}

impl fmt::Display for Moved {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let role = match self.role {
            MovedRole::Pc => "PC is in",
            MovedRole::Ram => "RAM symbol",
            MovedRole::Stack => "return address on the stack is in",
            MovedRole::Breakpoint => "breakpoint on",
        };
        match self.new {
            Some(new) => write!(f, "{role} {}: {} -> {new}", self.name, self.old),
            None => write!(f, "{role} {}: {} -> gone", self.name, self.old),
        }
    }
}

/// Which side of a ROM change lacks symbols.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SymbolSide {
    Saved,
    Current,
    Both,
}

/// A refused load. The first failing verification step stops the load
/// before the core is touched.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum CheckpointError {
    /// Step 1: not an envelope, or an unknown version. `checkpoint_unreadable`.
    Unreadable { reason: String },
    /// Step 2: written by another backend. `checkpoint_mismatch`.
    OtherBackend { saved: String, ours: String },
    /// Step 2 or 7: the core refused the blob. `checkpoint_mismatch`.
    CoreRejected {
        saved: CoreId,
        ours: CoreId,
        reason: String,
    },
    /// Step 3: the caller asked for another model. `bad_request`.
    ModelConflict { saved: Model, requested: Model },
    /// Step 4: no boot ROM we have matches. `boot_rom_missing`.
    BootRomMissing { sha256: Sha256 },
    /// Step 5: the ROM changed and there are no symbols to check it by. `rom_changed`.
    SymbolsMissing { side: SymbolSide },
    /// Step 5: the ROM changed under something the state depends on. `rom_changed`.
    RomChanged { moved: Vec<Moved> },
    /// Step 6: a patch's replaced bytes differ in the current ROM. `rom_changed`.
    PatchConflict {
        offset: u32,
        expected: Vec<u8>,
        found: Vec<u8>,
    },
    /// Step 6: the core refused a patch. `checkpoint_mismatch`.
    PatchRefused { offset: u32, reason: String },
    /// No checkpoint by that name. `bad_request`.
    Unknown { name: String, known: Vec<String> },
}

impl CheckpointError {
    /// The next thing to do, with real values filled in.
    pub fn remedy(&self) -> String {
        match self {
            CheckpointError::Unreadable { .. } => {
                "the file isn't a checkpoint this gbcli can read; save a new one with \
                 `gbcli checkpoint <name>`"
                    .into()
            }
            CheckpointError::OtherBackend { saved, ours } => format!(
                "this checkpoint is from the {saved} backend and this gbcli runs {ours}; \
                 load it with a gbcli built on {saved}"
            ),
            CheckpointError::CoreRejected { saved, ours, .. } => format!(
                "the blob was written by {} {} and {} {} can't read it; load it with the \
                 gbcli that wrote it, or re-create the state from the ROM",
                saved.backend, saved.commit, ours.backend, ours.commit
            ),
            CheckpointError::ModelConflict { saved, .. } => format!(
                "a checkpoint keeps its model: drop --model, or pass --model {}",
                model_name(*saved)
            ),
            CheckpointError::BootRomMissing { sha256 } => {
                format!("pass --boot-rom <file> with the boot ROM whose SHA-256 is {sha256}")
            }
            CheckpointError::SymbolsMissing { .. } => {
                "the ROM changed and can't be checked without symbols: rebuild with \
                 `rgblink -n game.sym`, or pass --allow-rom-change"
                    .into()
            }
            CheckpointError::RomChanged { .. } => {
                "re-create the state on the new ROM (run from boot or an older checkpoint), \
                 or pass --allow-rom-change to load it anyway"
                    .into()
            }
            CheckpointError::PatchConflict { offset, .. } => format!(
                "the ROM bytes under the patch at rom:{offset:#x} changed; pass \
                 --allow-rom-change to apply it anyway, or re-apply it with --patch"
            ),
            CheckpointError::PatchRefused { offset, .. } => format!(
                "the core refused the patch at rom:{offset:#x}; restore an older checkpoint"
            ),
            CheckpointError::Unknown { known, .. } if known.is_empty() => {
                "no checkpoints yet; save one with `gbcli checkpoint <name>`".into()
            }
            CheckpointError::Unknown { known, .. } => {
                format!("known checkpoints: {}", known.join(", "))
            }
        }
    }
}

impl fmt::Display for CheckpointError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            CheckpointError::Unreadable { reason } => write!(f, "checkpoint unreadable: {reason}"),
            CheckpointError::OtherBackend { saved, ours } => {
                write!(f, "checkpoint is from backend {saved}, not {ours}")
            }
            CheckpointError::CoreRejected {
                saved,
                ours,
                reason,
            } => write!(
                f,
                "{} {} refused a blob from {} {}: {reason}",
                ours.backend, ours.commit, saved.backend, saved.commit
            ),
            CheckpointError::ModelConflict { saved, requested } => write!(
                f,
                "checkpoint is model {}, but {} was asked for",
                model_name(*saved),
                model_name(*requested)
            ),
            CheckpointError::BootRomMissing { sha256 } => {
                write!(f, "no boot ROM with SHA-256 {sha256}")
            }
            CheckpointError::SymbolsMissing { side } => {
                let which = match side {
                    SymbolSide::Saved => "the checkpoint was saved without a .sym",
                    SymbolSide::Current => "no .sym is loaded for the current ROM",
                    SymbolSide::Both => "neither side has a .sym",
                };
                write!(f, "the ROM changed since the checkpoint, and {which}")
            }
            CheckpointError::RomChanged { moved } => {
                write!(f, "the ROM changed under the saved state: ")?;
                for (i, m) in moved.iter().enumerate() {
                    if i > 0 {
                        write!(f, "; ")?;
                    }
                    write!(f, "{m}")?;
                }
                Ok(())
            }
            CheckpointError::PatchConflict {
                offset,
                expected,
                found,
            } => write!(
                f,
                "patch at rom:{offset:#x} replaced {expected:02X?}, but the ROM has {found:02X?}"
            ),
            CheckpointError::PatchRefused { offset, reason } => {
                write!(f, "patch at rom:{offset:#x} refused: {reason}")
            }
            CheckpointError::Unknown { name, .. } => write!(f, "no checkpoint named {name:?}"),
        }
    }
}

impl std::error::Error for CheckpointError {}

/// Something a successful load wants the caller to know.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum LoadWarning {
    /// Another commit of the same backend wrote the blob, and the core accepted it.
    OtherCommit { saved: String, ours: String },
    /// The ROM changed since the checkpoint. `moved` lists the live symbols
    /// that moved: empty when the check passed, and possibly not when
    /// `--allow-rom-change` forced the load (`forced`). `checked` counts the
    /// symbols compared; zero when there were no symbols to compare.
    RomChanged {
        moved: Vec<Moved>,
        checked: usize,
        forced: bool,
    },
    /// A forced load applied a patch over bytes that differ from the ones it replaced.
    PatchOverChangedBytes { offset: u32 },
}
