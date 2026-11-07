//! The warm machine against the files on disk: a long-running `serve` must
//! answer a rebuilt ROM exactly as a fresh process would (design §1, §6).

use gbcli_protocol::{ErrorCode, Op, Response, RunArgs, StepArgs, StepKind};

use super::test_support::{handler, request, run_frames};
use crate::fixture::{Fixture, V2, V2_SYM, V3, V3_SYM};

fn code(response: &Response) -> Option<ErrorCode> {
    response.outcome.as_ref().err().map(|e| e.code)
}

#[test]
fn a_rebuild_without_symbols_under_a_warm_machine_is_refused_as_a_cold_load_is() {
    let f = Fixture::new("warm-rebuild-nosym");
    let mut warm = handler(&f);
    assert!(run_frames(&mut warm, 1).is_ok());
    // Another routine appended: same live layout, but no `.sym` to check.
    std::fs::write(f.dir.join("demo.gb"), V3).unwrap();
    let cold = run_frames(&mut handler(&f), 1);
    assert_eq!(code(&cold), Some(ErrorCode::RomChanged));
    let again = run_frames(&mut warm, 1);
    assert_eq!(
        code(&again),
        code(&cold),
        "the warm machine kept the old ROM"
    );
}

#[test]
fn a_compatible_rebuild_under_a_warm_machine_warns_as_a_cold_load_does() {
    let f = Fixture::with_sym("warm-rebuild-warn");
    let mut warm = handler(&f);
    assert!(run_frames(&mut warm, 1).is_ok());
    f.rebuild(V3, V3_SYM);
    let again = run_frames(&mut warm, 1);
    assert!(again.is_ok(), "{:?}", again.error());
    assert!(
        again.warnings.iter().any(|w| w.contains("ROM")),
        "no rebuild warning: {:?}",
        again.warnings
    );
}

#[test]
fn a_run_refused_in_planning_keeps_no_machine_its_flag_loaded() {
    let f = Fixture::with_sym("warm-d1");
    let mut h = handler(&f);
    assert!(run_frames(&mut h, 1).is_ok());
    f.rebuild(V2, V2_SYM);
    let mut run = request(Op::Run(RunArgs {
        frames: Some(1),
        read: vec!["u16[noSuchSymbol]".into()],
        ..RunArgs::default()
    }));
    run.allow_rom_change = true;
    let forced = h.handle(&run);
    assert!(!forced.is_ok(), "the bad read should refuse the run");
    // Without the flag, the head over the changed ROM must be refused again.
    let step = h.handle(&request(Op::Step(StepArgs {
        unit: StepKind::Instruction,
    })));
    assert_eq!(code(&step), Some(ErrorCode::RomChanged), "{step:?}");
}

#[test]
fn an_unchanged_rom_keeps_the_machine_warm() {
    let f = Fixture::with_sym("warm-unchanged");
    let mut h = handler(&f);
    assert!(run_frames(&mut h, 1).is_ok());
    assert!(h.warm.is_some());
    assert!(run_frames(&mut h, 1).is_ok());
    assert!(h.warm.is_some(), "a clean head is kept warm");
}
