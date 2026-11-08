//! Save → load → continue equals running straight through, at random
//! instruction-boundary stop points (property test, hand-rolled generator).

use crate::common;

use common::*;
use gbcli_core::{Budget, Core};
use gbcli_sameboy::SameBoy;

/// Runs `total` instructions straight, and again with a save/load round trip
/// (through a fresh instance) at each of `cuts`; the final states must match.
fn check(make: &dyn Fn() -> SameBoy, total: u64, cuts: &[u64]) {
    let mut straight = make();
    run(&mut straight, Budget::instructions(total));

    let mut interrupted = make();
    let mut done = 0;
    for &cut in cuts {
        run(&mut interrupted, Budget::instructions(cut - done));
        done = cut;
        let blob = interrupted.save();
        interrupted = make();
        interrupted.load(&blob).unwrap();
    }
    run(&mut interrupted, Budget::instructions(total - done));

    assert_eq!(interrupted.now(), straight.now(), "cuts {cuts:?}");
    assert!(
        interrupted.save() == straight.save(),
        "states differ; cuts {cuts:?}"
    );
}

fn random_cuts(rng: &mut Rng, total: u64, n: usize) -> Vec<u64> {
    let mut cuts: Vec<u64> = (0..n).map(|_| 1 + rng.below(total - 1)).collect();
    cuts.sort_unstable();
    cuts.dedup();
    cuts
}

#[test]
fn save_load_continue_equals_straight_through_on_the_busy_rom() {
    let mut rng = Rng::new(0x5A7E);
    let total = 200_000; // about 11 frames
    for _ in 0..8 {
        let cuts = random_cuts(&mut rng, total, 3);
        check(&|| quick_dmg(BUSY), total, &cuts);
    }
}

#[test]
fn save_load_continue_equals_straight_through_across_the_boot_rom() {
    let mut rng = Rng::new(0xB0B);
    let total = 1_600_000; // past the ~84-frame handover
    for _ in 0..3 {
        let cuts = random_cuts(&mut rng, total, 4);
        check(&|| dmg(BUSY, 5), total, &cuts);
    }
}

#[test]
fn save_load_continue_equals_straight_through_on_cpu_instrs() {
    let rom = cpu_instrs();
    let mut rng = Rng::new(0xC0DE);
    let total = 3_000_000;
    for _ in 0..3 {
        let cuts = random_cuts(&mut rng, total, 4);
        check(&|| quick_dmg(&rom), total, &cuts);
    }
}
