//! Runs and steps through the handler.

use super::*;
use crate::fixture::Fixture;
use crate::handle::test_support::{handler, request, run_frames};
use gbcli_core::BudgetKind;
use gbcli_protocol::{Op, Rev};
use std::time::Duration;

#[test]
fn an_explicit_budget_runs_and_says_it_was_not_defaulted() {
    let f = Fixture::new("explicit-budget");
    let response = run_frames(&mut handler(&f), 3);
    assert!(response.is_ok(), "{:?}", response.error());
    assert_eq!(response.rev, Some(Rev(1)));
    let stop = response.outcome.unwrap().stop.unwrap();
    assert_eq!(
        stop.reason,
        StopReason::BudgetExhausted {
            kind: BudgetKind::Frames
        }
    );
    assert_eq!(stop.budget.frames, 3);
    assert_eq!(stop.budget.limits.frames, Some(3));
    assert!(!stop.budget.defaulted);
}

#[test]
fn a_run_without_a_budget_gets_the_default_and_the_record_says_so() {
    let f = Fixture::new("default-budget");
    let response = handler(&f).handle(&request(Op::Run(RunArgs::default())));
    let error = response.outcome.expect_err("a bare run ran out of budget");
    assert_eq!(error.code, ErrorCode::BudgetExhausted);
    let stop = error.stop.expect("the stop record rides on the error");
    assert!(stop.budget.defaulted);
    assert_eq!(stop.budget.limits.frames, Some(DEFAULT_FRAMES));
    assert_eq!(stop.budget.frames, DEFAULT_FRAMES);
}

#[test]
fn from_power_on_skips_the_head_and_the_boot_cache() {
    let f = Fixture::new("power-on");
    let mut h = handler(&f);
    assert!(run_frames(&mut h, 2).is_ok());
    let response = h.handle(&request(Op::Run(RunArgs {
        from: Some(gbcli_protocol::Start::PowerOn),
        frames: Some(1),
        ..RunArgs::default()
    })));
    let stop = response.outcome.unwrap().stop.unwrap();
    assert_eq!(stop.time.since, gbcli_protocol::TimeBase::PowerOn);
    assert_eq!(
        stop.time.frame, 1,
        "one vblank since power-on, not from boot"
    );
}

#[test]
fn until_stops_where_the_predicate_holds_and_names_its_cadence() {
    let f = Fixture::new("until");
    let mut h = handler(&f);
    assert!(run_frames(&mut h, 2).is_ok());
    let response = h.handle(&request(Op::Run(RunArgs {
        until: Some("u16[$C000] >= 2000".into()),
        frames: Some(600),
        ..RunArgs::default()
    })));
    let stop = response.outcome.unwrap().stop.unwrap();
    assert!(
        matches!(stop.reason, StopReason::Predicate { ref cadence, .. } if *cadence == gbcli_protocol::Cadence::Writes),
        "{:?}",
        stop.reason
    );
    assert!(!stop.budget.defaulted);
}

#[test]
fn a_predicate_already_true_at_the_start_costs_no_instructions() {
    let f = Fixture::new("until-start");
    let mut h = handler(&f);
    assert!(run_frames(&mut h, 1).is_ok());
    let response = h.handle(&request(Op::Run(RunArgs {
        until: Some("1".into()),
        frames: Some(600),
        ..RunArgs::default()
    })));
    let stop = response.outcome.unwrap().stop.unwrap();
    assert!(matches!(stop.reason, StopReason::Predicate { .. }));
    assert_eq!(stop.budget.instructions, 0);
}

