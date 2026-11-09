//! `run --from boot` restarts a session on the ROM on disk: the head is
//! skipped (it stays in history, and named checkpoints stay named), and
//! breakpoints and watches set by a symbol follow it into the new build.

use gbcli_protocol::{
    BreakArgs, CheckpointArgs, ErrorCode, Op, PowerCycleArgs, Response, RunArgs, Start, StopReason,
    WatchAccess, WatchArgs,
};

use super::Handler;
use super::test_support::{handler, request, run_frames};
use crate::fixture::{Fixture, V2, V2_SYM, V3, V3_SYM, V4, V4_SYM};

fn code(response: &Response) -> Option<ErrorCode> {
    response.outcome.as_ref().err().map(|e| e.code)
}

fn restart(h: &mut Handler, frames: u64) -> Response {
    h.handle(&request(Op::Run(RunArgs {
        from: Some(Start::Boot),
        frames: Some(frames),
        ..RunArgs::default()
    })))
}

fn break_at(h: &mut Handler, at: &str) {
    let made = h.handle(&request(Op::Break(BreakArgs {
        at: at.into(),
        condition: None,
    })));
    assert!(made.is_ok(), "{:?}", made.error());
}

fn watch_writes(h: &mut Handler, at: &str) {
    let made = h.handle(&request(Op::Watch(WatchArgs {
        access: WatchAccess::Write,
        at: at.into(),
        condition: None,
        once: false,
    })));
    assert!(made.is_ok(), "{:?}", made.error());
}

#[test]
fn run_from_boot_restarts_a_session_whose_rebuild_moved_ram_symbols() {
    let f = Fixture::with_sym("restart-ram");
    let mut h = handler(&f);
    break_at(&mut h, "Work.done");
    watch_writes(&mut h, "wNever");
    assert!(run_frames(&mut h, 1).is_ok());
    let made = h.handle(&request(Op::Checkpoint(CheckpointArgs {
        name: "a".into(),
    })));
    assert!(made.is_ok(), "{:?}", made.error());
    // wFrame, wSpeed and wNever all move up a byte.
    f.rebuild(V4, V4_SYM);

    let refused = run_frames(&mut handler(&f), 1);
    assert_eq!(code(&refused), Some(ErrorCode::RomChanged), "{refused:?}");

    let mut h = handler(&f);
    let restarted = restart(&mut h, 1);
    assert!(restarted.is_ok(), "{:?}", restarted.error());
    // Work.done (which didn't move) runs in the first frame after the boot
    // handoff, so the kept breakpoint stops it there.
    let stop = restarted.outcome.unwrap().stop.unwrap();
    assert!(
        matches!(stop.reason, StopReason::Breakpoint { .. }),
        "{stop:?}"
    );
    assert_eq!((stop.pc, stop.time.frame), (0x0170, 0), "{stop:?}");

    let meta = f.bundle.session("demo").unwrap().read_meta().unwrap();
    assert_eq!(meta.debugger.breakpoints.len(), 1, "{:?}", meta.debugger);
    assert_eq!(meta.debugger.breakpoints[0].at.addr, 0x0170);
    assert_eq!(meta.debugger.watches.len(), 1, "{:?}", meta.debugger);
    assert_eq!(meta.debugger.watches[0].at.addr, 0xC004, "wNever moved");
    assert!(
        restarted
            .warnings
            .iter()
            .any(|w| w.contains("wNever") && w.contains("$C004")),
        "the move is reported: {:?}",
        restarted.warnings
    );

    // The new head is on the new ROM: a plain run continues from it.
    assert!(run_frames(&mut handler(&f), 1).is_ok());
    // The checkpoint stays named, and still belongs to the old build.
    let store = f.bundle.session("demo").unwrap();
    assert_eq!(store.checkpoint_names().unwrap(), vec!["a".to_string()]);
    let at_a = handler(&f).handle(&request(Op::Run(RunArgs {
        at: Some("a".into()),
        frames: Some(1),
        ..RunArgs::default()
    })));
    assert_eq!(code(&at_a), Some(ErrorCode::RomChanged), "{at_a:?}");
}

#[test]
fn a_breakpoint_set_by_a_symbol_stops_where_the_rebuild_moved_it() {
    let f = Fixture::with_sym("restart-break");
    let mut h = handler(&f);
    break_at(&mut h, "Work.done");
    assert!(run_frames(&mut h, 1).is_ok());
    // Work.done moves from $0170 to $0172.
    f.rebuild(V2, V2_SYM);

    let restarted = restart(&mut handler(&f), 10);
    let stop = restarted.outcome.expect("restarted").stop.unwrap();
    assert!(
        matches!(stop.reason, StopReason::Breakpoint { .. }),
        "{stop:?}"
    );
    assert_eq!(stop.pc, 0x0172);
}

#[test]
fn a_breakpoint_whose_symbol_is_gone_is_kept_at_its_address_with_a_warning() {
    let f = Fixture::with_sym("restart-gone");
    f.rebuild(V3, V3_SYM);
    let mut h = handler(&f);
    break_at(&mut h, "Extra");
    assert!(run_frames(&mut h, 1).is_ok());
    f.rebuild(V2, V2_SYM);

    let restarted = restart(&mut handler(&f), 1);
    assert!(restarted.is_ok(), "{:?}", restarted.error());
    assert!(
        restarted
            .warnings
            .iter()
            .any(|w| w.contains("#1") && w.contains("Extra") && w.contains("gbcli delete 1")),
        "{:?}",
        restarted.warnings
    );
    let meta = f.bundle.session("demo").unwrap().read_meta().unwrap();
    assert_eq!(meta.debugger.breakpoints[0].at.addr, 0x0171, "kept");
}

#[test]
fn an_explicit_from_boot_skips_the_head_over_an_unchanged_rom() {
    let f = Fixture::with_sym("restart-same");
    let mut h = handler(&f);
    assert!(run_frames(&mut h, 5).is_ok());
    // The same handler: its warm head must not be reused either.
    let restarted = restart(&mut h, 1);
    let stop = restarted.outcome.expect("restarted").stop.unwrap();
    assert_eq!(stop.time.frame, 1, "from the boot handoff, not the head");
    assert!(
        restarted.warnings.is_empty(),
        "nothing moved: {:?}",
        restarted.warnings
    );
}

#[test]
fn a_power_cycle_across_a_rebuild_moves_breakpoints_with_their_symbols() {
    let f = Fixture::with_sym("restart-cycle");
    let mut h = handler(&f);
    break_at(&mut h, "Work.done");
    assert!(run_frames(&mut h, 1).is_ok());
    f.rebuild(V2, V2_SYM);

    let cycled = handler(&f).handle(&request(Op::PowerCycle(PowerCycleArgs::default())));
    assert!(cycled.is_ok(), "{:?}", cycled.error());
    assert!(
        cycled.warnings.iter().any(|w| w.contains("$0172")),
        "{:?}",
        cycled.warnings
    );
    let meta = f.bundle.session("demo").unwrap().read_meta().unwrap();
    assert_eq!(meta.debugger.breakpoints[0].at.addr, 0x0172);
}
