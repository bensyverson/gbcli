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

/// What every wall-guard error tells the caller to do.
pub(in crate::handle) const WALL_GUARD_REMEDY: &str = "host time ran out, not the emulated budget: \
     raise --wall-guard SECONDS (up to 3600), or split the work into shorter requests \
     (run --checkpoint NAME, then continue with --at NAME)";

/// The wall guard's answer to a run or step: `wall_guard`, and not
/// reproducible.
pub(super) fn wall_guard_error(guard: Duration, stop: StopRecord) -> ErrorBody {
    ErrorBody::new(
        ErrorCode::WallGuard,
        format!("the {guard:?} wall guard ended the request; this stop isn't reproducible"),
        WALL_GUARD_REMEDY,
    )
    .with_stop(stop)
}

/// The wall guard's answer to work with no stop of its own (a measurement,
/// an equivalence check); `reached` says how far it got, and `remedy` is
/// the work's own way to fit under the guard.
pub(in crate::handle) fn wall_guard_ended(
    guard: Duration,
    reached: &str,
    remedy: &str,
) -> ErrorBody {
    ErrorBody::new(
        ErrorCode::WallGuard,
        format!("the {guard:?} wall guard ended the request {reached}"),
        remedy,
    )
}

/// What a targeted run's stop means for its target (design §11, the
/// stop-reason table of 2026-11-07).
enum Ending {
    /// The target was reached, or the run had none to miss.
    Answered,
    /// A breakpoint or watch the caller set fired first: their own stop, but
    /// short of the target, so the run says so in a warning.
    CallersStop(String),
    /// The CPU locked up short of the target: `target_not_reached`.
    LockedUp {
        reason: &'static str,
        why: String,
        remedy: &'static str,
    },
}

fn ending(stop: &StopRecord) -> Ending {
    match &stop.reason {
        StopReason::Breakpoint { id } => Ending::CallersStop(format!("breakpoint #{id}")),
        StopReason::Watchpoint { id, .. } => Ending::CallersStop(format!("watch #{id}")),
        StopReason::IllegalOpcode { opcode } => Ending::LockedUp {
            reason: "illegal_opcode",
            why: format!(
                "the CPU locked up on illegal opcode ${opcode:02X} at ${:04X}",
                stop.pc
            ),
            remedy: "execution reached bytes that aren't code (a bad jump, a return \
                     through a broken stack, or a routine run from where it can't run); \
                     set a breakpoint before it and step toward it",
        },
        StopReason::HaltForever => Ending::LockedUp {
            reason: "halt_forever",
            why: format!(
                "HALT at ${:04X} with no interrupt enabled in IE: nothing can wake it",
                stop.pc
            ),
            remedy: "enable the interrupt that should wake it in rIE before the HALT \
                     (and ei, if its handler should run)",
        },
        StopReason::Stopped => Ending::LockedUp {
            reason: "stopped",
            why: format!("STOP at ${:04X} with no key held to wake it", stop.pc),
            remedy: "press a key that wakes it with --keys (as `A@0`), with its joypad \
                     row selected in rP1, or check why STOP ran",
        },
        // The target itself, the budget (`budget_exhausted` below), the wall
        // guard and a predicate or condition error each answer on their own;
        // a step's units never end a run.
        StopReason::StepDone { .. }
        | StopReason::Predicate { .. }
        | StopReason::PredicateError { .. }
        | StopReason::ConditionError { .. }
        | StopReason::BudgetExhausted { .. }
        | StopReason::UserBreak
        | StopReason::WallGuard => Ending::Answered,
    }
}

/// What a run with a target was asked, as an error message ends.
fn missed(args: &RunArgs) -> Option<String> {
    match (&args.until, &args.to) {
        (Some(until), _) => Some(format!("`{until}` never held")),
        (None, Some(to)) => Some(format!("PC never reached {to}")),
        (None, None) => None,
    }
}

/// The warning for a targeted run that the caller's own breakpoint or watch
/// stopped first.
pub(super) fn short_of_target(args: &RunArgs, stop: &StopRecord) -> Option<String> {
    let missed = missed(args)?;
    let Ending::CallersStop(which) = ending(stop) else {
        return None;
    };
    Some(format!(
        "{which} stopped the run first; {missed} (run again to continue toward it)"
    ))
}

/// The error a run's stop is, if it didn't answer the question. A run with a
/// target (`--until`, `--to`) that its budget ended is unanswered whatever
/// the budget; without a target, reaching an explicit budget is the answer
/// and a defaulted one is not (design §3 ruling, 2026-11-06). A targeted run
/// the CPU locked up in is unanswered too (§11, 2026-11-07).
pub(super) fn unanswered(args: &RunArgs, defaulted: bool, stop: &StopRecord) -> Option<ErrorBody> {
    if let (
        Some(missed),
        Ending::LockedUp {
            reason,
            why,
            remedy,
        },
    ) = (missed(args), ending(stop))
    {
        let message = format!(
            "the run stopped with {reason} at frame {}; {why}, and {missed}",
            stop.time.frame
        );
        let remedy = format!("the stop is {reason}: {remedy}");
        return Some(
            ErrorBody::new(ErrorCode::TargetNotReached, message, remedy).with_stop(stop.clone()),
        );
    }
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