#[test]
fn a_predicate_reading_never_written_memory_waits_and_points_at_allow_uninit() {
    // Design §8 ruling: a stop trigger reads an unwritten byte as "not yet".
    let f = Fixture::new("until-uninit");
    let mut h = handler(&f);
    let response = h.handle(&request(Op::Run(RunArgs {
        until: Some("[$C003] == 0".into()),
        frames: Some(2),
        ..RunArgs::default()
    })));
    // Design §3 ruling: a target the budget ended first is unanswered.
    let error = response.error().expect("the predicate never held");
    assert_eq!(error.code, ErrorCode::BudgetExhausted);
    let stop = error.stop.as_ref().unwrap();
    assert!(matches!(stop.reason, StopReason::BudgetExhausted { .. }));
    assert!(
        response
            .warnings
            .iter()
            .any(|w| w.contains("$C003") && w.contains("--allow-uninit")),
        "{:?}",
        response.warnings
    );
}

#[test]
fn a_read_of_never_written_memory_still_fails() {
    let f = Fixture::new("read-uninit");
    let mut h = handler(&f);
    let response = h.handle(&request(Op::Run(RunArgs {
        read: vec!["[$C003]".into()],
        frames: Some(1),
        ..RunArgs::default()
    })));
    let error = response.outcome.expect_err("an uninitialized read fails");
    assert_eq!(error.code, ErrorCode::UninitializedRead);
    assert!(error.remedy.contains("--allow-uninit"), "{}", error.remedy);
}

#[test]
fn step_one_instruction_advances_and_saves_a_head() {
    let f = Fixture::new("step");
    let mut h = handler(&f);
    let response = h.handle(&request(Op::Step(StepArgs {
        unit: StepKind::Instruction,
    })));
    let stop = response.outcome.unwrap().stop.unwrap();
    assert_eq!(
        stop.reason,
        StopReason::StepDone {
            unit: StepKind::Instruction
        }
    );
    assert_eq!(response.rev, Some(Rev(1)));
}

fn run(h: &mut Handler, args: RunArgs) -> gbcli_protocol::Response {
    h.handle(&request(Op::Run(args)))
}

#[test]
fn allow_uninit_reaches_the_reads_after_the_run() {
    let f = Fixture::new("read-allow-uninit");
    let response = run(
        &mut handler(&f),
        RunArgs {
            read: vec!["[$C003]".into()],
            frames: Some(1),
            allow_uninit: true,
            ..RunArgs::default()
        },
    );
    assert!(response.is_ok(), "{:?}", response.error());
    assert!(
        response.warnings.iter().any(|w| w.contains("$C003")),
        "{:?}",
        response.warnings
    );
}

#[test]
fn a_failed_read_after_the_run_keeps_the_run() {
    let f = Fixture::new("read-after");
    let mut h = handler(&f);
    let response = run(
        &mut h,
        RunArgs {
            read: vec!["[$C003]".into()],
            frames: Some(1),
            ..RunArgs::default()
        },
    );
    assert_eq!(response.rev, Some(Rev(1)), "the run was thrown away");
    let error = response.outcome.expect_err("the read fails");
    assert_eq!(error.code, ErrorCode::UninitializedRead);
    assert!(error.stop.is_some(), "the stop rides on the read error");
}

/// A request refused before emulating leaves the session and the warm
/// machine as they were.
fn assert_refused_untouched(h: &mut Handler, args: RunArgs, code: ErrorCode) {
    assert!(run_frames(h, 1).is_ok());
    let response = run(h, args);
    assert_eq!(response.outcome.err().map(|e| e.code), Some(code));
    assert_eq!(response.rev, Some(Rev(1)));
    assert!(
        h.warm.is_some(),
        "the machine was taken, so it may have run"
    );
}

#[test]
fn a_bad_read_expression_fails_before_the_run() {
    let f = Fixture::new("read-bad");
    let args = RunArgs {
        read: vec!["1 +".into()],
        frames: Some(2),
        ..RunArgs::default()
    };
    assert_refused_untouched(&mut handler(&f), args, ErrorCode::BadExpression);
}

#[test]
fn a_bad_checkpoint_name_fails_before_the_run() {
    let f = Fixture::new("checkpoint-bad");
    let args = RunArgs {
        checkpoint: Some("../escape".into()),
        frames: Some(2),
        ..RunArgs::default()
    };
    assert_refused_untouched(&mut handler(&f), args, ErrorCode::BadRequest);
}

