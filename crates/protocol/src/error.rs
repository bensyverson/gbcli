//! The closed error-code set and its exit codes (design §5). Every code maps
//! to exactly one house exit code and to a remedy worth printing.

use std::fmt;

use serde::{Deserialize, Serialize};

use crate::battery::FlashNoteRow;
use crate::keys::KeyEvent;
use crate::moved::MovedSymbol;
use crate::response::ResultBody;
use crate::stop::StopRecord;

/// Every way a request can fail. The wire spelling is snake_case.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ErrorCode {
    /// An assertion's condition was false: `measure --max`, `equiv`, `run
    /// --expect`.
    PredicateFalse,
    /// The last complete frame wasn't a normal one (LCD off, boot), so a
    /// screenshot of it was refused (design §9): pass `--any-frame`.
    FrameNotNormal,
    BadRequest,
    BadAddress,
    BadExpression,
    AmbiguousSymbol,
    UnknownSymbol,
    /// The verb needs a flag the caller didn't pass.
    Refused,
    BudgetExhausted,
    /// A run with a target (`--to`, `--until`) ended without reaching it,
    /// for a reason other than its budget: the CPU locked up (an illegal
    /// opcode, HALT or STOP that nothing can wake). The stop rides along.
    TargetNotReached,
    RevMismatch,
    SessionBusy,
    RomChanged,
    UninitializedRead,
    RomUnreadable,
    CheckpointUnreadable,
    CheckpointMismatch,
    SymbolsUnreadable,
    /// The session's own files (`session.json`) don't parse.
    SessionUnreadable,
    /// RGBDS refused the sources, a step wrote no output (DX 10), or an
    /// image broke a limit (in a build, or in `assets`, which has no code
    /// of its own); the report is in the error's `result`.
    BuildFailed,
    /// A song file (`.uge` or song text) is missing or malformed; the problem,
    /// located to a line and column or a byte offset, is in the error's `result`.
    SongUnreadable,
    /// An image file (PNG or pixel text) is missing or malformed, or names
    /// a palette that doesn't resolve; the problem, located to a line and
    /// column or a pixel, is in the error's `result`.
    PixelsUnreadable,
    /// A font file (`.yaff`) is missing or isn't gbcli's YAFF subset; the
    /// problem, located to a line and column, is in the error's `result`.
    FontUnreadable,
    Unsupported,
    NoSession,
    BootRomMissing,
    ToolchainMissing,
    /// The filesystem refused a read or write: permissions, a full disk.
    IoFailed,
    /// gbcli itself failed: a bug, or a backend fault. The server keeps
    /// running and trusts nothing it held in memory (design §5).
    Internal,
}

impl ErrorCode {
    /// The process exit code. One code, one exit code (design §5).
    pub fn exit_code(self) -> u8 {
        match self {
            ErrorCode::PredicateFalse | ErrorCode::FrameNotNormal => 1,
            ErrorCode::BadRequest
            | ErrorCode::BadAddress
            | ErrorCode::BadExpression
            | ErrorCode::AmbiguousSymbol
            | ErrorCode::UnknownSymbol
            | ErrorCode::Refused => 2,
            ErrorCode::BudgetExhausted
            | ErrorCode::TargetNotReached
            | ErrorCode::RevMismatch
            | ErrorCode::SessionBusy
            | ErrorCode::RomChanged
            | ErrorCode::UninitializedRead => 3,
            ErrorCode::RomUnreadable
            | ErrorCode::CheckpointUnreadable
            | ErrorCode::CheckpointMismatch
            | ErrorCode::SymbolsUnreadable
            | ErrorCode::SessionUnreadable
            | ErrorCode::BuildFailed
            | ErrorCode::SongUnreadable
            | ErrorCode::PixelsUnreadable
            | ErrorCode::FontUnreadable => 4,
            ErrorCode::Unsupported
            | ErrorCode::NoSession
            | ErrorCode::BootRomMissing
            | ErrorCode::ToolchainMissing
            | ErrorCode::IoFailed
            | ErrorCode::Internal => 5,
        }
    }

