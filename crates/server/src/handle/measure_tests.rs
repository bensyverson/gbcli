//! `measure` and `run --measure` through the handler (design §11). The V1
//! test ROM's main loop calls Work and jumps back; by the opcode table a
//! pass is `call` 24 + Work (`ld a,[a16]` 16, `add` 4, `ld b,a` 4,
//! `ld hl,d16` 12, `inc [hl]` 12, `jr nz` taken 12, `ret` 16) + `jr` 12 =
//! 112 T while wFrame's low byte doesn't wrap.

use std::time::Duration;

use gbcli_protocol::{
    CheckpointArgs, CpuSpeedName, ErrorCode, MeasureArgs, MeasureMade, Op, Response, RunArgs,
    RunMeasure, Start, StopReason,
};

use super::Handler;
use super::run::WALL_GUARD_REMEDY;
use super::test_support::{handler, request, run_frames};
use crate::fixture::Fixture;

const PASS_T: u64 = 112;

fn measure(h: &mut Handler, args: MeasureArgs) -> Response {
    h.handle(&request(Op::Measure(args)))
}

fn main_loop(iterations: u32) -> MeasureArgs {
    MeasureArgs {
        from: "Main.loop".into(),
        to: "Main.loop".into(),
        iterations: Some(iterations),
        ..MeasureArgs::default()
    }
}

fn made(response: Response) -> MeasureMade {
    response
        .outcome
        .expect("measured")
        .measure
        .expect("a measurement")
}

#[test]
fn measure_counts_each_pass_of_the_main_loop_at_its_table_cost() {
    let f = Fixture::with_sym("measure-loop");
    let mut h = handler(&f);
    run_frames(&mut h, 2);
    let m = made(measure(&mut h, main_loop(3)));
    assert_eq!(m.n, 3);
    assert_eq!(m.speed, CpuSpeedName::Single);
    assert_eq!(
        (m.t.min, m.t.max, m.t.mean.0),
        (PASS_T, PASS_T, PASS_T as f64)
    );
    assert_eq!((m.m.min, m.m.max), (PASS_T / 4, PASS_T / 4));
    // 224 ticks of a 140,448-tick frame.
    let pct = 224.0 * 100.0 / 140_448.0;
    assert_eq!((m.frame_pct.min.0, m.frame_pct.max.0), (pct, pct));
    assert_eq!((m.from.as_str(), m.to.as_str()), ("Main.loop", "Main.loop"));
}

#[test]
fn measure_leaves_the_head_and_its_history_alone() {
    let f = Fixture::with_sym("measure-head");
    let mut h = handler(&f);
    let ran = run_frames(&mut h, 2);
    let before = h.handle(&request(Op::Show)).outcome.expect("show");
    let measured = measure(&mut h, main_loop(5));
    assert_eq!(measured.rev, ran.rev, "no new revision");
    let after = h.handle(&request(Op::Show)).outcome.expect("show");
    assert_eq!(before.registers, after.registers);
    assert_eq!(before.stop, after.stop);
}

#[test]
fn measure_at_a_checkpoint_measures_there_and_leaves_the_head() {
    let f = Fixture::with_sym("measure-at");
    let mut h = handler(&f);
    run_frames(&mut h, 1);
    h.handle(&request(Op::Checkpoint(CheckpointArgs {
        name: "one".into(),
    })))
    .outcome
    .expect("saved");
    let ran = run_frames(&mut h, 3);
    let before = h.handle(&request(Op::Show)).outcome.expect("show");
    let mut args = main_loop(2);
    args.at = Some("one".into());
    let m = made(measure(&mut h, args));
    assert_eq!(m.n, 2);
    let after = measure(&mut h, main_loop(1));
    assert_eq!(after.rev, ran.rev);
    let shown = h.handle(&request(Op::Show)).outcome.expect("show");
    assert_eq!(before.registers, shown.registers);
}

