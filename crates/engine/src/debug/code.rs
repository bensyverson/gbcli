//! The protocol code each debugger failure maps to (design §5): one mapping,
//! so the server and the bundle can't drift apart. Every match is
//! exhaustive, so a new variant fails to compile until it has a code.

use gbcli_core::CoreError;
use gbcli_protocol::ErrorCode;

use super::DebugError;
use crate::cond::{EvalError, ExprError};

/// A core's refusal. `InvalidState` means the request doesn't fit the
/// machine's state, a caller's mistake; `Backend` is the core failing, which
/// is gbcli's fault, never the caller's.
pub fn core_code(e: &CoreError) -> ErrorCode {
    match e {
        CoreError::Unsupported { .. } => ErrorCode::Unsupported,
        CoreError::OutOfRange { .. } => ErrorCode::BadAddress,
        CoreError::InvalidState { .. } => ErrorCode::BadRequest,
        CoreError::Backend { .. } => ErrorCode::Internal,
    }
}

pub fn expr_code(e: &ExprError) -> ErrorCode {
    match e {
        ExprError::Syntax { .. } => ErrorCode::BadExpression,
        ExprError::UnknownSymbol { .. } | ExprError::NoSymbols { .. } => ErrorCode::UnknownSymbol,
        ExprError::AmbiguousSymbol { .. } => ErrorCode::AmbiguousSymbol,
    }
}

pub fn eval_code(e: &EvalError) -> ErrorCode {
    match e {
        EvalError::UninitializedRead(_) => ErrorCode::UninitializedRead,
        EvalError::Core(core) => core_code(core),
        EvalError::DivideByZero
        | EvalError::NoAccess { .. }
        | EvalError::OldUnknown
        | EvalError::ShiftOutOfRange { .. }
        | EvalError::AddressOutOfRange { .. }
        | EvalError::BlockPastArea { .. } => ErrorCode::BadExpression,
    }
}

impl DebugError {
    pub fn code(&self) -> ErrorCode {
        match self {
            DebugError::Condition { error, .. } | DebugError::UntilSyntax { error } => {
                expr_code(error)
            }
            DebugError::Eval { error, .. } | DebugError::Until { error, .. } => eval_code(error),
            DebugError::Core(core) => core_code(core),
            DebugError::NeedsAddress => ErrorCode::BadRequest,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use gbcli_core::Feature;

    #[test]
    fn a_core_failure_is_never_refused_which_means_a_flag_is_missing() {
        let cases = [
            (
                CoreError::Unsupported {
                    feature: Feature::ExtraOam,
                    reason: "no backing store",
                },
                ErrorCode::Unsupported,
            ),
            (
                CoreError::OutOfRange {
                    what: "offset",
                    value: 9,
                    limit: 8,
                },
                ErrorCode::BadAddress,
            ),
            (
                CoreError::InvalidState {
                    reason: "not now".into(),
                },
                ErrorCode::BadRequest,
            ),
            (
                CoreError::Backend {
                    message: "fault".into(),
                },
                ErrorCode::Internal,
            ),
        ];
        for (error, code) in cases {
            assert_eq!(core_code(&error), code, "{error}");
            assert_eq!(DebugError::Core(error).code(), code);
        }
    }

    #[test]
    fn an_evaluation_failure_keeps_its_own_code() {
        assert_eq!(
            eval_code(&EvalError::DivideByZero),
            ErrorCode::BadExpression
        );
        assert_eq!(DebugError::NeedsAddress.code(), ErrorCode::BadRequest);
    }
}
