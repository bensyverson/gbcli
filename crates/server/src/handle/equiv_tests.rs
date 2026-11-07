//! `equiv` through the handler (design §11). Side B is V1 with a patch: the
//! `inc [hl]` at Work+8 ($016B) that counts wFrame made a `nop`, so wFrame
//! stops counting in B's first pass.

use std::time::Duration;

use gbcli_engine::equiv::{EquivError, StartField, StartMismatch};
use gbcli_protocol::{
    DifferenceMade, EquivArgs, EquivSide, ErrorCode, Op, RamRangeArg, ReadArgs, Response,
    RestoreArgs, WriteArg, WriteKind,
};

use super::Handler;
use super::run::equiv_failure;
use super::test_support::{handler, request, run_frames};
use crate::fixture::Fixture;

fn equiv(h: &mut Handler, args: EquivArgs) -> Response {
    h.handle(&request(Op::Equiv(Box::new(args))))
}

fn frame_counter(frames: u64, patched: bool) -> EquivArgs {
    let writes = if patched {
        vec![WriteArg {
            kind: WriteKind::Patch,
            target: "$016B".into(),
            bytes: vec![0x00],
        }]
    } else {
        Vec::new()
    };
    EquivArgs {
        b: EquivSide {
            writes,
            ..EquivSide::default()
        },
        frames,
        ram: vec![RamRangeArg {
            at: "wFrame".into(),
            len: 2,
        }],
        ..EquivArgs::default()
    }
}

fn read(h: &mut Handler, expr: &str) -> i64 {
    h.handle(&request(Op::Read(ReadArgs {
        reads: vec![expr.into()],
        since: None,
        allow_uninit: false,
    })))
    .outcome
    .expect("read")
    .reads
    .expect("values")[0]
        .value
}

#[test]
fn the_same_build_is_the_same_for_every_frame() {
    let f = Fixture::with_sym("equiv-same");
    let made = equiv(&mut handler(&f), frame_counter(20, false))
        .outcome
        .expect("the same")
        .equiv
        .expect("a verdict");
    assert_eq!(made.frames, 20);
    assert_eq!(made.divergence, None);
}

#[test]
fn a_divergence_exits_1_naming_the_frame_and_the_byte() {
    let f = Fixture::with_sym("equiv-diverged");
    let error = equiv(&mut handler(&f), frame_counter(20, true))
        .outcome
        .expect_err("diverged");
    assert_eq!(error.code, ErrorCode::PredicateFalse);
    assert_eq!(error.exit, 1);
    assert!(error.message.contains("frame 1"), "{}", error.message);
    let made = error.result.and_then(|r| r.equiv).expect("the verdict");
    let d = made.divergence.expect("a divergence");
    assert_eq!(d.frame, 1);
    let ram = d
        .differences
        .iter()
        .find(|x| matches!(x, DifferenceMade::Ram { .. }))
        .expect("wFrame differs");
    let DifferenceMade::Ram { range, offset, .. } = ram else {
        unreachable!()
    };
    assert_eq!((range.as_str(), *offset), ("wFrame", 0));
    assert_eq!(
        (d.a.checkpoint.name.as_str(), d.b.checkpoint.name.as_str()),
        ("equiv-a", "equiv-b")
    );
}

#[test]
fn both_sides_of_a_divergence_restore_and_the_head_never_moves() {
    let f = Fixture::with_sym("equiv-restore");
    let mut h = handler(&f);
    let ran = run_frames(&mut h, 3);
    let head_frame = read(&mut h, "u16[wFrame]");
    let response = equiv(&mut h, frame_counter(5, true));
    assert!(response.outcome.is_err(), "diverged");
    assert_eq!(response.rev, ran.rev, "the head never moves");
    assert_eq!(read(&mut h, "u16[wFrame]"), head_frame);

    let restore = |h: &mut Handler, name: &str| {
        h.handle(&request(Op::Restore(RestoreArgs { name: name.into() })))
            .outcome
            .unwrap_or_else(|e| panic!("{name}: {e:?}"))
    };
    restore(&mut h, "equiv-b");
    let b = (read(&mut h, "[wFrame]"), read(&mut h, "u16[wFrame]"));
    restore(&mut h, "equiv-a");
    let a = read(&mut h, "u16[wFrame]");
    // B's patched Work never increments the low byte (its fallthrough still
    // bumps the high one), so the two saved states differ there.
    assert_eq!(b.0, 0, "B's low byte never counts");
    assert_ne!(a, b.1, "the sides were saved as they diverged");
}

#[test]
fn a_range_that_does_not_resolve_is_the_address_error() {
    let f = Fixture::with_sym("equiv-range");
    let mut args = frame_counter(5, false);
    args.ram[0].at = "wNowhere".into();
    let error = equiv(&mut handler(&f), args).outcome.expect_err("refused");
    assert_eq!(error.code, ErrorCode::UnknownSymbol);
    assert!(error.message.contains("wNowhere"), "{}", error.message);
}

#[test]
fn a_start_mismatch_is_bad_request_naming_the_field() {
    let failure = equiv_failure(&EquivError::Start(StartMismatch {
        field: StartField::Ticks,
        a: "100".into(),
        b: "200".into(),
    }));
    assert_eq!(failure.body.code, ErrorCode::BadRequest);
    assert!(
        failure.body.message.contains("Ticks"),
        "{}",
        failure.body.message
    );
    assert!(
        failure.body.remedy.contains("--from boot"),
        "{}",
        failure.body.remedy
    );
}

#[test]
fn the_wall_guard_ends_the_check() {
    let f = Fixture::with_sym("equiv-guard");
    let mut h = handler(&f);
    assert!(run_frames(&mut h, 1).is_ok(), "past the boot run");
    h.set_wall_guard(Duration::ZERO);
    let error = equiv(&mut h, frame_counter(600, false))
        .outcome
        .expect_err("the guard fired");
    assert_eq!(error.code, ErrorCode::BudgetExhausted);
    assert!(error.message.contains("wall guard"), "{}", error.message);
}

#[test]
fn zero_frames_is_refused() {
    let f = Fixture::with_sym("equiv-zero");
    let error = equiv(&mut handler(&f), frame_counter(0, false))
        .outcome
        .expect_err("refused");
    assert_eq!(error.code, ErrorCode::BadRequest);
}
