//! The wall guard answers as itself, not as the emulated budget, and a
//! request can raise it (leaf xdES5r, from the drum play-test: full-song
//! runs under a load of 134 stopped at frames 4247 and 4288 as
//! `budget_exhausted`, exit 3, the same as a budget miss).

use std::time::Duration;

use gbcli_protocol::{ErrorCode, Op, RunArgs, StopReason, WallGuardSeconds};

use super::run::WALL_GUARD_REMEDY;
use super::test_support::{handler, request, run_frames};
use crate::InterruptFlag;
use crate::fixture::Fixture;

fn guarded_run(frames: u64, seconds: u64) -> gbcli_protocol::Request {
    let mut r = request(Op::Run(RunArgs {
        frames: Some(frames),
        ..RunArgs::default()
    }));
    r.wall_guard_s = Some(WallGuardSeconds::new(seconds).unwrap());
    r
}

#[test]
fn a_full_run_the_guard_ends_is_wall_guard_not_budget_exhausted() {
    let f = Fixture::new("guard-code");
    let mut h = handler(&f);
    assert!(run_frames(&mut h, 1).is_ok(), "past the boot run");
    h.set_wall_guard(Duration::ZERO);
    let error = run_frames(&mut h, 4700)
        .outcome
        .expect_err("the guard fired");
    assert_eq!(error.code, ErrorCode::WallGuard);
    assert_eq!(error.exit, 3);
    assert_eq!(error.stop.map(|s| s.reason), Some(StopReason::WallGuard));
    assert!(error.message.contains("wall guard"), "{}", error.message);
    assert_eq!(error.remedy, WALL_GUARD_REMEDY);
    assert!(error.remedy.contains("--wall-guard"), "{}", error.remedy);
}

#[test]
fn an_emulated_budget_miss_is_still_budget_exhausted() {
    let f = Fixture::new("guard-budget");
    let mut h = handler(&f);
    let error = h
        .handle(&request(Op::Run(RunArgs {
            until: Some("[$C000] == $1234".into()),
            frames: Some(2),
            ..RunArgs::default()
        })))
        .outcome
        .expect_err("never held");
    assert_eq!(error.code, ErrorCode::BudgetExhausted);
    assert!(!error.remedy.contains("--wall-guard"), "{}", error.remedy);
}

#[test]
fn a_requests_wall_guard_replaces_the_default_for_that_request_only() {
    let f = Fixture::new("guard-raised");
    let mut h = handler(&f);
    assert!(run_frames(&mut h, 1).is_ok(), "past the boot run");
    h.set_wall_guard(Duration::ZERO);
    let raised = h.handle(&guarded_run(3, 60));
    assert!(raised.is_ok(), "{:?}", raised.error());
    let next = run_frames(&mut h, 3)
        .outcome
        .expect_err("the default again");
    assert_eq!(next.code, ErrorCode::WallGuard);
}

/// A handler whose caller supplies the interrupt and starts no watchdog.
fn flagged(f: &Fixture) -> (super::Handler, InterruptFlag) {
    let interrupt = InterruptFlag::new();
    let h = super::Handler::new(f.host(), f.bundle.clone(), Box::new(interrupt.clone()))
        .with_defaults(Some("demo".into()), None);
    (h, interrupt)
}

#[test]
fn with_no_watchdog_short_requests_are_answered_and_a_zero_allowance_still_stops() {
    let f = Fixture::new("guard-flag");
    let (mut h, _interrupt) = flagged(&f);
    for _ in 0..3 {
        let response = run_frames(&mut h, 2);
        assert!(response.is_ok(), "{:?}", response.error());
    }
    h.set_wall_guard(Duration::ZERO);
    let error = run_frames(&mut h, 2).outcome.expect_err("stopped");
    assert_eq!(error.code, ErrorCode::WallGuard);
}

/// Sets `interrupt`'s flag from another thread once `after` has passed, as
/// a caller stopping the request in flight does.
pub(super) fn stop_after(
    interrupt: &InterruptFlag,
    after: Duration,
) -> std::thread::JoinHandle<()> {
    let flag = interrupt.flag();
    std::thread::spawn(move || {
        std::thread::sleep(after);
        flag.store(true, std::sync::atomic::Ordering::Relaxed);
    })
}

/// The caller ends a request in flight by setting its flag, from another
/// thread here. The request answers `interrupted`, not `wall_guard`, says
/// the caller stopped it, and its stop is the caller's (`user_break`); the
/// next request runs.
#[test]
fn the_caller_setting_its_flag_ends_the_request_in_flight_as_the_callers_stop() {
    let f = Fixture::new("guard-caller");
    let (mut h, interrupt) = flagged(&f);
    assert!(run_frames(&mut h, 1).is_ok(), "past the boot run");
    let setter = stop_after(&interrupt, Duration::from_millis(50));
    let error = run_frames(&mut h, 1_000_000)
        .outcome
        .expect_err("the caller stopped it");
    setter.join().unwrap();
    assert_eq!(error.code, ErrorCode::Interrupted);
    assert_eq!(error.stop.map(|s| s.reason), Some(StopReason::UserBreak));
    assert!(error.message.contains("the caller"), "{}", error.message);
    assert!(!error.message.contains("wall guard"), "{}", error.message);
    assert!(!error.remedy.contains("--wall-guard"), "{}", error.remedy);
    let next = run_frames(&mut h, 2);
    assert!(next.is_ok(), "{:?}", next.error());
    // A zero allowance on the same handler is still the guard's stop.
    h.set_wall_guard(Duration::ZERO);
    let guard = run_frames(&mut h, 2).outcome.expect_err("stopped");
    assert_eq!(guard.stop.map(|s| s.reason), Some(StopReason::WallGuard));
    assert!(guard.message.contains("wall guard"), "{}", guard.message);
    assert_eq!(guard.remedy, WALL_GUARD_REMEDY);
}

/// The request's own allowance arms the guard, and the message names it.
/// A million frames take minutes, so the 1 s guard ends it.
#[test]
fn a_requests_wall_guard_is_the_allowance_that_fires() {
    let f = Fixture::new("guard-fires");
    let mut h = handler(&f);
    assert!(run_frames(&mut h, 1).is_ok(), "past the boot run");
    let error = h
        .handle(&guarded_run(1_000_000, 1))
        .outcome
        .expect_err("the 1 s guard fired");
    assert_eq!(error.code, ErrorCode::WallGuard);
    assert!(error.message.contains("1s wall guard"), "{}", error.message);
}
