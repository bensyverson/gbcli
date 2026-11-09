//! Why a checkpoint didn't load, what to do about it, and what loaded with a
//! warning. Each refusal has one protocol error code (design §5) and a remedy.

use std::fmt;

use gbcli_core::Model;
use gbcli_protocol::{CoreId, ErrorCode, Sha256, model_name};

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
    /// Step 1: not an envelope, or an unknown version.
    Unreadable { reason: String },
    /// Step 2: written by another backend.
    OtherBackend { saved: String, ours: String },
    /// Step 2 or 7: the core refused the blob.
    CoreRejected {
        saved: CoreId,
        ours: CoreId,
        reason: String,
    },
    /// Step 3: the caller asked for another model.
    ModelConflict { saved: Model, requested: Model },
    /// Step 3: the state is from another cart: another flash board, or
    /// flash where this one is mask ROM (`None`), or the reverse.
    FlashConflict {
        saved: Option<String>,
        ours: Option<String>,
    },
    /// Step 4: no boot ROM we have matches.
    BootRomMissing { sha256: Sha256 },
    /// Step 5: the ROM changed and there are no symbols to check it by.
    SymbolsMissing { side: SymbolSide },
    /// Step 5: the ROM changed under something the state depends on.
    RomChanged { moved: Vec<Moved> },
    /// Step 6: a patch's replaced bytes differ in the current ROM.
    PatchConflict {
        offset: u32,
        expected: Vec<u8>,
        found: Vec<u8>,
    },
    /// Step 6: the core refused a patch.
    PatchRefused { offset: u32, reason: String },
    /// No checkpoint by that name.
    Unknown { name: String, known: Vec<String> },
}

impl CheckpointError {
    /// The protocol code. No wildcard: a new variant needs a code to compile.
    pub fn code(&self) -> ErrorCode {
        match self {
            CheckpointError::Unreadable { .. } => ErrorCode::CheckpointUnreadable,
            CheckpointError::OtherBackend { .. }
            | CheckpointError::CoreRejected { .. }
            | CheckpointError::PatchRefused { .. } => ErrorCode::CheckpointMismatch,
            CheckpointError::ModelConflict { .. }
            | CheckpointError::FlashConflict { .. }
            | CheckpointError::Unknown { .. } => ErrorCode::BadRequest,
            CheckpointError::BootRomMissing { .. } => ErrorCode::BootRomMissing,
            CheckpointError::SymbolsMissing { .. }
            | CheckpointError::RomChanged { .. }
            | CheckpointError::PatchConflict { .. } => ErrorCode::RomChanged,
        }
    }

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
            CheckpointError::FlashConflict { saved, .. } => format!(
                "a session's cart is fixed when it is created: restore this state into a session \
                 created with {}",
                match saved {
                    Some(board) => format!("--flash {board}"),
                    None => "no --flash (mask ROM)".into(),
                }
            ),
            CheckpointError::BootRomMissing { sha256 } => {
                format!(
                    "the state was saved with a boot ROM this gbcli doesn't have (SHA-256 \
                     {sha256}); load it with the gbcli that saved it, or re-create it from the ROM"
                )
            }
            CheckpointError::SymbolsMissing { .. } => format!(
                "the ROM changed and can't be checked without symbols: rebuild with \
                 `rgblink -n game.sym`, {RESTART}, or pass --allow-rom-change"
            ),
            CheckpointError::RomChanged { .. } => {
                format!("{RESTART}, or pass --allow-rom-change to load the old state anyway")
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
            CheckpointError::FlashConflict { saved, ours } => {
                let cart = |board: &Option<String>| match board {
                    Some(board) => format!("the {board} flash board"),
                    None => "mask ROM".into(),
                };
                write!(
                    f,
                    "the state is from a cart on {}, and this session's is on {}",
                    cart(saved),
                    cart(ours)
                )
            }
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

/// How a session gets past a rebuild that its saved states can't cross.
const RESTART: &str = "restart the session on the ROM on disk with `gbcli run --from boot`, which \
                       keeps its breakpoints and watches (the old head stays in history, and \
                       checkpoints stay named)";

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
    /// The chip-address ranges of the changed flash sectors a load across a
    /// ROM change did not carry, because the new build's array holds code
    /// there (a byte other than $FF): the save no longer fits beside it. A
    /// sector the build leaves erased keeps the save. A power cycle across a
    /// rebuild drops the same sectors
    /// ([`Machine::power_cycle`](crate::Machine::power_cycle)).
    FlashSectorsDropped { sectors: Vec<std::ops::Range<u32>> },
}

#[cfg(test)]
mod tests {
    use super::*;

