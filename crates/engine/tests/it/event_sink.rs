//! The run loop's event sink: a run's execution events reach a caller's sink
//! while breakpoints, budgets and keys still apply (`run --profile`,
//! `run --frame-budget`).

use crate::common;

use std::cell::RefCell;

use common::*;
use gbcli_core::Budget;
use gbcli_engine::profile::{FrameSplit, Routines};
use gbcli_engine::{EventTap, FrameSink, Machine, RunOptions, Symbols};
use gbcli_protocol::{Location, StepKind, StopReason};

fn main_loop() -> Location {
    let sym = Symbols::parse(V1_SYM).unwrap();
    let s = sym.get("Main.loop").unwrap();
    Location {
        addr: s.addr,
        bank: None,
        symbol: Some("Main.loop".into()),
    }
}

#[test]
fn a_run_with_the_routines_sink_profiles_as_machine_profile_does() {
    let mut profiled = machine(V1, Some(V1_SYM));
    let expected = profiled.profile(Budget::frames(3)).unwrap();

    let mut run = machine(V1, Some(V1_SYM));
    let sink = RefCell::new(Routines::new());
    let mut opts = RunOptions::new(Budget::frames(3));
    opts.on_event = Some(EventTap(&sink));
    run.run(&opts).unwrap();
    let got = sink.into_inner().finish(run.symbols());
    assert_eq!(got, expected);
    assert!(got.total_t > 0);
}

#[test]
fn a_sink_does_not_change_where_a_run_to_a_breakpoint_stops() {
    let stop_of = |sunk: bool| {
        let mut m = machine(V1, Some(V1_SYM));
        let sink = RefCell::new(Routines::new());
        let mut opts = RunOptions::new(Budget::frames(30));
        if sunk {
            opts.on_event = Some(EventTap(&sink));
        }
        let stop = m.run_to(main_loop(), &opts).unwrap().stop;
        (stop, m.core().now(), sink.into_inner().finish(None).total_t)
    };
    let (plain, plain_now, _) = stop_of(false);
    let (sunk, sunk_now, charged) = stop_of(true);
    assert!(
        matches!(
            plain.reason,
            StopReason::StepDone {
                unit: StepKind::ToAddress
            }
        ),
        "{:?}",
        plain.reason
    );
    assert_eq!(plain, sunk);
    assert_eq!(plain_now, sunk_now);
    assert!(charged > 0, "the sink saw the run");
}

#[test]
fn a_split_cut_at_each_vblank_is_the_frame_budget() {
    let mut budgeted: Machine = machine(V1, Some(V1_SYM));
    let expected = budgeted.frame_budget(4).unwrap();

    let mut run = machine(V1, Some(V1_SYM));
    let split = RefCell::new(FrameSplit::starting(&run));
    let frames = RefCell::new(Vec::new());
    let cut = |m: &Machine, _: u64| frames.borrow_mut().extend(split.borrow_mut().cut(m));
    let mut opts = RunOptions::new(Budget::frames(4));
    opts.on_event = Some(EventTap(&split));
    opts.on_frame = Some(FrameSink(&cut));
    run.run(&opts).unwrap();
    let (last, rest) = split.into_inner().finish(&run);
    let mut frames = frames.into_inner();
    frames.extend(last);
    assert_eq!(frames, expected);
    assert_eq!(
        rest.active_t + rest.idle_t,
        0,
        "a run that stops at a vblank"
    );
}

/// The frames of `roms/busy_wait_off.gb` up to the end of its busy frame,
/// split as a run collects them, then `more` of the next frame; the last
/// whole frame as `finish` settles it.
fn busy_frame_settled(more: u64) -> gbcli_engine::profile::FrameBudget {
    let mut m = machine(include_bytes!("../roms/busy_wait_off.gb"), None);
    let split = RefCell::new(FrameSplit::starting(&m));
    let frames = RefCell::new(Vec::new());
    let cut = |m: &Machine, _: u64| frames.borrow_mut().extend(split.borrow_mut().cut(m));
    // Frames 0-3 halt; frame 4 polls LY for VBlank; frame 5 turns the LCD
    // off in its first instructions.
    let mut opts = RunOptions::new(Budget::frames(5));
    opts.on_event = Some(EventTap(&split));
    opts.on_frame = Some(FrameSink(&cut));
    m.run(&opts).unwrap();
    if more > 0 {
        opts.budget = Budget::ticks(more);
        m.run(&opts).unwrap();
    }
    let (last, rest) = split.into_inner().finish(&m);
    let frames = frames.into_inner();
    assert_eq!(frames.len(), 4, "{frames:?}");
    assert_eq!(rest.active_t + rest.idle_t > 0, more > 0, "{rest:?}");
    let last = last.expect("the busy frame");
    assert_eq!((last.frame, last.idle_t), (4, 0), "{last:?}");
    last
}

/// A frame's verdict waits on the next frame, so the run's last frame is
/// settled by what the run saw of the next: the start of a frame that turns
/// the LCD off before halting, or nothing at all (a stop at a vblank).
#[test]
fn the_last_frame_is_settled_by_what_the_run_saw_of_the_next() {
    let seen = busy_frame_settled(2_000);
    assert!(seen.lcd_off && !seen.lag, "{seen:?}");
    let unseen = busy_frame_settled(0);
    assert!(unseen.lag && !unseen.lcd_off, "{unseen:?}");
}