    /// A fallback remedy, when a constructor has nothing more specific.
    pub fn default_remedy(self) -> &'static str {
        match self {
            ErrorCode::PredicateFalse => "the predicate stayed false; relax it or raise the budget",
            ErrorCode::FrameNotNormal => {
                "pass --any-frame to take it anyway, or run until the LCD shows a frame"
            }
            ErrorCode::BadRequest => "see `gbcli help schema` for the request shape",
            ErrorCode::BadAddress => "addresses are $C000, 0xC000, 03:4123 or a .sym name",
            ErrorCode::BadExpression => "see `gbcli help expressions`",
            ErrorCode::AmbiguousSymbol => "qualify the symbol with a bank: 03:4123",
            ErrorCode::UnknownSymbol => {
                "check the name against `gbcli sym <pattern>`, or load the ROM's .sym"
            }
            ErrorCode::Refused => "the remedy names the flag this verb needs",
            ErrorCode::BudgetExhausted => "raise --frames, --ticks or --instructions",
            ErrorCode::TargetNotReached => {
                "the stop record says where the CPU locked up; step toward the target from an earlier state"
            }
            ErrorCode::RevMismatch => {
                "re-read the session, then repeat the request with its current rev"
            }
            ErrorCode::SessionBusy => "wait for the other writer, or use a different --session",
            ErrorCode::RomChanged => {
                "restore from a checkpoint of this build, or pass --allow-rom-change"
            }
            ErrorCode::UninitializedRead => {
                "read it after the program has written it (run further), or pass --allow-uninit to see the value anyway"
            }
            ErrorCode::RomUnreadable => "check the path and permissions",
            ErrorCode::CheckpointUnreadable => {
                "this gbcli can't read the file; save a new one with `gbcli checkpoint <name>`"
            }
            ErrorCode::CheckpointMismatch => {
                "this checkpoint needs another core or model; rebuild it"
            }
            ErrorCode::SymbolsUnreadable => "rebuild with `-n game.sym`, or pass --sym <file>",
            ErrorCode::SessionUnreadable => {
                "remove the session with `gbcli sessions rm <name>`, then create it again"
            }
            ErrorCode::BuildFailed => {
                "fix the first error at its file and line, then `gbcli build` again"
            }
            ErrorCode::SongUnreadable => {
                "fix the song at the line and column given; `gbcli help music` has the format"
            }
            ErrorCode::PixelsUnreadable => {
                "fix the image at the line and column (or pixel) given; `gbcli help pixels` has \
                 the format"
            }
            ErrorCode::FontUnreadable => {
                "fix the font at the line and column given; `gbcli help fonts` has the format"
            }
            ErrorCode::Unsupported => {
                "the core can't do this; `gbcli gaps` lists every such gap this project hit"
            }
            ErrorCode::NoSession => "pass --session <name> and --rom <file> to create one",
            ErrorCode::BootRomMissing => {
                "this gbcli has no boot ROM with that hash; re-create the state with `gbcli run`"
            }
            ErrorCode::ToolchainMissing => "install the pinned RGBDS, or put it on PATH",
            ErrorCode::IoFailed => "check the path's permissions and the free disk space",
            ErrorCode::Internal => {
                "this is a gbcli bug: retry once, then report the request line and the message"
            }
        }
    }
}

/// The wire spelling. A test holds it to serde's over every code.
impl fmt::Display for ErrorCode {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let name = match self {
            ErrorCode::PredicateFalse => "predicate_false",
            ErrorCode::FrameNotNormal => "frame_not_normal",
            ErrorCode::BadRequest => "bad_request",
            ErrorCode::BadAddress => "bad_address",
            ErrorCode::BadExpression => "bad_expression",
            ErrorCode::AmbiguousSymbol => "ambiguous_symbol",
            ErrorCode::UnknownSymbol => "unknown_symbol",
            ErrorCode::Refused => "refused",
            ErrorCode::BudgetExhausted => "budget_exhausted",
            ErrorCode::TargetNotReached => "target_not_reached",
            ErrorCode::RevMismatch => "rev_mismatch",
            ErrorCode::SessionBusy => "session_busy",
            ErrorCode::RomChanged => "rom_changed",
            ErrorCode::UninitializedRead => "uninitialized_read",
            ErrorCode::RomUnreadable => "rom_unreadable",
            ErrorCode::CheckpointUnreadable => "checkpoint_unreadable",
            ErrorCode::CheckpointMismatch => "checkpoint_mismatch",
            ErrorCode::SymbolsUnreadable => "symbols_unreadable",
            ErrorCode::SessionUnreadable => "session_unreadable",
            ErrorCode::BuildFailed => "build_failed",
            ErrorCode::SongUnreadable => "song_unreadable",
            ErrorCode::PixelsUnreadable => "pixels_unreadable",
            ErrorCode::FontUnreadable => "font_unreadable",
            ErrorCode::Unsupported => "unsupported",
            ErrorCode::NoSession => "no_session",
            ErrorCode::BootRomMissing => "boot_rom_missing",
            ErrorCode::ToolchainMissing => "toolchain_missing",
            ErrorCode::IoFailed => "io_failed",
            ErrorCode::Internal => "internal",
        };
        f.write_str(name)
    }
}