#[test]
fn measure_max_exceeded_is_predicate_false_with_the_measurement() {
    let f = Fixture::with_sym("measure-max");
    let mut h = handler(&f);
    let mut args = main_loop(2);
    args.max_t = Some(PASS_T - 1);
    let error = measure(&mut h, args).outcome.expect_err("exceeded");
    assert_eq!(error.code, ErrorCode::PredicateFalse);
    assert_eq!(error.exit, 1);
    assert!(error.message.contains("112"), "{}", error.message);
    let m = error
        .result
        .and_then(|r| r.measure)
        .expect("the measurement rides along");
    assert_eq!((m.t.max, m.max_t), (PASS_T, Some(PASS_T - 1)));
}

#[test]
fn measure_max_met_succeeds() {
    let f = Fixture::with_sym("measure-max-ok");
    let mut h = handler(&f);
    let mut args = main_loop(2);
    args.max_t = Some(PASS_T);
    assert_eq!(made(measure(&mut h, args)).max_t, Some(PASS_T));
}

#[test]
fn measure_refuses_zero_iterations() {
    let f = Fixture::with_sym("measure-zero");
    let error = measure(&mut handler(&f), main_loop(0))
        .outcome
        .expect_err("refused");
    assert_eq!(error.code, ErrorCode::BadRequest);
    assert!(error.remedy.contains("--iterations"), "{}", error.remedy);
}

#[test]
fn measure_out_of_budget_says_how_far_it_got() {
    let f = Fixture::with_sym("measure-budget");
    let mut args = main_loop(2);
    args.to = "$0000".into();
    args.frames = Some(1);
    let error = measure(&mut handler(&f), args)
        .outcome
        .expect_err("out of budget");
    assert_eq!(error.code, ErrorCode::BudgetExhausted);
    assert!(
        error
            .message
            .contains("before reaching --to after 0 iterations"),
        "{}",
        error.message
    );
    assert!(error.remedy.contains("--frames"), "{}", error.remedy);
}

#[test]
fn measure_names_a_bad_address() {
    let f = Fixture::with_sym("measure-addr");
    let mut args = main_loop(1);
    args.to = "Nowhere".into();
    let error = measure(&mut handler(&f), args)
        .outcome
        .expect_err("refused");
    assert_eq!(error.code, ErrorCode::UnknownSymbol);
}

#[test]
fn run_measure_measures_one_pass_on_a_copy_of_the_stop() {
    let plain = Fixture::with_sym("run-measure-plain");
    let plain_stop = run_frames(&mut handler(&plain), 2)
        .outcome
        .expect("ran")
        .stop;
    let f = Fixture::with_sym("run-measure");
    let result = handler(&f)
        .handle(&request(Op::Run(RunArgs {
            frames: Some(2),
            measure: Some(Box::new(RunMeasure {
                from: "Main.loop".into(),
                to: "Main.loop".into(),
            })),
            ..RunArgs::default()
        })))
        .outcome
        .expect("ran");
    let m = result.measure.expect("a measurement");
    assert_eq!((m.n, m.t.max), (1, PASS_T));
    assert_eq!(result.stop, plain_stop, "the head is the run's stop");
}

#[test]
fn the_wall_guard_ends_a_measurement() {
    // 3000 frames looking for an arrival that never comes: seconds of work
    // the guard must end at once.
    let f = Fixture::with_sym("measure-guard");
    let mut h = handler(&f);
    assert!(run_frames(&mut h, 1).is_ok(), "past the boot run");
    h.set_wall_guard(Duration::ZERO);
    let mut args = main_loop(1);
    args.to = "$0000".into();
    args.frames = Some(3000);
    let error = measure(&mut h, args).outcome.expect_err("the guard fired");
    assert_eq!(error.code, ErrorCode::WallGuard);
    assert!(error.message.contains("wall guard"), "{}", error.message);
    assert_eq!(error.remedy, WALL_GUARD_REMEDY);
}

