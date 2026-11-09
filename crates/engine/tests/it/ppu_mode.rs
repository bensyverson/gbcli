//! The PPU mode where a register write lands and where a condition reads
//! STAT, on `ppu_mode.asm`: it writes SCX in modes 2, 3 and 0 without reading
//! a PPU register first, so SameBoy's display is batched at each write (has
//! cycles it hasn't run), which is where STAT used to be refused.

use crate::common;

use common::{machine, quick_dmg};
use gbcli_core::{Budget, Core, Event, EventKinds, EventSink, IoReg, PpuMode, RunRequest};
use gbcli_engine::{DebugError, Machine, RunOptions, Symbols};
use gbcli_protocol::{Location, StopReason};

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
