//! Breakpoints, watches, stepping and every stop reason, each checked for
//! its PC, bank and emulated time.

mod common;

use std::sync::atomic::AtomicBool;

use common::*;
use gbcli_core::{Access, Budget, BudgetKind, CoreConfig, Model};
use gbcli_engine::{Backend, DebugError, Machine, Outcome, RunOptions, Since};
use gbcli_protocol::{Location, StepKind, StopReason, StopRecord, TimeBase, WatchAccess};

#[test]
fn mapped_symbols_name_addresses_for_the_disassembler() {
    use gbcli_engine::disasm::{SymbolLookup, SymbolRef};
    let m = machine(V1, Some(V1_SYM));
    let mapped = m.symbols().unwrap().mapped(m.core());
    let work = SymbolRef {
        name: "Work".into(),
        offset: 1,
    };
    assert_eq!(mapped.symbol(0x0164), Some(work));
    assert_eq!(mapped.symbol(0x4000), None);
}

const ILLEGAL: &[u8] = include_bytes!("../../sameboy/tests/roms/illegal.gb");
const STOP: &[u8] = include_bytes!("../../sameboy/tests/roms/stop.gb");
const HALT: &[u8] = include_bytes!("roms/halt.gb");

fn at(addr: u16) -> Location {
    Location {
        addr,
        bank: None,
        symbol: None,
    }
}

fn opts() -> RunOptions<'static> {
    RunOptions::new(Budget::frames(60))
}

/// The record's PC, bank and time agree with the machine it came from.
fn check_place(m: &Machine, stop: &StopRecord, pc: u16) {
    assert_eq!(stop.pc, pc, "{stop:?}");
    assert_eq!(m.core().registers().pc, pc);
    assert_eq!(stop.bank, 0);
    assert_eq!(stop.time.ticks, m.core().now().0);
    assert!(stop.time.ticks > 0);
    assert_eq!(stop.time.ly, m.core().ppu().ly);
    assert_eq!(stop.time.dot, None);
}

fn run(m: &mut Machine, o: &RunOptions) -> StopRecord {
    m.run(o).expect("run").stop
}

#[test]
fn a_breakpoint_stops_before_its_instruction() {
    let mut m = machine(V1, Some(V1_SYM));
    let id = m
        .add_breakpoint(
            Location {
                addr: 0x0163,
                bank: Some(0),
                symbol: Some("Work".into()),
            },
            None,
        )
        .unwrap();
    let stop = run(&mut m, &opts());
    assert_eq!(stop.reason, StopReason::Breakpoint { id });
    assert_eq!(stop.symbol.as_deref(), Some("Work"));
    check_place(&m, &stop, 0x0163);
}

#[test]
fn a_conditional_breakpoint_stops_only_when_it_holds() {
    let mut m = machine(V1, Some(V1_SYM));
    let id = m.add_breakpoint(at(0x0163), Some("[wFrame] == 3")).unwrap();
    let stop = run(&mut m, &opts());
    assert_eq!(stop.reason, StopReason::Breakpoint { id });
    check_place(&m, &stop, 0x0163);
    assert_eq!(
        m.core().region(gbcli_core::Region::Wram).unwrap().bytes[0],
        3
    );
}

#[test]
fn a_bad_condition_is_refused_when_set() {
    let mut m = machine(V1, Some(V1_SYM));
    assert!(m.add_breakpoint(at(0x0163), Some("wFrame >= 3")).is_err());
    assert!(m.add_breakpoint(at(0x0163), Some("[nope]")).is_err());
    assert!(m.debugger_config().breakpoints.is_empty());
}

#[test]
fn a_write_watch_stops_after_the_access_with_old_and_new() {
    let mut m = machine(V1, Some(V1_SYM));
    let id = m
        .add_watch(WatchAccess::Write, at(0xC000), Some("new == 2"), false)
        .unwrap();
    let stop = run(&mut m, &opts());
    assert_eq!(
        stop.reason,
        StopReason::Watchpoint {
            id,
            access: Access::Write,
            addr: 0xC000,
            old: Some(1),
            new: 2,
            writer_pc: 0x016B,
        }
    );
    check_place(&m, &stop, 0x016C);
}

