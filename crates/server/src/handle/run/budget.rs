//! A run's budget, and what its stop answers (design §3).

use std::time::Duration;

use gbcli_core::Budget;
use gbcli_protocol::{BudgetLimits, ErrorBody, ErrorCode, RunArgs, StopReason, StopRecord};

/// The default budget for `run` without one (design Q4).
pub(super) const DEFAULT_FRAMES: u64 = 600;

/// The budget a run gets, and whether the server supplied the default.
pub(super) fn budget_of(args: &RunArgs) -> (Budget, BudgetLimits, bool) {
    budget_from(args.frames, args.instructions, args.ticks)
}

/// The budget the three limits give, 600 frames when none is given, and
/// whether that default applied.
pub(super) fn budget_from(
    frames: Option<u64>,
    instructions: Option<u64>,
    ticks: Option<u64>,
) -> (Budget, BudgetLimits, bool) {
    if frames.is_none() && instructions.is_none() && ticks.is_none() {
        let limits = BudgetLimits {
            frames: Some(DEFAULT_FRAMES),
            ..BudgetLimits::default()
        };
        return (Budget::frames(DEFAULT_FRAMES), limits, true);
    }
    let limits = BudgetLimits {
        ticks,
        instructions,
        frames,
    };
    let budget = Budget::frames(frames.unwrap_or(u64::MAX))
        .with_instructions(instructions.unwrap_or(u64::MAX))
        .with_ticks(ticks.unwrap_or(u64::MAX));
    (budget, limits, false)
}

/// The wall guard's answer: `budget_exhausted`, and not reproducible.
pub(super) fn wall_guard_error(guard: Duration, stop: StopRecord) -> ErrorBody {
    ErrorBody::new(
        ErrorCode::BudgetExhausted,
        format!("the {guard:?} wall guard ended the request; this stop isn't reproducible"),
        "narrow the experiment, or give it a structure that ends",
    )
    .with_stop(stop)
}

/// The error a run's stop is, if it didn't answer the question. A run with a
/// target (`--until`, `--to`) that its budget ended is unanswered whatever
/// the budget; without a target, reaching an explicit budget is the answer
/// and a defaulted one is not (design §3 ruling, 2026-11-06).
pub(super) fn unanswered(args: &RunArgs, defaulted: bool, stop: &StopRecord) -> Option<ErrorBody> {
    if !matches!(stop.reason, StopReason::BudgetExhausted { .. }) {
        return None;
    }
    let (what, remedy) = match (&args.until, &args.to) {
        (Some(until), _) => (
            format!("`{until}` never held"),
            format!("raise --frames, or check the predicate: {until}"),
        ),
        (None, Some(to)) => (
            format!("PC never reached {to}"),
            format!("raise --frames, or check the address: {to}"),
        ),
        (None, None) if defaulted => (
            "nothing stopped it".to_string(),
            "pass --frames, --ticks or --instructions to bound it".to_string(),
        ),
        (None, None) => return None,
    };
    let message = format!(
        "the budget ended the run at frame {}; {what}",
        stop.time.frame
    );
    Some(ErrorBody::new(ErrorCode::BudgetExhausted, message, remedy).with_stop(stop.clone()))
}
