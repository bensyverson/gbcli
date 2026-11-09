//! The PPU mode where a register write lands and where a condition reads
//! STAT, on `ppu_mode.asm`: it writes SCX in modes 2, 3 and 0 without reading
//! a PPU register first, so SameBoy's display is batched at each write (has
//! cycles it hasn't run), which is where STAT used to be refused.

use crate::common;

use common::{machine, quick_dmg};
use gbcli_core::{Budget, Core, Event, EventKinds, EventSink, IoReg, PpuMode, RunRequest};
use gbcli_engine::{DebugError, Machine, RunOptions, Symbols};
use gbcli_protocol::{Location, StopReason, WatchAccess};

const ROM: &[u8] = include_bytes!("../../../sameboy/tests/roms/ppu_mode.gb");
const SYM: &str = include_str!("../../../sameboy/tests/roms/ppu_mode.sym");

fn addr(name: &str) -> u16 {
    Symbols::parse(SYM).unwrap().get(name).unwrap().addr
}

#[derive(Default)]
struct Writes(Vec<(u16, u8, Option<PpuMode>)>);

impl EventSink for Writes {
    fn kinds(&self) -> EventKinds {
        EventKinds {
            exec: false,
            raster: true,
            ppu_modes: false,
        }
    }
    fn event(&mut self, event: Event) {
        if let Event::IoWrite { pc, reg, mode, .. } = event {
            self.0.push((pc, reg, mode));
        }
    }
}

/// The mode of every SCX write from `label` over three frames.
fn modes_at(label: &str) -> Vec<Option<PpuMode>> {
    let mut core = quick_dmg(ROM);
    let mut sink = Writes::default();
    let mut req = RunRequest::new(Budget::frames(3));
    req.sink = Some(&mut sink);
    core.run(&mut req).expect("run");
    let pc = addr(label);
    let modes: Vec<_> = sink
        .0
        .iter()
        .filter(|(at, reg, _)| *at == pc && *reg == IoReg::SCX.0)
        .map(|w| w.2)
        .collect();
    assert!(modes.len() >= 2, "{label}: {modes:?}");
    modes
}

#[test]
fn a_write_carries_the_mode_it_landed_in_or_none() {
    for (label, mode) in [
        ("Line50Oam", Some(PpuMode::OamScan)),
        ("Line50Drawing", Some(PpuMode::Drawing)),
        ("Line50HBlank", Some(PpuMode::HBlank)),
        // Ten objects and no HBlank interrupt: SameBoy batches mode 3 by
        // 300 dots, an upper bound, so 190 dots in it can't tell 3 from 0
        // without running the PPU ahead, which would change the emulation.
        // Unknown, never wrong.
        ("Line100Late", None),
    ] {
        let got = modes_at(label);
        assert!(got.iter().all(|m| *m == mode), "{label}: {got:?}");
    }
}

fn at(label: &str) -> Location {
    Location {
        addr: addr(label),
        bank: None,
        symbol: None,
    }
}

fn opts() -> RunOptions<'static> {
    RunOptions::new(Budget::frames(3))
}

fn with_breakpoint(label: &str, condition: &str) -> (Machine, u32) {
    let mut m = machine(ROM, Some(SYM));
    let id = m.add_breakpoint(at(label), Some(condition)).unwrap();
    (m, id)
}

#[test]
fn a_stat_condition_evaluates_while_mode_2_is_batched() {
    // The racer's case: a write in mode 2, reached with the OAM scan batched.
    let (mut m, id) = with_breakpoint("Line50Oam", "([rSTAT] & 3) != 0");
    let stop = m.run(&opts()).expect("evaluates").stop;
    assert_eq!(stop.reason, StopReason::Breakpoint { id });
    assert_eq!(stop.time.ly, 50);
}

#[test]
fn a_stat_condition_evaluates_while_mode_3_is_batched() {
    let (mut m, id) = with_breakpoint("Line50Drawing", "([rSTAT] & 3) == 3 && [rLY] == 50");
    let stop = m.run(&opts()).expect("evaluates").stop;
    assert_eq!(stop.reason, StopReason::Breakpoint { id });
}

#[test]
fn a_condition_that_cannot_be_evaluated_is_an_error_not_a_breakpoint_stop() {
    // The one window left where STAT can't be read without running the PPU.
    // The run fails, and its record says why it stopped: not "breakpoint",
    // which `show` would then report as if the condition had held.
    let (mut m, id) = with_breakpoint("Line100Late", "([rSTAT] & 3) != 0");
    let err = m.run(&opts()).unwrap_err();
    let DebugError::Eval { stop, .. } = &err else {
        panic!("{err:?}")
    };
    assert_eq!(stop.reason, StopReason::ConditionError { id });
    assert_eq!(m.core().registers().pc, addr("Line100Late"));
}

const SWEEP: &[u8] = include_bytes!("../../../sameboy/tests/roms/mode_sweep.gb");
const SWEEP_SYM: &str = include_str!("../../../sameboy/tests/roms/mode_sweep.sym");
/// The frame of the sweep whose VRAM write starts at the end of mode 2 and
/// lands in mode 3 (`crates/sameboy/tests/it/mode_sweep.rs`).
const K: u16 = 107;

/// `help traps`' check for a dropped VRAM write: a write watch on the byte
/// whose condition says it didn't take the value written. It is evaluated
/// after the write, so it sees where the write landed, and catches the write
/// that STAT at the instruction's start (the trap's old breakpoint) misses.
#[test]
fn a_watch_on_the_byte_catches_a_dropped_vram_write_a_stat_breakpoint_misses() {
    let sweep_addr = |name: &str| Symbols::parse(SWEEP_SYM).unwrap().get(name).unwrap().addr;
    let opts = RunOptions::new(Budget::frames(130));

    let mut m = machine(SWEEP, Some(SWEEP_SYM));
    let at_write = Location {
        addr: sweep_addr("WriteVram"),
        bank: None,
        symbol: None,
    };
    let condition = format!("([rSTAT] & 3) == 3 && [$FF80] == {K}");
    m.add_breakpoint(at_write, Some(&condition)).unwrap();
    let stop = m.run(&opts).expect("runs").stop;
    assert!(
        matches!(stop.reason, StopReason::BudgetExhausted { .. }),
        "{:?}",
        stop.reason
    );
    assert_eq!(m.core_mut().bus_read(0x8000 + K).unwrap(), 0, "dropped");

    let mut m = machine(SWEEP, Some(SWEEP_SYM));
    let byte = Location {
        addr: 0x8000 + K,
        bank: None,
        symbol: None,
    };
    let condition = format!("[${:04X}] != new", 0x8000 + K);
    let id = m
        .add_watch(WatchAccess::Write, byte, Some(&condition), false)
        .unwrap();
    let stop = m.run(&opts).expect("runs").stop;
    let StopReason::Watchpoint {
        id: hit,
        new,
        writer_pc,
        ..
    } = stop.reason
    else {
        panic!("{:?}", stop.reason)
    };
    assert_eq!((hit, new, writer_pc), (id, 0xFF, sweep_addr("WriteVram")));
}