#[test]
fn a_once_watch_is_deleted_after_its_stop() {
    let mut m = machine(V1, Some(V1_SYM));
    m.add_watch(WatchAccess::Read, at(0xC002), None, true)
        .unwrap();
    let stop = run(&mut m, &opts());
    assert!(matches!(
        stop.reason,
        StopReason::Watchpoint {
            access: Access::Read,
            addr: 0xC002,
            writer_pc: 0x0163,
            ..
        }
    ));
    check_place(&m, &stop, 0x0166);
    assert!(m.debugger_config().watches.is_empty());
    let next = run(&mut m, &RunOptions::new(Budget::instructions(50)));
    assert!(matches!(next.reason, StopReason::BudgetExhausted { .. }));
}

#[test]
fn a_condition_reading_never_written_memory_fails_the_run_with_its_stop() {
    let mut m = machine(V1, Some(V1_SYM));
    m.add_breakpoint(at(0x0163), Some("[wNever] == [wNever]"))
        .unwrap();
    let err = m.run(&opts()).unwrap_err();
    let DebugError::Eval { stop, error, .. } = &err else {
        panic!("{err:?}")
    };
    assert!(error.remedy().contains("--allow-uninit"));
    check_place(&m, stop, 0x0163);

    let mut allowed = opts();
    allowed.allow_uninit = true;
    let Outcome { stop, warnings } = m.run(&allowed).unwrap();
    assert!(matches!(stop.reason, StopReason::Breakpoint { .. }));
    assert!(!warnings.is_empty());
}

#[test]
fn stepping_one_instruction() {
    let mut m = machine(V1, Some(V1_SYM));
    let stop = m.step(StepKind::Instruction, &opts()).unwrap().stop;
    assert_eq!(
        stop.reason,
        StopReason::StepDone {
            unit: StepKind::Instruction
        }
    );
    assert_eq!(stop.budget.instructions, 1);
    check_place(&m, &stop, 0x0101);
}

#[test]
fn stepping_over_a_call_stops_at_the_return_address() {
    let mut m = machine(V1, Some(V1_SYM));
    step_to(&mut m, 0x015E);
    let stop = m.step(StepKind::Over, &opts()).unwrap().stop;
    assert_eq!(
        stop.reason,
        StopReason::StepDone {
            unit: StepKind::Over
        }
    );
    assert!(stop.budget.instructions > 5);
    check_place(&m, &stop, 0x0161);
}

#[test]
fn stepping_out_runs_until_the_routine_returns() {
    let mut m = machine(V1, Some(V1_SYM));
    step_to(&mut m, 0x0167);
    let stop = m.step(StepKind::Out, &opts()).unwrap().stop;
    assert_eq!(
        stop.reason,
        StopReason::StepDone {
            unit: StepKind::Out
        }
    );
    check_place(&m, &stop, 0x0161);
}

#[test]
fn stepping_a_frame_ends_at_the_next_vblank() {
    let mut m = machine(V1, Some(V1_SYM));
    let before = m.core().last_complete_frame().map_or(0, |f| f.tag.number);
    let stop = m.step(StepKind::Frame, &opts()).unwrap().stop;
    assert_eq!(
        stop.reason,
        StopReason::StepDone {
            unit: StepKind::Frame
        }
    );
    assert_eq!(stop.budget.frames, 1);
    let pc = m.core().registers().pc;
    check_place(&m, &stop, pc);
    let shown = stop.frame_shown.expect("a frame");
    assert_eq!(shown.n as u64, before + 1);
}

#[test]
fn stepping_a_scanline_ends_when_ly_changes() {
    let mut m = machine(BUSY, None);
    m.step(StepKind::Frame, &opts()).unwrap();
    let ly = m.core().ppu().ly;
    let stop = m.step(StepKind::Scanline, &opts()).unwrap().stop;
    assert_eq!(
        stop.reason,
        StopReason::StepDone {
            unit: StepKind::Scanline
        }
    );
    assert_ne!(stop.time.ly, ly);
    let pc = m.core().registers().pc;
    check_place(&m, &stop, pc);
}

#[test]
fn running_to_an_address() {
    let mut m = machine(V1, Some(V1_SYM));
    let stop = m.run_to(at(0x0170), &opts()).unwrap().stop;
    assert_eq!(
        stop.reason,
        StopReason::StepDone {
            unit: StepKind::ToAddress
        }
    );
    assert_eq!(stop.symbol.as_deref(), Some("Work.done"));
    check_place(&m, &stop, 0x0170);
}

