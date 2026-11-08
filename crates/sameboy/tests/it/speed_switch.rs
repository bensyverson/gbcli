//! The CGB speed switch: STOP with KEY1 bit 0 set pauses the CPU in a HALT
//! that ends on its own countdown, and SameBoy runs the next instruction in
//! the same step that ends it.

use crate::common;

use common::*;
use gbcli_core::{
    AddrFilter, BankSel, Budget, BudgetKind, Core, CoreConfig, Feature, HaltWake, Model, Region,
    RunRequest, StepUnit, StopReason, Support,
};
use gbcli_sameboy::SameBoy;

const SPEED_SWITCH: &[u8] = include_bytes!("../roms/speed_switch.gb");
const SPEED_SWITCH_SYM: &str = include_str!("../roms/speed_switch.sym");
const HALT: &[u8] = include_bytes!("../../../engine/tests/roms/halt.gb");

/// A label's address in speed_switch.sym.
fn label(name: &str) -> u16 {
    SPEED_SWITCH_SYM
        .lines()
        .find_map(|line| {
            let (at, sym) = line.split_once(' ')?;
            (sym == name).then(|| u16::from_str_radix(at.split_once(':')?.1, 16).ok())?
        })
        .unwrap_or_else(|| panic!("no {name} in speed_switch.sym"))
}

fn cgb() -> SameBoy {
    quick_boot(CoreConfig::new(Model::CgbE, SPEED_SWITCH.to_vec()))
}

fn wram(core: &SameBoy, offset: usize) -> u8 {
    core.region(Region::Wram).unwrap().bytes[offset]
}

/// Runs with an execution filter on `addr`.
fn run_to(core: &mut SameBoy, addr: u16) -> gbcli_core::Stop {
    let mut filter = AddrFilter::new();
    filter.insert(addr, BankSel::Any);
    let mut req = RunRequest::new(Budget::frames(30));
    req.exec = &filter;
    core.run(&mut req).unwrap()
}

#[test]
fn a_breakpoint_after_a_switch_to_double_speed_fires_before_the_instruction_runs() {
    let mut core = cgb();
    let stop = run_to(&mut core, label("ToDouble"));
    assert!(
        matches!(stop.reason, StopReason::Candidate(_)),
        "{:?}",
        stop.reason
    );
    assert_eq!(stop.pc, label("ToDouble"));
    assert_eq!(wram(&core, 0), 0x00, "the instruction after STOP ran");
    assert!(core.registers().double_speed);
    // Resuming runs it.
    run(&mut core, Budget::instructions(2));
    assert_eq!(wram(&core, 0), 0x99);
}

#[test]
fn a_breakpoint_after_a_switch_back_to_normal_speed_fires_before_the_instruction_runs() {
    let mut core = cgb();
    let stop = run_to(&mut core, label("ToNormal"));
    assert!(
        matches!(stop.reason, StopReason::Candidate(_)),
        "{:?}",
        stop.reason
    );
    assert_eq!(stop.pc, label("ToNormal"));
    assert_eq!(wram(&core, 0), 0x99);
    assert_eq!(wram(&core, 1), 0x00, "the instruction after STOP ran");
    assert!(!core.registers().double_speed);
}

#[test]
fn a_breakpoint_after_a_switch_waits_through_the_pause() {
    // Stepped into the pause, the filter still waits for the step that ends
    // it, rather than firing at once.
    let mut core = cgb();
    while !core.registers().halted {
        core.step(StepUnit::Instruction).unwrap();
    }
    let paused_at = core.now();
    let stop = run_to(&mut core, label("ToDouble"));
    assert!(matches!(stop.reason, StopReason::Candidate(_)));
    // About 2^17 T-cycles at double speed: 2^17 ticks of 8 MHz.
    let paused = stop.at.0 - paused_at.0;
    assert!(
        (0x1F000..0x21000).contains(&paused),
        "paused {paused} ticks"
    );
}

#[test]
fn halt_wake_tells_a_speed_switch_from_a_halt_waiting_for_an_interrupt() {
    let mut core = cgb();
    assert_eq!(core.supports(Feature::HaltWake), Support::Yes);
    assert_eq!(core.halt_wake(), Ok(None), "running, not halted");
    while !core.registers().halted {
        core.step(StepUnit::Instruction).unwrap();
    }
    assert_eq!(core.halt_wake(), Ok(Some(HaltWake::SpeedSwitch)));
    let mut steps = 0;
    while core.halt_wake() == Ok(Some(HaltWake::SpeedSwitch)) {
        core.step(StepUnit::Instruction).unwrap();
        steps += 1;
    }
    assert_eq!(core.halt_wake(), Ok(Some(HaltWake::SpeedSwitchEnding)));
    assert!(steps > 1000, "ending after {steps} steps");
    assert_eq!(core.registers().pc, label("ToDouble"));
    // The step that ends the pause runs the instruction at PC.
    core.step(StepUnit::Instruction).unwrap();
    assert!(!core.registers().halted);
    assert_eq!(core.registers().pc, label("ToDouble") + 2);

    let mut halt = quick_dmg(HALT);
    let stop = run(&mut halt, Budget::frames(1));
    assert_eq!(stop.reason, StopReason::Budget(BudgetKind::Frames));
    assert!(halt.registers().halted);
    assert_eq!(halt.halt_wake(), Ok(Some(HaltWake::Interrupt)));
}
