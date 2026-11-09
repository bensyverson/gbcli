//! A run or step that fails after it ran keeps what it saw: the ignored-write
//! and LCD-off warnings, and the `ignored_writes` rows on the error, as the
//! flash notes ride on it (review rev-hw, F1).

use gbcli_core::IgnoredWriteKind;
use gbcli_protocol::{BreakArgs, ErrorBody, Op, Response, RunArgs, StepArgs, StepKind, StopReason};

use crate::fixture::Fixture;
use crate::handle::test_support::{handler, request};

/// `help traps`' demo: one VRAM write in mode 3 on line 10, then an idle loop.
const VRAM_MODE3: &[u8] = include_bytes!("../../../../gbcli/tests/traps/vram_mode3.gb");
const VRAM_MODE3_SYM: &[u8] = include_bytes!("../../../../gbcli/tests/traps/vram_mode3.sym");

fn failed(response: Response) -> (ErrorBody, Vec<String>) {
    let warnings = response.warnings.clone();
    (response.outcome.expect_err("the request fails"), warnings)
}

/// The dropped write's warning and its one row, on the error.
fn assert_kept(error: &ErrorBody, warnings: &[String]) {
    assert!(error.stop.is_some(), "the machine moved: {error:?}");
    assert!(
        warnings
            .iter()
            .any(|w| w.starts_with("1 VRAM write ignored in mode 3")),
        "{warnings:?}"
    );
    let rows = error.ignored_writes.as_ref().expect("rows on the error");
    assert_eq!(rows.len(), 1, "{rows:?}");
    assert_eq!(rows[0].kind, IgnoredWriteKind::Vram);
    assert_eq!((rows[0].addr, rows[0].line), (0x8000, 10));
}

#[test]
fn a_run_short_of_its_target_keeps_the_rows() {
    let f = Fixture::with_rom_sym("failed-short", VRAM_MODE3, VRAM_MODE3_SYM);
    let mut h = handler(&f);
    let (error, warnings) = failed(h.handle(&request(Op::Run(RunArgs {
        frames: Some(2),
        to: Some("$7FF0".into()),
        ..RunArgs::default()
    }))));
    assert_kept(&error, &warnings);
}

#[test]
fn a_predicate_error_after_the_drop_keeps_the_warning_and_rows() {
    let f = Fixture::with_rom_sym("failed-until", VRAM_MODE3, VRAM_MODE3_SYM);
    let mut h = handler(&f);
    let (error, warnings) = failed(h.handle(&request(Op::Run(RunArgs {
        frames: Some(2),
        until: Some("1 / (ly - 20) == 99".into()),
        ..RunArgs::default()
    }))));
    assert_kept(&error, &warnings);
}

/// A step's condition error after the drop: a breakpoint at the idle loop
/// whose condition can't be evaluated, reached by `step out`.
#[test]
fn a_steps_condition_error_after_the_drop_keeps_the_warning_and_rows() {
    let f = Fixture::with_rom_sym("failed-step", VRAM_MODE3, VRAM_MODE3_SYM);
    let mut h = handler(&f);
    let set = h.handle(&request(Op::Break(BreakArgs {
        at: "Idle".into(),
        condition: Some("1 / (a - a) == 1".into()),
    })));
    assert!(set.is_ok(), "{:?}", set.error());
    let (error, warnings) = failed(h.handle(&request(Op::Step(StepArgs {
        unit: StepKind::Out,
    }))));
    let reason = error.stop.as_ref().map(|s| s.reason.clone());
    assert_eq!(reason, Some(StopReason::ConditionError { id: 1 }));
    assert_kept(&error, &warnings);
}
