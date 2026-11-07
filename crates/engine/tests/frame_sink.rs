//! The run loop's frame sink: called at every vblank a run passes, after the
//! key events due there, so a caller can collect frames (filmstrips).

mod common;

use std::cell::RefCell;

use common::*;
use gbcli_core::Budget;
use gbcli_engine::{FrameSink, RunOptions};

#[test]
fn the_sink_sees_every_vblank_the_run_passes_with_its_frame_count() {
    let mut m = machine(V1, Some(V1_SYM));
    let seen = RefCell::new(Vec::new());
    let record = |m: &gbcli_engine::Machine, frames: u64| {
        let shown = m.core().last_complete_frame().map(|f| f.tag.number);
        seen.borrow_mut().push((frames, shown));
    };
    let mut opts = RunOptions::new(Budget::frames(5));
    opts.on_frame = Some(FrameSink(&record));
    let outcome = m.run(&opts).unwrap();
    let seen = seen.into_inner();
    assert_eq!(
        seen.iter().map(|s| s.0).collect::<Vec<_>>(),
        [1, 2, 3, 4, 5]
    );
    // Each call sees the frame that vblank completed, one apart.
    let shown: Vec<u64> = seen.iter().map(|s| s.1.unwrap()).collect();
    assert!(shown.windows(2).all(|w| w[1] == w[0] + 1), "{shown:?}");
    assert_eq!(outcome.stop.budget.frames, 5);
}

#[test]
fn a_sink_does_not_change_where_the_run_stops() {
    let mut plain = machine(V1, Some(V1_SYM));
    let mut sunk = machine(V1, Some(V1_SYM));
    let until = "u16[wFrame] >= 3";
    let with = |m: &mut gbcli_engine::Machine, sink: bool| {
        let ignore = |_: &gbcli_engine::Machine, _: u64| {};
        let mut opts = RunOptions::new(Budget::frames(30));
        if sink {
            opts.on_frame = Some(FrameSink(&ignore));
        }
        m.run_until(until, &opts).unwrap().stop
    };
    assert_eq!(with(&mut plain, false), with(&mut sunk, true));
    assert_eq!(plain.core().now(), sunk.core().now());
}
