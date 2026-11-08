//! An address that holds an illegal opcode can still be reached: `run --to`
//! it, or a breakpoint on it, stops there before the CPU locks up, and the
//! next run stops on the illegal opcode rather than on the arrival again.
//! The ROM reaches the illegal opcode $D3 at $0156.

use crate::common;

use common::machine;
use gbcli_core::Budget;
use gbcli_engine::{Machine, RunOptions};
use gbcli_protocol::{Location, StepKind, StopReason};

const ILLEGAL: &[u8] = include_bytes!("../../../sameboy/tests/roms/illegal.gb");

fn opts() -> RunOptions<'static> {
    RunOptions::new(Budget::frames(5))
}

fn at_illegal() -> Location {
    gbcli_engine::addr::parse("$0156", None).unwrap()
}

fn the_next_run_locks_up(m: &mut Machine) {
    let stop = m.run(&opts()).expect("run").stop;
    assert_eq!(stop.reason, StopReason::IllegalOpcode { opcode: 0xD3 });
    assert_eq!(m.core().registers().pc, 0x0156);
}

#[test]
fn running_to_an_address_holding_an_illegal_opcode_reaches_it() {
    let mut m = machine(ILLEGAL, None);
    let outcome = m.run_to(at_illegal(), &opts()).expect("run");
    assert_eq!(m.core().registers().pc, 0x0156, "PC is at the target");
    assert_eq!(
        outcome.stop.reason,
        StopReason::StepDone {
            unit: StepKind::ToAddress
        }
    );
    the_next_run_locks_up(&mut m);
}

#[test]
fn a_breakpoint_on_an_illegal_opcode_stops_before_the_lock_up() {
    let mut m = machine(ILLEGAL, None);
    let id = m.add_breakpoint(at_illegal(), None).unwrap();
    let stop = m.run(&opts()).expect("run").stop;
    assert_eq!(stop.reason, StopReason::Breakpoint { id });
    assert_eq!(m.core().registers().pc, 0x0156);
    the_next_run_locks_up(&mut m);
}

/// The control: `--until "pc == $0156"` held before the reorder too.
#[test]
fn until_pc_at_an_illegal_opcode_holds() {
    let mut m = machine(ILLEGAL, None);
    let outcome = m.run_until("pc == $0156", &opts()).expect("run");
    assert_eq!(m.core().registers().pc, 0x0156, "PC is at the target");
    assert!(
        matches!(outcome.stop.reason, StopReason::Predicate { .. }),
        "{:?}",
        outcome.stop.reason
    );
}
