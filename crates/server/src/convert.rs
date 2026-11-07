//! Engine and store errors as protocol error bodies: one code, one exit code,
//! one remedy with real values (design §5).

use gbcli_bundle::StoreError;
use gbcli_core::{CoreError, Feature};
use gbcli_engine::{
    DebugError, SetupError, addr::AddrError, cond::EvalError, cond::ExprError, core_code,
    eval_code, expr_code,
};
use gbcli_protocol::{ErrorBody, ErrorCode};

pub fn store(e: &StoreError) -> ErrorBody {
    ErrorBody::new(e.code(), e.to_string(), e.remedy())
}

pub fn addr(e: &AddrError) -> ErrorBody {
    ErrorBody::new(e.code(), e.to_string(), e.remedy())
}

pub fn expr(e: &ExprError) -> ErrorBody {
    ErrorBody::new(expr_code(e), e.to_string(), e.remedy())
}

pub fn eval(e: &EvalError) -> ErrorBody {
    match e {
        EvalError::UninitializedRead(read) => ErrorBody::new(
            eval_code(e),
            e.to_string(),
            format!("{read}: {}", e.remedy()),
        ),
        EvalError::Core(core) => core_error(core),
        other => ErrorBody::new(eval_code(other), other.to_string(), other.remedy()),
    }
}

pub fn core_error(e: &CoreError) -> ErrorBody {
    let code = core_code(e);
    let remedy = match e {
        CoreError::Unsupported { feature, reason } => format!(
            "the core can't {}: {reason}; `gbcli gaps` lists every such gap this project hit",
            feature.name()
        ),
        CoreError::OutOfRange { .. } => "check the address or offset".into(),
        CoreError::InvalidState { .. } => {
            "the machine's current state refuses this; change it first".into()
        }
        CoreError::Backend { .. } => ErrorCode::Internal.default_remedy().into(),
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
        DebugError::UntilSyntax { error } => {
            let mut body = expr(error);
            body.message = format!("until: {}", body.message);
            body
        }
        DebugError::Until { error, stop } => {
            let mut body = eval(error);
            body.message = format!("until: {}", body.message);
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

/// A gap-ledger entry: a feature the core said it can't do, and why.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Gap {
    pub feature: Feature,
    pub reason: &'static str,
}

/// The gap-ledger entry a core failure deserves: every `Unsupported`.
pub fn gap_of_core(e: &CoreError) -> Option<Gap> {
    match e {
        CoreError::Unsupported { feature, reason } => Some(Gap {
            feature: *feature,
            reason,
        }),
        CoreError::OutOfRange { .. }
        | CoreError::InvalidState { .. }
        | CoreError::Backend { .. } => None,
    }
}

/// The gap-ledger entry an evaluation failure deserves, if any.
pub fn gap_of_eval(e: &EvalError) -> Option<Gap> {
    match e {
        EvalError::Core(core) => gap_of_core(core),
        _ => None,
    }
}

/// The gap-ledger entry a store failure deserves: an `Unsupported` from the
/// core while building the machine or running the boot ROM.
pub fn gap_of_store(e: &StoreError) -> Option<Gap> {
    match e {
        StoreError::BootFailed(debug) => gap_of(debug),
        StoreError::Setup(SetupError::Core(core)) => gap_of_core(core),
        // No other store failure carries a core error.
        _ => None,
    }
}

/// The gap-ledger entry a failure deserves, if any.
pub fn gap_of(e: &DebugError) -> Option<Gap> {
    let core = match e {
        DebugError::Core(core) => core,
        DebugError::Eval {
            error: EvalError::Core(core),
            ..
        }
        | DebugError::Until {
            error: EvalError::Core(core),
            ..
        } => core,
        _ => return None,
    };
    gap_of_core(core)
}

#[cfg(test)]
mod tests {
    use super::*;
    use gbcli_core::CoreError;

    #[test]
    fn a_core_refusal_is_not_refused_which_means_a_flag_is_missing() {
        let state = core_error(&CoreError::InvalidState {
            reason: "the LCD is off".into(),
        });
        assert_eq!(state.code, ErrorCode::BadRequest);
        let backend = core_error(&CoreError::Backend {
            message: "the core said no".into(),
        });
        assert_eq!(backend.code, ErrorCode::Internal);
    }
}
