//! A breakpoint or watch set by a symbol follows it whenever the session runs
//! on a ROM other than the one it was resolved against, not only on a restart
//! (Ben's ruling, 2026-11-08): a compatible rebuild whose head still loads
//! moves it too. One set by an address never moves.

use gbcli_protocol::{
    BreakArgs, CheckpointArgs, Op, Response, RestoreArgs, RunArgs, Start, StopReason, WatchAccess,
    WatchArgs,
};

use super::Handler;
use super::test_support::{handler, request, run_frames};
use crate::fixture::{Fixture, V1, V1_SYM, V2, V2_SYM, V3, V3_SYM};

fn break_at(h: &mut Handler, at: &str) {
    let made = h.handle(&request(Op::Break(BreakArgs {
        at: at.into(),
        condition: None,
    })));
    assert!(made.is_ok(), "{:?}", made.error());
}

fn stop_pc(response: Response) -> (StopReason, u16) {
    let stop = response.outcome.expect("ran").stop.expect("a stop");
    (stop.reason, stop.pc)
}

/// A head parked at Main ($0150) by breakpoint #1, which no later run hits
/// again: nothing live there moves in V2 (only Main.loop and Work do), so the
/// head loads across that rebuild.
fn parked_at_main(h: &mut Handler) {
    break_at(h, "$0150");
    let (reason, pc) = stop_pc(run_frames(h, 10));
    assert!(
        matches!(reason, StopReason::Breakpoint { .. }),
        "{reason:?}"
    );
    assert_eq!(pc, 0x0150);
}

#[test]
fn a_label_breakpoint_fires_at_its_new_address_after_a_rebuild_whose_head_loads() {
    let f = Fixture::with_sym("relabel-break");
    let mut h = handler(&f);
    parked_at_main(&mut h);
    break_at(&mut h, "Work.done");
    // Work.done moves from $0170 to $0172; the head is still at Main.
    f.rebuild(V2, V2_SYM);

    let ran = run_frames(&mut handler(&f), 10);
    assert!(
        ran.warnings
            .iter()
            .any(|w| w.contains("#2") && w.contains("$0170 -> $0172")),
        "the move is a warning: {:?}",
        ran.warnings
    );
    let (reason, pc) = stop_pc(ran);
    assert!(
        matches!(reason, StopReason::Breakpoint { id: 2 }),
        "{reason:?}"
    );
    assert_eq!(pc, 0x0172, "fires where the new build puts Work.done");
    let meta = f.bundle.session("demo").unwrap().read_meta().unwrap();
    assert_eq!(meta.debugger.breakpoints[1].at.addr, 0x0172, "kept");
}

#[test]
fn a_label_watch_moves_with_its_symbol_after_a_rebuild_whose_head_loads() {
    let f = Fixture::with_sym("relabel-watch");
    let mut h = handler(&f);
    parked_at_main(&mut h);
    let made = h.handle(&request(Op::Watch(WatchArgs {
        access: WatchAccess::Write,
        at: "Work.done".into(),
        condition: None,
        once: false,
    })));
    assert!(made.is_ok(), "{:?}", made.error());
    f.rebuild(V2, V2_SYM);

    let ran = run_frames(&mut handler(&f), 1);
    assert!(ran.is_ok(), "{:?}", ran.error());
    assert!(
        ran.warnings
            .iter()
            .any(|w| w.starts_with("watch #2") && w.contains("$0172")),
        "{:?}",
        ran.warnings
    );
    let meta = f.bundle.session("demo").unwrap().read_meta().unwrap();
    assert_eq!(meta.debugger.watches[0].at.addr, 0x0172);
}

#[test]
fn a_raw_address_breakpoint_never_moves() {
    let f = Fixture::with_sym("relabel-raw");
    let mut h = handler(&f);
    parked_at_main(&mut h);
    // V1's Work.done, by address: the rebuild moves the label, not this.
    break_at(&mut h, "$0170");
    f.rebuild(V2, V2_SYM);

    let ran = run_frames(&mut handler(&f), 1);
    assert!(ran.is_ok(), "{:?}", ran.error());
    assert!(
        !ran.warnings.iter().any(|w| w.contains("breakpoint #2")),
        "{:?}",
        ran.warnings
    );
    let restarted = handler(&f).handle(&request(Op::Run(RunArgs {
        from: Some(Start::Boot),
        frames: Some(1),
        ..RunArgs::default()
    })));
    assert!(restarted.is_ok(), "{:?}", restarted.error());
    let meta = f.bundle.session("demo").unwrap().read_meta().unwrap();
    let at = &meta.debugger.breakpoints[1].at;
    assert_eq!((at.addr, at.symbol.as_deref()), (0x0170, None));
}