#[test]
fn a_budget_ends_a_run_that_finds_nothing() {
    let mut m = machine(V1, Some(V1_SYM));
    let stop = run(&mut m, &RunOptions::new(Budget::instructions(10)));
    assert_eq!(
        stop.reason,
        StopReason::BudgetExhausted {
            kind: BudgetKind::Instructions
        }
    );
    assert_eq!(stop.budget.instructions, 10);
    let pc = m.core().registers().pc;
    check_place(&m, &stop, pc);
}

#[test]
fn a_budget_spans_passed_over_candidates() {
    let mut m = machine(V1, Some(V1_SYM));
    m.add_breakpoint(at(0x0163), Some("0")).unwrap();
    let stop = run(&mut m, &RunOptions::new(Budget::instructions(500)));
    assert_eq!(stop.budget.instructions, 500);
}

#[test]
fn an_illegal_opcode_stops_before_it() {
    let mut m = machine(ILLEGAL, None);
    let stop = run(&mut m, &opts());
    assert_eq!(stop.reason, StopReason::IllegalOpcode { opcode: 0xD3 });
    check_place(&m, &stop, 0x0156);
}

#[test]
fn halt_with_interrupts_off_stops() {
    let mut m = machine(HALT, None);
    let stop = run(&mut m, &opts());
    assert_eq!(stop.reason, StopReason::HaltForever);
    assert!(stop.budget.frames < 60);
    check_place(&m, &stop, 0x0155);
    assert!(m.core().registers().halted);
}

#[test]
fn stop_with_no_wake_source_stops() {
    let mut m = machine(STOP, None);
    let stop = run(&mut m, &opts());
    assert_eq!(stop.reason, StopReason::Stopped);
    let pc = m.core().registers().pc;
    check_place(&m, &stop, pc);
}

#[test]
fn a_user_break_stops_the_run() {
    let mut m = machine(V1, Some(V1_SYM));
    let flag = AtomicBool::new(true);
    let mut o = opts();
    o.interrupt = Some(&flag);
    let stop = run(&mut m, &o);
    assert_eq!(stop.reason, StopReason::UserBreak);
    let pc = m.core().registers().pc;
    check_place(&m, &stop, pc);
}

#[test]
fn time_counts_from_the_boot_handoff_once_seen() {
    let mut config = CoreConfig::new(Model::DmgB, V1.to_vec());
    config.boot_rom = gbcli_core::BootRom::Custom(QUICK_BOOT.to_vec());
    let mut m = Machine::new(Backend::sameboy(), config, Some(V1_SYM)).unwrap();
    let first = m.step(StepKind::Instruction, &opts()).unwrap().stop;
    assert_eq!(first.time.since, TimeBase::PowerOn, "no handoff yet");
    for _ in 0..3 {
        m.step(StepKind::Instruction, &opts()).unwrap();
    }
    let boot = m.boot().expect("the FF50 write was seen");
    let stop = m.step(StepKind::Instruction, &opts()).unwrap().stop;
    assert_eq!(stop.time.since, TimeBase::Boot);
    assert_eq!(stop.time.t, ((stop.time.ticks - boot.ticks) / 2) as i64);

    let mut o = opts();
    o.since = Since::PowerOn;
    let stop = m.step(StepKind::Instruction, &o).unwrap().stop;
    assert_eq!(stop.time.since, TimeBase::PowerOn);
    assert_eq!(stop.time.t, (stop.time.ticks / 2) as i64);

    o.since = Since::Checkpoint {
        name: "f0".into(),
        mark: gbcli_protocol::TimeMark {
            ticks: stop.time.ticks + 8,
            frame: 0,
        },
    };
    let stop = m.step(StepKind::Instruction, &o).unwrap().stop;
    assert_eq!(stop.time.since, TimeBase::Checkpoint("f0".into()));
    assert!(stop.time.t < 4);
}

#[test]
fn a_backtrace_lists_return_addresses_on_the_stack() {
    let mut m = machine(V1, Some(V1_SYM));
    step_to(&mut m, 0x0167);
    let frames = m.backtrace();
    assert_eq!(frames.len(), 1, "{frames:?}");
    assert_eq!(frames[0].return_addr, 0x0161);
    assert_eq!(frames[0].symbol.as_deref(), Some("Main.loop+3"));
}