    fn core_id(commit: &str) -> CoreId {
        CoreId {
            backend: "sameboy".into(),
            commit: commit.into(),
        }
    }

    fn assert_code(e: CheckpointError, code: ErrorCode, exit: u8) {
        assert_eq!(e.code(), code, "{e:?}");
        assert_eq!(e.code().exit_code(), exit, "{e:?}");
    }

    #[test]
    fn an_unreadable_envelope_is_checkpoint_unreadable() {
        let e = CheckpointError::Unreadable {
            reason: "bad magic".into(),
        };
        assert_code(e, ErrorCode::CheckpointUnreadable, 4);
    }

    #[test]
    fn another_backend_is_a_checkpoint_mismatch() {
        let e = CheckpointError::OtherBackend {
            saved: "mgba".into(),
            ours: "sameboy".into(),
        };
        assert_code(e, ErrorCode::CheckpointMismatch, 4);
    }

    #[test]
    fn a_blob_the_core_rejects_is_a_checkpoint_mismatch() {
        let e = CheckpointError::CoreRejected {
            saved: core_id("1234+layer"),
            ours: core_id("1234+other"),
            reason: "another patch set".into(),
        };
        assert_code(e, ErrorCode::CheckpointMismatch, 4);
    }

    #[test]
    fn a_conflicting_model_is_a_bad_request() {
        let e = CheckpointError::ModelConflict {
            saved: Model::DmgB,
            requested: Model::Mgb,
        };
        assert_code(e, ErrorCode::BadRequest, 2);
    }

    #[test]
    fn a_state_from_another_cart_is_a_bad_request() {
        let e = CheckpointError::FlashConflict {
            saved: Some("generic-wr-aaa-a9".into()),
            ours: None,
        };
        assert!(
            e.remedy().contains("--flash generic-wr-aaa-a9"),
            "{}",
            e.remedy()
        );
        assert_code(e, ErrorCode::BadRequest, 2);
    }

    #[test]
    fn a_missing_boot_rom_is_boot_rom_missing() {
        let e = CheckpointError::BootRomMissing {
            sha256: Sha256([0xab; 32]),
        };
        assert_code(e, ErrorCode::BootRomMissing, 5);
    }

    #[test]
    fn a_changed_rom_without_symbols_is_rom_changed() {
        let e = CheckpointError::SymbolsMissing {
            side: SymbolSide::Both,
        };
        assert_code(e, ErrorCode::RomChanged, 3);
    }

    #[test]
    fn a_rom_that_moved_live_symbols_is_rom_changed() {
        let e = CheckpointError::RomChanged {
            moved: vec![Moved {
                name: "wSpeed".into(),
                role: MovedRole::Ram,
                old: SymbolAt {
                    bank: 0,
                    addr: 0xc000,
                },
                new: None,
            }],
        };
        assert_code(e, ErrorCode::RomChanged, 3);
    }

    #[test]
    fn a_rebuild_the_state_cant_cross_names_the_restart() {
        let refusals = [
            CheckpointError::SymbolsMissing {
                side: SymbolSide::Both,
            },
            CheckpointError::RomChanged { moved: Vec::new() },
        ];
        for e in refusals {
            assert!(e.remedy().contains("gbcli run --from boot"), "{e:?}");
        }
    }

    #[test]
    fn a_patch_over_changed_bytes_is_rom_changed() {
        let e = CheckpointError::PatchConflict {
            offset: 0x150,
            expected: vec![0x00],
            found: vec![0x76],
        };
        assert_code(e, ErrorCode::RomChanged, 3);
    }

    #[test]
    fn a_patch_the_core_refuses_is_a_checkpoint_mismatch() {
        let e = CheckpointError::PatchRefused {
            offset: 0x150,
            reason: "out of range".into(),
        };
        assert_code(e, ErrorCode::CheckpointMismatch, 4);
    }

    #[test]
    fn an_unknown_name_is_a_bad_request() {
        let e = CheckpointError::Unknown {
            name: "f30".into(),
            known: vec!["f10".into()],
        };
        assert_code(e, ErrorCode::BadRequest, 2);
    }
}
