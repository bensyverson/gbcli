//! The routine profiler's idle cycles (leaf jGOxRH), from steps fed to the
//! sink by hand: what a routine spent halted is told apart from what it ran.

use gbcli_core::{CpuStep, Event, EventSink};

use super::{RoutineCost, Routines};

const SP: u16 = 0xD000;

/// One single-speed step of `t` T-cycles at `pc`, leaving PC at `next`.
fn step(r: &mut Routines, pc: u16, step: CpuStep, t: u32, sp: (u16, u16), next: u16) {
    r.event(Event::Exec {
        pc,
        bank: 0,
        step,
        ticks: 2 * t,
        sp_before: sp.0,
        sp_after: sp.1,
        next_pc: next,
        next_bank: 0,
        double_speed: false,
    });
}

fn op(opcode: u8) -> CpuStep {
    CpuStep::Instruction { opcode }
}

fn cost(routines: &[RoutineCost], addr: u16) -> &RoutineCost {
    routines
        .iter()
        .find(|r| r.entry.addr == addr)
        .unwrap_or_else(|| panic!("no ${addr:04X} in {routines:#?}"))
}

/// The rogue's case: `MaybeYield` ends in `jp WaitFrame`, so WaitFrame's
/// HALT runs in MaybeYield's frame and is charged to it; `idle_t` says how
/// much of that exclusive time was the wait.
#[test]
fn a_routines_halt_cycles_are_its_idle_t_within_its_exclusive_time() {
    let mut r = Routines::new();
    let below = SP - 2;
    // Main at $0150 calls MaybeYield at $0200.
    step(&mut r, 0x0150, op(0xCD), 24, (SP, below), 0x0200);
    // MaybeYield: a NOP, then `jp WaitFrame` ($0300): no frame is pushed.
    step(&mut r, 0x0200, op(0x00), 4, (below, below), 0x0201);
    step(&mut r, 0x0201, op(0xC3), 16, (below, below), 0x0300);
    // WaitFrame: HALT, 100 T halted, then RET back into Main.
    step(&mut r, 0x0300, op(0x76), 4, (below, below), 0x0301);
    step(&mut r, 0x0301, CpuStep::Idle, 100, (below, below), 0x0301);
    step(&mut r, 0x0301, op(0xC9), 16, (below, SP), 0x0153);
    // Main runs on, and halts too.
    step(&mut r, 0x0153, CpuStep::Idle, 40, (SP, SP), 0x0153);

    let profile = r.finish(None);
    let yield_ = cost(&profile.routines, 0x0200);
    assert_eq!(yield_.exclusive_t, 4 + 16 + 4 + 100 + 16);
    assert_eq!(yield_.idle_t, 100, "WaitFrame's HALT, run in its frame");
    let main = cost(&profile.routines, 0x0150);
    assert_eq!((main.exclusive_t, main.idle_t), (24 + 40, 40), "the root");
    let idle: u64 = profile.routines.iter().map(|r| r.idle_t).sum();
    assert_eq!(idle, profile.idle_t, "every idle cycle is some routine's");
}
