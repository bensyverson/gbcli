//! Runs that end or wait in HALT or STOP. A breakpoint on the instruction
//! after either waits for the wake: PC already points past it, but nothing
//! there has run. The core guards its own execution filter this way; these
//! pin the engine's check at the boundaries between core calls. And a run
//! that ends in STOP says what wakes it.

mod common;

use common::machine;
use gbcli_core::{Budget, BudgetKind, Region, Registers};
use gbcli_engine::cond::EvalWarning;
use gbcli_engine::{FrameSink, Machine, RunOptions, Symbols};
use gbcli_protocol::{Location, StopReason};

const PROFILE: &[u8] = include_bytes!("roms/profile.gb");
const PROFILE_SYM: &str = include_str!("roms/profile.sym");
/// Executes STOP with neither joypad row selected; PC is then $015D.
const STOP: &[u8] = include_bytes!("../../sameboy/tests/roms/stop.gb");

fn addr(name: &str) -> u16 {
    Symbols::parse(PROFILE_SYM).unwrap().get(name).unwrap().addr
}

fn at(addr: u16) -> Location {
    Location {
        addr,
        bank: None,
        symbol: None,
    }
}

fn frames_counted(m: &Machine) -> u8 {
    let wram = m.core().region(Region::Wram).unwrap();
    wram.bytes[usize::from(addr("wFrames") - 0xC000)]
}

#[test]
fn a_breakpoint_after_halt_fires_after_the_wake() {
    // HaltLoop waits for each vblank in HALT. Frame stops end each core call
    // at the vblank, with the CPU still in HALT and PC on the `nop` after it.
    let mut m = machine(PROFILE, Some(PROFILE_SYM));
    let regs = Registers {
        pc: addr("HaltLoop"),
        ..m.core().registers()
    };
    m.core_mut().set_registers(&regs);
    let after_halt = addr("HaltLoop.wait") + 1;
    m.add_breakpoint(at(after_halt), None).unwrap();
    let on_frame = |_: &Machine, _: u64| {};
    let mut opts = RunOptions::new(Budget::frames(5));
    opts.on_frame = Some(FrameSink(&on_frame));
    let before = frames_counted(&m);
    let stop = m.run(&opts).expect("run").stop;
    assert!(
        matches!(stop.reason, StopReason::Breakpoint { .. }),
        "{stop:?}"
    );
    assert_eq!(stop.pc, after_halt);
    assert!(!m.core().registers().halted, "stopped still in HALT");
    assert_eq!(
        frames_counted(&m),
        before.wrapping_add(1),
        "the vblank handler the wake dispatched has run"
    );
}

#[test]
fn a_breakpoint_after_stop_never_fires_while_nothing_wakes_it() {
    let mut m = machine(STOP, None);
    m.add_breakpoint(at(0x015D), None).unwrap();
    let stop = m
        .run(&RunOptions::new(Budget::frames(10)))
        .expect("run")
        .stop;
    assert_eq!(
        stop.reason,
        StopReason::BudgetExhausted {
            kind: BudgetKind::Frames
        }
    );
}

#[test]
fn a_run_ending_in_stop_warns_that_only_a_key_wakes_it() {
    let mut m = machine(STOP, None);
    let outcome = m.run(&RunOptions::new(Budget::frames(3))).expect("run");
    assert_eq!(outcome.warnings, vec![EvalWarning::InStop]);
    let text = outcome.warnings[0].to_string();
    assert!(text.contains("selected joypad row"), "{text}");
    assert!(text.contains("--keys"), "{text}");
}

#[test]
fn a_run_ending_in_halt_has_no_stop_warning() {
    let mut m = machine(PROFILE, Some(PROFILE_SYM));
    let regs = Registers {
        pc: addr("HaltLoop"),
        ..m.core().registers()
    };
    m.core_mut().set_registers(&regs);
    let mut outcome = m.run(&RunOptions::new(Budget::frames(1))).expect("run");
    for _ in 0..100 {
        if m.core().registers().halted {
            break;
        }
        outcome = m
            .run(&RunOptions::new(Budget::instructions(1)))
            .expect("run");
    }
    assert!(m.core().registers().halted);
    assert_eq!(outcome.warnings, vec![]);
}