/// A failure, as one JSON object: code, exit code, message, remedy, and the
/// stop record when the machine got somewhere first.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ErrorBody {
    pub code: ErrorCode,
    /// The house exit code for `code`, repeated so a client needn't map it.
    pub exit: u8,
    pub message: String,
    pub remedy: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub stop: Option<StopRecord>,
    /// A failed run's key events past its stop, as a successful run returns
    /// them in `ResultBody::keys_unapplied`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub keys_unapplied: Option<Vec<KeyEvent>>,
    /// The flash notes a failed run or step took, as a successful one
    /// returns them in `ResultBody::flash_notes`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub flash_notes: Option<Vec<FlashNoteRow>>,
    /// An assertion's answer when it is no (`measure --max`, `equiv`): the
    /// result it asserted on, as the op returns it on success.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub result: Option<Box<ResultBody>>,
    /// `rom_changed`: every symbol the saved state depends on that the
    /// rebuild moved, where the message names only the first few.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub moved: Vec<MovedSymbol>,
}

impl ErrorBody {
    pub fn new(code: ErrorCode, message: impl Into<String>, remedy: impl Into<String>) -> Self {
        ErrorBody {
            code,
            exit: code.exit_code(),
            message: message.into(),
            remedy: remedy.into(),
            stop: None,
            keys_unapplied: None,
            flash_notes: None,
            result: None,
            moved: Vec::new(),
        }
    }

    /// The code's fallback remedy, for a constructor with nothing specific.
    pub fn plain(code: ErrorCode, message: impl Into<String>) -> Self {
        ErrorBody::new(code, message, code.default_remedy())
    }