/// A measurement the caller stops says so, not that the wall guard did.
#[test]
fn the_callers_stop_ends_a_measurement_as_the_callers() {
    let f = Fixture::with_sym("measure-caller");
    let interrupt = crate::InterruptFlag::new();
    let mut h = Handler::new(f.host(), f.bundle.clone(), Box::new(interrupt.clone()))
        .with_defaults(Some("demo".into()), None);
    assert!(run_frames(&mut h, 1).is_ok(), "past the boot run");
    let mut args = main_loop(1);
    args.to = "$0000".into();
    args.frames = Some(1_000_000);
    let setter = super::wall_guard_tests::stop_after(&interrupt, Duration::from_millis(50));
    let error = measure(&mut h, args).outcome.expect_err("stopped");
    setter.join().unwrap();
    assert_eq!(error.code, ErrorCode::Interrupted);
    assert!(error.message.contains("the caller"), "{}", error.message);
    assert!(!error.message.contains("wall guard"), "{}", error.message);
    assert!(!error.remedy.contains("--wall-guard"), "{}", error.remedy);
}

#[test]
fn the_wall_guard_ends_a_runs_measurement() {
    // The guard falls after the one-frame run and during its measurement,
    // which looks 600 frames (about half a second) for an arrival never made.
    let f = Fixture::with_sym("run-measure-guard");
    let mut h = handler(&f);
    assert!(run_frames(&mut h, 1).is_ok(), "past the boot run");
    h.set_wall_guard(Duration::from_millis(150));
    let error = h
        .handle(&request(Op::Run(RunArgs {
            frames: Some(1),
            measure: Some(Box::new(RunMeasure {
                from: "Main.loop".into(),
                to: "$0000".into(),
            })),
            ..RunArgs::default()
        })))
        .outcome
        .expect_err("the guard fired");
    let stop = error.stop.as_ref().expect("the run's stop rides along");
    assert_ne!(
        stop.reason,
        StopReason::WallGuard,
        "the run itself finished"
    );
    assert_eq!(error.code, ErrorCode::WallGuard);
    assert!(error.message.contains("wall guard"), "{}", error.message);
    assert_eq!(error.remedy, WALL_GUARD_REMEDY);
}

/// Key events the measurement ended before are reported, as a run reports
/// them: never silently dropped.
#[test]
fn measure_reports_the_key_events_it_never_reached() {
    let f = Fixture::with_sym("measure-keys-late");
    let mut h = handler(&f);
    run_frames(&mut h, 2);
    let mut args = main_loop(1);
    args.keys = gbcli_protocol::parse_keys("A@0 START@500").unwrap();
    let response = measure(&mut h, args);
    // The result lists it, as a run's does, and no warning repeats it.
    assert!(response.warnings.is_empty(), "{:?}", response.warnings);
    let result = response.outcome.expect("measured");
    let late: Vec<String> = result
        .keys_unapplied
        .expect("the late event")
        .iter()
        .map(|k| k.to_string())
        .collect();
    assert_eq!(late, vec!["START@500"]);
}

#[test]
fn measure_from_boot_or_power_on_starts_a_cold_copy_and_leaves_the_head() {
    let f = Fixture::with_sym("measure-start");
    let mut h = handler(&f);
    let ran = run_frames(&mut h, 2);
    for start in [Start::Boot, Start::PowerOn] {
        let args = MeasureArgs {
            start: Some(start),
            ..main_loop(2)
        };
        let m = made(measure(&mut h, args));
        assert_eq!((m.t.min, m.t.max), (PASS_T, PASS_T), "{start:?}");
    }
    let after = h.handle(&request(Op::Show));
    assert_eq!(after.rev, ran.rev);
}

#[test]
fn measure_refuses_at_with_from() {
    let f = Fixture::with_sym("measure-start-at");
    let mut h = handler(&f);
    run_frames(&mut h, 2);
    let args = MeasureArgs {
        at: Some("~0".into()),
        start: Some(Start::Boot),
        ..main_loop(1)
    };
    let err = measure(&mut h, args).outcome.expect_err("both");
    assert_eq!(err.code, ErrorCode::BadRequest);
}
