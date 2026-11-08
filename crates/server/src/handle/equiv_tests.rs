//! `equiv` through the handler (design §11). Side B is V1 with a patch: the
//! `inc [hl]` at Work+8 ($016B) that counts wFrame made a `nop`, so wFrame
//! stops counting in B's first pass.

use std::time::Duration;

use gbcli_engine::equiv::{EquivError, StartField, StartMismatch};
use gbcli_protocol::{
    DifferenceMade, EquivArgs, EquivSide, ErrorCode, Op, RamRangeArg, ReadArgs, Response,
    RestoreArgs, SideName, WriteArg, WriteKind,
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
        .expect("a value")
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
    // A patched side runs the session's ROM, so restoring it works (the
    // next test restores both).
    assert!(
        error.remedy.contains("`gbcli restore equiv-b`"),
        "{}",
        error.remedy
    );
}

#[test]
fn a_divergence_on_another_rom_doesnt_send_the_reader_to_restore_it() {
    // Side B is a build of its own: V1 with Work's `inc [hl]` made a `nop` in
    // the file. This session restores onto its own ROM, so equiv-b would run
    // B's state on A's code, or be refused as rom_changed.
    let f = Fixture::with_sym("equiv-rom-b");
    let mut rom = crate::fixture::V1.to_vec();
    rom[0x016B] = 0x00;
    std::fs::write(f.dir.join("b.gb"), rom).expect("rom b");
    std::fs::write(f.dir.join("b.sym"), crate::fixture::V1_SYM).expect("sym b");
    let mut args = frame_counter(20, false);
    args.b.rom = Some("b.gb".into());
    let error = equiv(&mut handler(&f), args).outcome.expect_err("diverged");
    assert_eq!(error.code, ErrorCode::PredicateFalse);
    assert!(
        !error.remedy.contains("restore equiv-b"),
        "{}",
        error.remedy
    );
    assert!(
        error.remedy.contains("`gbcli restore equiv-a`"),
        "{}",
        error.remedy
    );
    assert!(error.remedy.contains("b.gb"), "{}", error.remedy);
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
    assert_eq!(error.remedy, "compare fewer --frames per call");
}

#[test]
fn zero_frames_is_refused() {
    let f = Fixture::with_sym("equiv-zero");
    let error = equiv(&mut handler(&f), frame_counter(0, false))
        .outcome
        .expect_err("refused");
    assert_eq!(error.code, ErrorCode::BadRequest);
}

/// ROM_B with no sibling .sym has no symbols of its own: the error says
/// side B has none and how to give it one.
#[test]
fn a_side_b_with_no_sym_names_the_flag_that_gives_it_one() {
    let f = Fixture::with_sym("equiv-b-nosym");
    std::fs::write(f.dir.join("v2.gb"), crate::fixture::V1).expect("rom b");
    let mut args = frame_counter(5, false);
    args.b.rom = Some("v2.gb".into());
    let error = equiv(&mut handler(&f), args).outcome.expect_err("refused");
    assert_eq!(error.code, ErrorCode::UnknownSymbol);
    assert!(error.message.contains("side B"), "{}", error.message);
    assert!(error.message.contains("no .sym"), "{}", error.message);
    assert!(error.remedy.contains("--sym-b FILE"), "{}", error.remedy);
}

/// `equiv` over the late-init program (`equiv_late.inc`), comparing wX.
fn late(b: EquivSide) -> EquivArgs {
    EquivArgs {
        b,
        frames: 20,
        ram: vec![RamRangeArg {
            at: "wX".into(),
            len: 1,
        }],
        ..EquivArgs::default()
    }
}

/// The first divergence's wX difference.
fn never_written(error: gbcli_protocol::ErrorBody) -> (u64, Vec<SideName>) {
    let d = error
        .result
        .and_then(|r| r.equiv)
        .and_then(|e| e.divergence)
        .expect("a divergence");
    let [DifferenceMade::Ram { never_written, .. }] = d.differences.as_slice() else {
        panic!("expected wX to differ: {:?}", d.differences);
    };
    (d.frame, never_written.clone())
}

/// Side B is a build that moved wX, which both programs first write after
/// the fourth vblank: at frame 1 each side reads its own never-written byte,
/// at its own address. A checkpoint carries side A's RAM layout (and the
/// rebuilt-ROM check refuses it on B), so the remedy is to initialize earlier.
#[test]
fn a_moved_byte_neither_side_has_written_says_so_and_to_initialize_it_earlier() {
    use crate::fixture::{LATE_A, LATE_A_SYM, LATE_B, LATE_B_SYM};
    let f = Fixture::with_rom_sym("equiv-late", LATE_A, LATE_A_SYM);
    std::fs::write(f.dir.join("b.gb"), LATE_B).expect("rom b");
    std::fs::write(f.dir.join("b.sym"), LATE_B_SYM).expect("sym b");
    let args = late(EquivSide {
        rom: Some("b.gb".into()),
        ..EquivSide::default()
    });
    let error = equiv(&mut handler(&f), args).outcome.expect_err("diverged");
    assert_eq!(error.code, ErrorCode::PredicateFalse);
    assert!(
        error.message.contains("never written on both sides"),
        "{}",
        error.message
    );
    let remedy = error.remedy.clone();
    assert!(remedy.contains("power-on RAM is random"), "{remedy}");
    assert!(
        remedy.contains("initialize it before the first vblank"),
        "{remedy}"
    );
    assert!(!remedy.contains("restore"), "{remedy}");
    assert_eq!(never_written(error), (1, vec![SideName::A, SideName::B]));
}

/// Side B is the session's ROM with wX's first write (`ld [wX], a` at
/// $0164) patched out: wX stays where it was, so a checkpoint saved after
/// the program's init starts both sides past it.
#[test]
fn a_byte_one_side_never_wrote_names_that_side_and_a_checkpoint_after_init() {
    use crate::fixture::{LATE_A, LATE_A_SYM};
    let f = Fixture::with_rom_sym("equiv-late-patch", LATE_A, LATE_A_SYM);
    let args = late(EquivSide {
        writes: vec![WriteArg {
            kind: WriteKind::Patch,
            target: "$0164".into(),
            bytes: vec![0x00; 3],
        }],
        ..EquivSide::default()
    });
    let error = equiv(&mut handler(&f), args).outcome.expect_err("diverged");
    assert!(
        error.message.contains("never written on side B"),
        "{}",
        error.message
    );
    // Only B never wrote it, so B may be missing the write: the remedy says
    // so rather than only sending the reader past the init.
    let remedy = error.remedy.clone();
    assert!(remedy.contains("though side A wrote it"), "{remedy}");
    assert!(remedy.contains("power-on RAM"), "{remedy}");
    assert!(remedy.contains("--at a checkpoint"), "{remedy}");
    assert!(remedy.contains("missing the write"), "{remedy}");
    // wX is first written after the fourth vblank, which the program sees
    // in the fifth frame.
    assert_eq!(never_written(error), (5, vec![SideName::B]));
}