#[test]
fn restoring_a_checkpoint_across_a_rebuild_moves_label_breakpoints() {
    let f = Fixture::with_sym("relabel-restore");
    let mut h = handler(&f);
    parked_at_main(&mut h);
    let made = h.handle(&request(Op::Checkpoint(CheckpointArgs {
        name: "main".into(),
    })));
    assert!(made.is_ok(), "{:?}", made.error());
    break_at(&mut h, "Work.done");
    f.rebuild(V2, V2_SYM);

    // A moved label is no reason to refuse the state: the breakpoint follows.
    let restored = handler(&f).handle(&request(Op::Restore(RestoreArgs {
        name: "main".into(),
    })));
    assert!(restored.is_ok(), "{:?}", restored.error());
    assert!(
        restored.warnings.iter().any(|w| w.contains("$0172")),
        "{:?}",
        restored.warnings
    );
    let meta = f.bundle.session("demo").unwrap().read_meta().unwrap();
    assert_eq!(meta.debugger.breakpoints[1].at.addr, 0x0172);
}

#[test]
fn a_breakpoint_whose_symbol_a_rebuild_removed_keeps_its_address_and_the_head_loads() {
    let f = Fixture::with_sym("relabel-gone");
    f.rebuild(V3, V3_SYM);
    let mut h = handler(&f);
    parked_at_main(&mut h);
    break_at(&mut h, "Extra");
    // V1 has no Extra; nothing else moves.
    f.rebuild(V1, V1_SYM);

    let ran = run_frames(&mut handler(&f), 1);
    assert!(ran.is_ok(), "{:?}", ran.error());
    assert!(
        ran.warnings
            .iter()
            .any(|w| w.contains("#2") && w.contains("Extra") && w.contains("gbcli delete 2")),
        "{:?}",
        ran.warnings
    );
    let meta = f.bundle.session("demo").unwrap().read_meta().unwrap();
    assert_eq!(meta.debugger.breakpoints[1].at.addr, 0x0171, "kept");
}

#[test]
fn a_run_at_a_checkpoint_across_a_rebuild_fires_label_breakpoints_where_they_moved() {
    let f = Fixture::with_sym("relabel-at");
    let mut h = handler(&f);
    parked_at_main(&mut h);
    let made = h.handle(&request(Op::Checkpoint(CheckpointArgs {
        name: "main".into(),
    })));
    assert!(made.is_ok(), "{:?}", made.error());
    break_at(&mut h, "Work.done");
    f.rebuild(V2, V2_SYM);

    let ran = handler(&f).handle(&request(Op::Run(RunArgs {
        at: Some("main".into()),
        frames: Some(10),
        ..RunArgs::default()
    })));
    let (reason, pc) = stop_pc(ran);
    assert!(
        matches!(reason, StopReason::Breakpoint { id: 2 }),
        "{reason:?}"
    );
    assert_eq!(pc, 0x0172);
}

/// `undo` across a rebuild moves a label breakpoint and keeps the move.
#[test]
fn undo_across_a_rebuild_moves_a_label_breakpoint() {
    let f = Fixture::with_sym("relabel-undo");
    let mut h = handler(&f);
    parked_at_main(&mut h);
    // A later head, so undo has Main's to go back to.
    let ran = run_frames(&mut h, 1);
    assert!(ran.is_ok(), "{:?}", ran.error());
    break_at(&mut h, "Work.done");
    f.rebuild(V2, V2_SYM);

    let undone = handler(&f).handle(&request(Op::Undo));
    assert!(undone.is_ok(), "{:?}", undone.error());
    assert!(
        undone
            .warnings
            .iter()
            .any(|w| w.contains("#2") && w.contains("$0170 -> $0172")),
        "{:?}",
        undone.warnings
    );
    let meta = f.bundle.session("demo").unwrap().read_meta().unwrap();
    assert_eq!(meta.debugger.breakpoints[1].at.addr, 0x0172);
}