    pub fn with_stop(mut self, stop: StopRecord) -> Self {
        self.stop = Some(stop);
        self
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Every code, so a new one can't be added without a test seeing it.
    const EVERY: &[ErrorCode] = &[
        ErrorCode::PredicateFalse,
        ErrorCode::FrameNotNormal,
        ErrorCode::BadRequest,
        ErrorCode::BadAddress,
        ErrorCode::BadExpression,
        ErrorCode::AmbiguousSymbol,
        ErrorCode::UnknownSymbol,
        ErrorCode::Refused,
        ErrorCode::BudgetExhausted,
        ErrorCode::TargetNotReached,
        ErrorCode::RevMismatch,
        ErrorCode::SessionBusy,
        ErrorCode::RomChanged,
        ErrorCode::UninitializedRead,
        ErrorCode::RomUnreadable,
        ErrorCode::CheckpointUnreadable,
        ErrorCode::CheckpointMismatch,
        ErrorCode::SymbolsUnreadable,
        ErrorCode::Unsupported,
        ErrorCode::NoSession,
        ErrorCode::BootRomMissing,
        ErrorCode::ToolchainMissing,
        ErrorCode::BuildFailed,
        ErrorCode::SongUnreadable,
        ErrorCode::PixelsUnreadable,
        ErrorCode::FontUnreadable,
        ErrorCode::SessionUnreadable,
        ErrorCode::IoFailed,
        ErrorCode::Internal,
    ];

    /// Fails to compile when a code is added, as a reminder to add it to `EVERY`.
    fn listed(code: ErrorCode) -> bool {
        match code {
            ErrorCode::PredicateFalse
            | ErrorCode::FrameNotNormal
            | ErrorCode::BadRequest
            | ErrorCode::BadAddress
            | ErrorCode::BadExpression
            | ErrorCode::AmbiguousSymbol
            | ErrorCode::UnknownSymbol
            | ErrorCode::Refused
            | ErrorCode::BudgetExhausted
            | ErrorCode::TargetNotReached
            | ErrorCode::RevMismatch
            | ErrorCode::SessionBusy
            | ErrorCode::RomChanged
            | ErrorCode::UninitializedRead
            | ErrorCode::RomUnreadable
            | ErrorCode::CheckpointUnreadable
            | ErrorCode::CheckpointMismatch
            | ErrorCode::SymbolsUnreadable
            | ErrorCode::SessionUnreadable
            | ErrorCode::Unsupported
            | ErrorCode::NoSession
            | ErrorCode::BootRomMissing
            | ErrorCode::ToolchainMissing
            | ErrorCode::BuildFailed
            | ErrorCode::SongUnreadable
            | ErrorCode::PixelsUnreadable
            | ErrorCode::FontUnreadable
            | ErrorCode::IoFailed
            | ErrorCode::Internal => EVERY.contains(&code),
        }
    }

    #[test]
    fn display_and_the_wire_spell_every_code_the_same_way() {
        for &code in EVERY {
            assert!(listed(code), "{code:?}");
            let wire = serde_json::to_value(code).unwrap();
            assert_eq!(
                wire,
                serde_json::Value::String(code.to_string()),
                "{code:?}"
            );
            let back: ErrorCode =
                serde_json::from_value(serde_json::Value::String(code.to_string())).unwrap();
            assert_eq!(back, code);
        }
    }

    #[test]
    fn every_error_code_has_a_non_empty_remedy_and_the_designed_exit_code() {
        for &code in EVERY {
            assert!(!code.default_remedy().trim().is_empty(), "{code:?}");
            let body = ErrorBody::plain(code, "boom");
            assert!(!body.remedy.trim().is_empty(), "{code:?}");
            assert_eq!(body.exit, code.exit_code(), "{code:?}");
        }
    }

    #[test]
    fn exit_codes_match_the_design_table() {
        let exits = [
            (ErrorCode::PredicateFalse, 1),
            (ErrorCode::FrameNotNormal, 1),
            (ErrorCode::BadRequest, 2),
            (ErrorCode::UnknownSymbol, 2),
            (ErrorCode::Refused, 2),
            (ErrorCode::BudgetExhausted, 3),
            (ErrorCode::TargetNotReached, 3),
            (ErrorCode::RevMismatch, 3),
            (ErrorCode::SessionBusy, 3),
            (ErrorCode::RomChanged, 3),
            (ErrorCode::UninitializedRead, 3),
            (ErrorCode::RomUnreadable, 4),
            (ErrorCode::CheckpointUnreadable, 4),
            (ErrorCode::CheckpointMismatch, 4),
            (ErrorCode::SymbolsUnreadable, 4),
            (ErrorCode::SessionUnreadable, 4),
            (ErrorCode::BuildFailed, 4),
            (ErrorCode::SongUnreadable, 4),
            (ErrorCode::PixelsUnreadable, 4),
            (ErrorCode::FontUnreadable, 4),
            (ErrorCode::Unsupported, 5),
            (ErrorCode::NoSession, 5),
            (ErrorCode::BootRomMissing, 5),
            (ErrorCode::ToolchainMissing, 5),
            (ErrorCode::IoFailed, 5),
            (ErrorCode::Internal, 5),
        ];
        for (code, exit) in exits {
            assert_eq!(code.exit_code(), exit, "{code:?}");
        }
    }

    #[test]
    fn an_error_body_round_trips_with_its_code_spelled_snake_case() {
        let body = ErrorBody::new(
            ErrorCode::BudgetExhausted,
            "ran 600 frames",
            "raise --frames",
        );
        let json = serde_json::to_value(&body).unwrap();
        assert_eq!(json["code"], "budget_exhausted");
        assert_eq!(json["exit"], 3);
        assert!(
            json.get("stop").is_none(),
            "no stop when the run never started"
        );
        let back: ErrorBody = serde_json::from_value(
            serde_json::json!({"code":"budget_exhausted","exit":3,"message":"m","remedy":"r"}),
        )
        .unwrap();
        assert_eq!(back.code, ErrorCode::BudgetExhausted);
    }
}