#[test]
fn until_and_to_together_are_refused_before_any_write() {
    let f = Fixture::new("until-and-to");
    let args = RunArgs {
        until: Some("1".into()),
        to: Some("$0150".into()),
        writes: vec![gbcli_protocol::WriteArg {
            kind: gbcli_protocol::WriteKind::Poke,
            target: "$C002".into(),
            bytes: vec![9],
        }],
        ..RunArgs::default()
    };
    assert_refused_untouched(&mut handler(&f), args, ErrorCode::BadRequest);
}

#[test]
fn a_bad_to_address_fails_before_the_run() {
    let f = Fixture::new("to-bad");
    let args = RunArgs {
        to: Some("nowhere".into()),
        frames: Some(2),
        ..RunArgs::default()
    };
    assert_refused_untouched(&mut handler(&f), args, ErrorCode::UnknownSymbol);
}

#[test]
fn an_until_that_does_not_compile_fails_before_any_write() {
    let f = Fixture::new("until-bad");
    let args = RunArgs {
        until: Some("1 +".into()),
        writes: vec![gbcli_protocol::WriteArg {
            kind: gbcli_protocol::WriteKind::Poke,
            target: "$C002".into(),
            bytes: vec![9],
        }],
        ..RunArgs::default()
    };
    assert_refused_untouched(&mut handler(&f), args, ErrorCode::BadExpression);
}

#[test]
fn a_target_the_budget_ends_first_is_budget_exhausted() {
    let f = Fixture::new("to-missed");
    let response = run(
        &mut handler(&f),
        RunArgs {
            to: Some("$0000".into()),
            frames: Some(2),
            ..RunArgs::default()
        },
    );
    let error = response.outcome.expect_err("PC never reaches $0000");
    assert_eq!(error.code, ErrorCode::BudgetExhausted);
    assert_eq!(error.exit, 3);
    assert!(error.stop.is_some());
}

#[test]
fn the_wall_guard_ends_a_run_and_says_so() {
    let f = Fixture::new("guard-run");
    let mut h = handler(&f);
    assert!(run_frames(&mut h, 1).is_ok(), "past the boot run");
    h.set_wall_guard(Duration::ZERO);
    let response = run_frames(&mut h, 600);
    let error = response.outcome.expect_err("the guard fired");
    assert_eq!(error.code, ErrorCode::BudgetExhausted);
    assert_eq!(error.stop.map(|s| s.reason), Some(StopReason::WallGuard));
    assert_eq!(response.rev, Some(Rev(2)), "the state reached is saved");
}

#[test]
fn the_wall_guard_ends_the_boot_run_too() {
    let f = Fixture::new("guard-boot");
    let mut h = handler(&f);
    h.set_wall_guard(Duration::ZERO);
    let response = run_frames(&mut h, 1);
    let error = response
        .outcome
        .expect_err("the guard fired during the boot run");
    assert_eq!(error.code, ErrorCode::BudgetExhausted);
    assert!(error.message.contains("boot"), "{}", error.message);
    let cached = std::fs::read_dir(f.bundle.cache_dir()).map_or(0, |d| d.count());
    assert_eq!(cached, 0, "an interrupted boot isn't a handoff to cache");
}

#[test]
fn the_wall_guard_ends_a_step_and_says_so() {
    let f = Fixture::new("guard-step");
    let mut h = handler(&f);
    assert!(run_frames(&mut h, 1).is_ok());
    h.set_wall_guard(Duration::ZERO);
    let response = h.handle(&request(Op::Step(StepArgs {
        unit: StepKind::Over,
    })));
    let error = response.outcome.expect_err("the guard fired");
    assert_eq!(error.code, ErrorCode::BudgetExhausted);
    assert_eq!(error.stop.map(|s| s.reason), Some(StopReason::WallGuard));
}
