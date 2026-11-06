//! Engine and store errors as protocol error bodies: one code, one exit code,
//! one remedy with real values (design §5).

use gbcli_bundle::StoreError;
use gbcli_engine::{DebugError, addr::AddrError, cond::EvalError, cond::ExprError};
use gbcli_protocol::{ErrorBody, ErrorCode};

pub fn store(e: &StoreError) -> ErrorBody {
    ErrorBody::new(e.code(), e.to_string(), e.remedy())
}

pub fn addr(e: &AddrError) -> ErrorBody {
    ErrorBody::new(e.code(), e.to_string(), e.remedy())
}

pub fn expr(e: &ExprError) -> ErrorBody {
    let code = match e {
        ExprError::Syntax { .. } => ErrorCode::BadExpression,
        ExprError::UnknownSymbol { .. } => ErrorCode::UnknownSymbol,
        ExprError::AmbiguousSymbol { .. } => ErrorCode::AmbiguousSymbol,
    };
    ErrorBody::new(code, e.to_string(), e.remedy())
}

pub fn eval(e: &EvalError) -> ErrorBody {
    match e {
        EvalError::UninitializedRead(read) => ErrorBody::new(
            ErrorCode::UninitializedRead,
            e.to_string(),
            format!("{read}: {}", e.remedy()),
        ),
        EvalError::Core(core) => core_error(core),
        other => ErrorBody::new(ErrorCode::BadExpression, other.to_string(), other.remedy()),
    }
}

pub fn core_error(e: &gbcli_core::CoreError) -> ErrorBody {
    use gbcli_core::CoreError;
    let code = match e {
        CoreError::Unsupported { .. } => ErrorCode::Unsupported,
        CoreError::OutOfRange { .. } => ErrorCode::BadAddress,
        CoreError::InvalidState { .. } => ErrorCode::Refused,
        CoreError::Backend { .. } => ErrorCode::Refused,
    };
    let remedy = match e {
        CoreError::Unsupported { feature, reason } => format!(
            "the core can't {}: {reason} — see `gbcli doctor --core`",
            feature.name()
        ),
        CoreError::OutOfRange { .. } => "check the address or offset".into(),
        CoreError::InvalidState { .. } => {
            "the machine's current state refuses this; change it first".into()
        }
        CoreError::Backend { .. } => "this is a backend fault; retry, then report it".into(),
    };
    ErrorBody::new(code, e.to_string(), remedy)
}

pub fn debug(e: &DebugError) -> ErrorBody {
    match e {
        DebugError::Condition { id, error } => {
            let mut body = expr(error);
            body.message = format!("condition of #{id}: {}", body.message);
            body
        }
        DebugError::Eval { id, error, stop } => {
            let mut body = eval(error);
            body.message = format!("condition of #{id}: {}", body.message);
            body.with_stop((**stop).clone())
        }
        DebugError::Core(core) => core_error(core),
        DebugError::NeedsAddress => ErrorBody::new(
            ErrorCode::BadRequest,
            "step doesn't take an address",
            "use `run --to <address>`",
        ),
    }
}

/// The gap-ledger entry an evaluation failure deserves, if any.
pub fn gap_of_eval(e: &EvalError) -> Option<(&'static str, &'static str)> {
    match e {
        EvalError::Core(gbcli_core::CoreError::Unsupported { feature, reason }) => {
            Some((feature.name(), reason))
        }
        _ => None,
    }
}

/// The gap-ledger entry a failure deserves, if any.
pub fn gap_of(e: &DebugError) -> Option<(&'static str, &'static str)> {
    let core = match e {
        DebugError::Core(core) => core,
        DebugError::Eval {
            error: EvalError::Core(core),
            ..
        } => core,
        _ => return None,
    };
    match core {
        gbcli_core::CoreError::Unsupported { feature, reason } => Some((feature.name(), reason)),
        _ => None,
    }
}
