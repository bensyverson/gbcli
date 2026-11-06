//! The closed error-code set and its exit codes (design §5). Every code maps
//! to exactly one house exit code and to a remedy worth printing.

use std::fmt;

use serde::{Deserialize, Serialize};

use crate::stop::StopRecord;

/// Every way a request can fail. The wire spelling is snake_case.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ErrorCode {
    /// An assertion verb's condition stayed false: `check`, `measure --max`.
    PredicateFalse,
    BadRequest,
    BadAddress,
    BadExpression,
    AmbiguousSymbol,
    UnknownSymbol,
    /// The verb needs a flag the caller didn't pass.
    Refused,
    BudgetExhausted,
    RevMismatch,
    SessionBusy,
    RomChanged,
    UninitializedRead,
    RomUnreadable,
    CheckpointUnreadable,
    CheckpointMismatch,
    SymbolsUnreadable,
    Unsupported,
    NoSession,
    BootRomMissing,
    ToolchainMissing,
}

impl ErrorCode {
    /// The process exit code. One code, one exit code (design §5).
    pub fn exit_code(self) -> u8 {
        match self {
            ErrorCode::PredicateFalse => 1,
            ErrorCode::BadRequest
            | ErrorCode::BadAddress
            | ErrorCode::BadExpression
            | ErrorCode::AmbiguousSymbol
            | ErrorCode::UnknownSymbol
            | ErrorCode::Refused => 2,
            ErrorCode::BudgetExhausted
            | ErrorCode::RevMismatch
            | ErrorCode::SessionBusy
            | ErrorCode::RomChanged
            | ErrorCode::UninitializedRead => 3,
            ErrorCode::RomUnreadable
            | ErrorCode::CheckpointUnreadable
            | ErrorCode::CheckpointMismatch
            | ErrorCode::SymbolsUnreadable => 4,
            ErrorCode::Unsupported
            | ErrorCode::NoSession
            | ErrorCode::BootRomMissing
            | ErrorCode::ToolchainMissing => 5,
        }
    }

    /// A fallback remedy, when a constructor has nothing more specific.
    pub fn default_remedy(self) -> &'static str {
        match self {
            ErrorCode::PredicateFalse => "the predicate stayed false; relax it or raise the budget",
            ErrorCode::BadRequest => "see `gbcli help schema` for the request shape",
            ErrorCode::BadAddress => "addresses are $C000, 0xC000, 03:4123 or a .sym name",
            ErrorCode::BadExpression => "see `gbcli help expressions`",
            ErrorCode::AmbiguousSymbol => "qualify the symbol with a bank: 03:4123",
            ErrorCode::UnknownSymbol => {
                "check the name against `gbcli sym <pattern>`, or load the ROM's .sym"
            }
            ErrorCode::Refused => "the remedy names the flag this verb needs",
            ErrorCode::BudgetExhausted => "raise --frames, --ticks or --instructions",
            ErrorCode::RevMismatch => {
                "re-read the session, then repeat the request with its current rev"
            }
            ErrorCode::SessionBusy => "wait for the other writer, or use a different --session",
            ErrorCode::RomChanged => {
                "restore from a checkpoint of this build, or pass --allow-rom-change"
            }
            ErrorCode::UninitializedRead => {
                "run from boot or later, or pass --allow-uninit to see the value anyway"
            }
            ErrorCode::RomUnreadable => "check the path and permissions",
            ErrorCode::CheckpointUnreadable => "check the checkpoint name with `gbcli checkpoints`",
            ErrorCode::CheckpointMismatch => {
                "this checkpoint needs another core or model; rebuild it"
            }
            ErrorCode::SymbolsUnreadable => "rebuild with `-n game.sym`, or pass --sym <file>",
            ErrorCode::Unsupported => "the core can't do this; see `gbcli doctor --core`",
            ErrorCode::NoSession => "pass --session <name> and --rom <file> to create one",
            ErrorCode::BootRomMissing => "pass --boot-rom <file> with the hash in the remedy",
            ErrorCode::ToolchainMissing => "install the pinned RGBDS, or put it on PATH",
        }
    }
}

impl fmt::Display for ErrorCode {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let name = match self {
            ErrorCode::PredicateFalse => "predicate_false",
            ErrorCode::BadRequest => "bad_request",
            ErrorCode::BadAddress => "bad_address",
            ErrorCode::BadExpression => "bad_expression",
            ErrorCode::AmbiguousSymbol => "ambiguous_symbol",
            ErrorCode::UnknownSymbol => "unknown_symbol",
            ErrorCode::Refused => "refused",
            ErrorCode::BudgetExhausted => "budget_exhausted",
            ErrorCode::RevMismatch => "rev_mismatch",
            ErrorCode::SessionBusy => "session_busy",
            ErrorCode::RomChanged => "rom_changed",
            ErrorCode::UninitializedRead => "uninitialized_read",
            ErrorCode::RomUnreadable => "rom_unreadable",
            ErrorCode::CheckpointUnreadable => "checkpoint_unreadable",
            ErrorCode::CheckpointMismatch => "checkpoint_mismatch",
            ErrorCode::SymbolsUnreadable => "symbols_unreadable",
            ErrorCode::Unsupported => "unsupported",
            ErrorCode::NoSession => "no_session",
            ErrorCode::BootRomMissing => "boot_rom_missing",
            ErrorCode::ToolchainMissing => "toolchain_missing",
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
}

impl ErrorBody {
    pub fn new(code: ErrorCode, message: impl Into<String>, remedy: impl Into<String>) -> Self {
        ErrorBody {
            code,
            exit: code.exit_code(),
            message: message.into(),
            remedy: remedy.into(),
            stop: None,
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
        ErrorCode::BadRequest,
        ErrorCode::BadAddress,
        ErrorCode::BadExpression,
        ErrorCode::AmbiguousSymbol,
        ErrorCode::UnknownSymbol,
        ErrorCode::Refused,
        ErrorCode::BudgetExhausted,
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
    ];

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
            (ErrorCode::BadRequest, 2),
            (ErrorCode::UnknownSymbol, 2),
            (ErrorCode::Refused, 2),
            (ErrorCode::BudgetExhausted, 3),
            (ErrorCode::RevMismatch, 3),
            (ErrorCode::SessionBusy, 3),
            (ErrorCode::RomChanged, 3),
            (ErrorCode::UninitializedRead, 3),
            (ErrorCode::RomUnreadable, 4),
            (ErrorCode::CheckpointUnreadable, 4),
            (ErrorCode::CheckpointMismatch, 4),
            (ErrorCode::SymbolsUnreadable, 4),
            (ErrorCode::Unsupported, 5),
            (ErrorCode::NoSession, 5),
            (ErrorCode::BootRomMissing, 5),
            (ErrorCode::ToolchainMissing, 5),
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
