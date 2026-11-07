//! A run with a target (`--to`, `--until`) that ends without reaching it
//! (design §11, the stop-reason table of 2026-11-07).

use super::*;
use crate::fixture::{Fixture, HALT, ILLEGAL, STOP};
use crate::handle::test_support::{handler, request};
use gbcli_protocol::{BreakArgs, Op, Rev, WatchAccess, WatchArgs};

fn run(h: &mut Handler, args: RunArgs) -> gbcli_protocol::Response {
    h.handle(&request(Op::Run(args)))
}

fn to(at: &str) -> RunArgs {
    RunArgs {
        to: Some(at.into()),
        frames: Some(30),
        ..RunArgs::default()
    }
}

fn until(text: &str) -> RunArgs {
    RunArgs {
        until: Some(text.into()),
        frames: Some(30),
        ..RunArgs::default()
    }
}

#[test]
fn a_target_a_locked_cpu_never_reaches_fails_naming_the_stop() {
    let cases: [(&str, &[u8], StopReason, &str); 2] = [
        (
            "illegal",
            ILLEGAL,
            StopReason::IllegalOpcode { opcode: 0xD3 },
            "illegal_opcode",
        ),
        ("halt", HALT, StopReason::HaltForever, "halt_forever"),
    ];
    for (name, rom, reason, wire) in cases {
        for (kind, args) in [("to", to("$0000")), ("until", until("pc == $0000"))] {
            let f = Fixture::with_rom(&format!("unreached-{name}-{kind}"), rom);
            let response = run(&mut handler(&f), args);
            assert_eq!(
                response.rev,
                Some(Rev(1)),
                "{name} {kind}: the state is saved"
            );
            let error = response
                .outcome
                .expect_err(&format!("{name} {kind}: the target was never reached"));
            assert_eq!(error.code, ErrorCode::TargetNotReached, "{name} {kind}");
            assert_eq!(error.exit, 3, "{name} {kind}");
            assert_eq!(error.stop.map(|s| s.reason), Some(reason.clone()));
            assert!(
                error.message.contains(wire) && error.remedy.contains(wire),
                "{name} {kind}: {} / {}",
                error.message,
                error.remedy
            );
            assert!(
                error.message.contains("$0000"),
                "{name} {kind}: names the target: {}",
                error.message
            );
        }
    }
}

/// Time passes in STOP during a run (design §2.2, leaf 34Nx7n), so a target
/// short of a STOP nothing wakes ends on the budget, with the STOP warning:
/// already exit 3, never `stopped`.
#[test]
fn a_target_short_of_a_stop_nothing_wakes_spends_its_budget_and_warns() {
    let f = Fixture::with_rom("unreached-stop", STOP);
    let response = run(&mut handler(&f), to("$0000"));
    let warnings = response.warnings.clone();
    let error = response.outcome.expect_err("never reached");
    assert_eq!(error.code, ErrorCode::BudgetExhausted);
    assert!(warnings.iter().any(|w| w.contains("STOP")), "{warnings:?}");
}

#[test]
fn a_run_without_a_target_that_locks_up_is_answered_by_its_stop() {
    // The contrast to the test above: with no target, the stop is the answer.
    let f = Fixture::with_rom("locked-untargeted", ILLEGAL);
    let response = run(
        &mut handler(&f),
        RunArgs {
            frames: Some(30),
            ..RunArgs::default()
        },
    );
    let stop = response.outcome.expect("answered").stop.unwrap();
    assert_eq!(stop.reason, StopReason::IllegalOpcode { opcode: 0xD3 });
    // And a target the CPU locks up short of is not.
    let response = run(&mut handler(&f), to("$0000"));
    assert!(response.outcome.is_err(), "the target was never reached");
}

#[test]
fn a_breakpoint_or_watch_before_the_target_is_the_callers_stop_with_a_warning() {
    let f = Fixture::new("break-before-target");
    let mut h = handler(&f);
    let set = h.handle(&request(Op::Break(BreakArgs {
        at: "$0170".into(),
        condition: None,
    })));
    assert!(set.is_ok(), "{:?}", set.error());
    let response = run(&mut h, to("$0000"));
    let warnings = response.warnings.clone();
    let stop = response
        .outcome
        .expect("the caller's own stop")
        .stop
        .unwrap();
    assert!(matches!(stop.reason, StopReason::Breakpoint { .. }));
    assert!(
        warnings
            .iter()
            .any(|w| w.contains("breakpoint #1") && w.contains("never reached $0000")),
        "{warnings:?}"
    );

    let f = Fixture::with_sym("watch-before-target");
    let mut h = handler(&f);
    let set = h.handle(&request(Op::Watch(WatchArgs {
        access: WatchAccess::Write,
        at: "wFrame".into(),
        condition: None,
        once: false,
    })));
    assert!(set.is_ok(), "{:?}", set.error());
    let response = run(&mut h, until("pc == $0000"));
    let warnings = response.warnings.clone();
    let stop = response
        .outcome
        .expect("the caller's own stop")
        .stop
        .unwrap();
    assert!(matches!(stop.reason, StopReason::Watchpoint { .. }));
    assert!(
        warnings
            .iter()
            .any(|w| w.contains("watch #1") && w.contains("`pc == $0000` never held")),
        "{warnings:?}"
    );
}
