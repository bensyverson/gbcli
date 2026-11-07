//! Cost of the `--until` cadences: the write-only cadence derived from a
//! memory predicate against the per-instruction baseline, over the same ROM
//! and stopping at the same instruction.
//!
//! ```sh
//! cargo run --release -p gbcli-engine --example until_bench -- [iterations=10000]
//! ```
//!
//! Both predicates are the same proposition: `pc == pc` is always true, and
//! exists only to force the per-instruction baseline.

use std::time::Instant;

use gbcli_core::{BootRom, Budget, CoreConfig, Model, StepUnit};
use gbcli_engine::{Backend, Machine, Outcome, RunOptions};
use gbcli_protocol::{Cadence, StopReason};

const QUICK_BOOT: &[u8] = include_bytes!("../../sameboy/tests/roms/quick_boot.bin");
const V1: &[u8] = include_bytes!("../tests/roms/rebuild_v1.gb");

const DERIVED: &str = "u16[$C000] >= 60";
const BASELINE: &str = "u16[$C000] >= 60 && pc == pc";
const RUNS: u32 = 3;

/// v1 after Main's initializing writes, so `$C000` is not never-written.
fn machine() -> Machine {
    let mut config = CoreConfig::new(Model::DmgB, V1.to_vec());
    config.boot_rom = BootRom::Custom(QUICK_BOOT.to_vec());
    let mut m = Machine::new(Backend::sameboy(), config, None).expect("machine");
    for _ in 0..10_000 {
        if m.core().registers().pc == 0x015E {
            return m;
        }
        m.core_mut().step(StepUnit::Instruction).expect("step");
    }
    panic!("never reached Main.loop");
}

/// Mean seconds per iteration of restoring from `blob` and running `text`,
/// and the last stop. This is the cost both cadences share plus the run.
fn measure(
    m: &mut Machine,
    blob: &[u8],
    opts: &RunOptions<'_>,
    text: &str,
    iterations: u32,
) -> (f64, Outcome) {
    let mut last = None;
    let start = Instant::now();
    for _ in 0..iterations {
        m.core_mut().load(blob).expect("restore");
        last = Some(m.run_until(text, opts).expect("run"));
    }
    (
        start.elapsed().as_secs_f64() / f64::from(iterations),
        last.expect("one run"),
    )
}

/// Mean seconds per iteration of restoring from `blob` alone.
fn restore_only(m: &mut Machine, blob: &[u8], iterations: u32) -> f64 {
    let start = Instant::now();
    for _ in 0..iterations {
        m.core_mut().load(blob).expect("restore");
    }
    start.elapsed().as_secs_f64() / f64::from(iterations)
}

/// Mean seconds per iteration of restoring and running the same 537
/// instructions with no predicate: the floor the cadences build on.
fn no_predicate(m: &mut Machine, blob: &[u8], iterations: u32) -> f64 {
    let opts = RunOptions::new(Budget::instructions(537));
    let start = Instant::now();
    for _ in 0..iterations {
        m.core_mut().load(blob).expect("restore");
        m.run(&opts).expect("run");
    }
    start.elapsed().as_secs_f64() / f64::from(iterations)
}

fn cadence(outcome: &Outcome) -> Cadence {
    match &outcome.stop.reason {
        StopReason::Predicate { cadence, .. } => *cadence,
        other => panic!("expected a predicate stop, got {other:?}"),
    }
}

fn main() {
    let iterations: u32 = std::env::args()
        .nth(1)
        .and_then(|a| a.parse().ok())
        .unwrap_or(10_000);
    let mut m = machine();
    let blob = m.core().save();
    let opts = RunOptions::new(Budget::frames(60));

    let (mut restore, mut floor, mut derived, mut baseline) = (f64::MAX, f64::MAX, None, None);
    for _ in 0..RUNS {
        restore = restore.min(restore_only(&mut m, &blob, iterations));
        floor = floor.min(no_predicate(&mut m, &blob, iterations));
        let d = measure(&mut m, &blob, &opts, DERIVED, iterations);
        let b = measure(&mut m, &blob, &opts, BASELINE, iterations);
        if derived.as_ref().is_none_or(|(wall, _)| d.0 < *wall) {
            derived = Some(d);
        }
        if baseline.as_ref().is_none_or(|(wall, _)| b.0 < *wall) {
            baseline = Some(b);
        }
    }
    let (d_wall, d_stop) = derived.expect("derived timing");
    let (b_wall, b_stop) = baseline.expect("baseline timing");

    assert_eq!(cadence(&d_stop), Cadence::Writes);
    assert_eq!(cadence(&b_stop), Cadence::EveryInstruction);
    assert_eq!(d_stop.stop.pc, b_stop.stop.pc, "different stop PC");
    assert_eq!(
        d_stop.stop.budget.instructions, b_stop.stop.budget.instructions,
        "different instruction count"
    );

    let us = |s: f64| s * 1e6;
    let f_run = us((floor - restore).max(0.0));
    let d_run = us((d_wall - restore).max(0.0));
    let b_run = us((b_wall - restore).max(0.0));
    println!(
        "until_bench: {iterations} iterations, best of {RUNS}; restore {:.2}us",
        us(restore)
    );
    println!("  floor    537 instructions, no predicate: run {f_run:.2}us");
    println!(
        "  derived  cadence={:?} stop=${:04X} instructions={} ticks={} run {d_run:.2}us \
         (overhead {:.2}us)",
        cadence(&d_stop),
        d_stop.stop.pc,
        d_stop.stop.budget.instructions,
        d_stop.stop.time.ticks,
        (d_run - f_run).max(0.0)
    );
    println!(
        "  baseline cadence={:?} stop=${:04X} instructions={} ticks={} run {b_run:.2}us \
         (overhead {:.2}us)",
        cadence(&b_stop),
        b_stop.stop.pc,
        b_stop.stop.budget.instructions,
        b_stop.stop.time.ticks,
        (b_run - f_run).max(0.0)
    );
    println!("  speedup {:.1}x", b_run / d_run.max(f64::MIN_POSITIVE));
}
