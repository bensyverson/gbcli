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
    let split = RefCell::new(FrameSplit::default());
    let frames = RefCell::new(Vec::new());
    let cut = |_: &Machine, _: u64| frames.borrow_mut().push(split.borrow_mut().cut());
    let mut opts = RunOptions::new(Budget::frames(4));
    opts.on_event = Some(EventTap(&split));
    opts.on_frame = Some(FrameSink(&cut));
    run.run(&opts).unwrap();
    assert_eq!(frames.into_inner(), expected);
}
