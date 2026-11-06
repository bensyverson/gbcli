//! Determinism on blargg's cpu_instrs (from the pinned corpus).

mod common;

use common::*;
use gbcli_core::{Budget, BudgetKind, Core, StopReason};

#[test]
fn two_instances_with_the_same_seed_rom_and_inputs_match_after_600_frames() {
    let rom = cpu_instrs();
    let mut a = dmg(&rom, 42);
    let mut b = dmg(&rom, 42);
    // Interleave the runs, so any shared global state would show.
    for _ in 0..6 {
        let sa = run(&mut a, Budget::frames(100));
        let sb = run(&mut b, Budget::frames(100));
        assert_eq!(sa, sb);
        assert_eq!(sa.reason, StopReason::Budget(BudgetKind::Frames));
    }
    assert_eq!(a.now(), b.now());
    assert!(a.save() == b.save(), "states differ after 600 frames");
}
