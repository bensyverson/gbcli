//! Execution candidates the run loop passes over or must not miss: a
//! breakpoint's condition is evaluated once per hit, and a breakpoint at the
//! first instruction of a core call (where the core skips its filter) still
//! stops.

use crate::common;

use std::cell::Cell;

use common::*;
use gbcli_core::{
    ApuState, Budget, Core, CoreConfig, CoreError, CoreInfo, Feature, FrameRef, IoReg, Keys,
    LayerMap, PpuStatus, Region, RegionView, Registers, RunRequest, StepUnit, Stop, Support, Ticks,
    WrittenView,
};
use gbcli_engine::{Backend, Machine, RunOptions};
use gbcli_protocol::{Location, StopReason};
use gbcli_sameboy::SameBoy;

fn at(addr: u16) -> Location {
    Location {
        addr,
        bank: None,
        symbol: None,
    }
}

thread_local! {
    /// Reads of SCY through `io_register` on this thread: the engine reads it
    /// only to evaluate a condition that names it.
    static SCY_READS: Cell<u32> = const { Cell::new(0) };
}

/// SameBoy, counting the reads a condition makes.
struct Counting(SameBoy);

impl Core for Counting {
    fn info(&self) -> CoreInfo {
        self.0.info()
    }
    fn supports(&self, feature: Feature) -> Support {
        self.0.supports(feature)
    }
    fn run(&mut self, req: &mut RunRequest<'_>) -> Result<Stop, CoreError> {
        self.0.run(req)
    }
    fn step(&mut self, unit: StepUnit) -> Result<Stop, CoreError> {
        self.0.step(unit)
    }
    fn now(&self) -> Ticks {
        self.0.now()
    }
    fn registers(&self) -> Registers {
        self.0.registers()
    }
    fn in_stop(&self) -> bool {
        self.0.in_stop()
    }
    fn region(&self, region: Region) -> Result<RegionView<'_>, CoreError> {
        self.0.region(region)
    }
    fn rom0_bank(&self) -> u16 {
        self.0.rom0_bank()
    }
    fn io_register(&self, reg: IoReg) -> Result<u8, CoreError> {
        if reg == IoReg::SCY {
            SCY_READS.with(|n| n.set(n.get() + 1));
        }
        self.0.io_register(reg)
    }
    fn ppu(&self) -> PpuStatus {
        self.0.ppu()
    }
    fn last_complete_frame(&self) -> Option<FrameRef<'_>> {
        self.0.last_complete_frame()
    }
    fn pixel_layers(&self) -> Result<LayerMap<'_>, CoreError> {
        self.0.pixel_layers()
    }
    fn set_pixel_layers(&mut self, on: bool) -> Result<(), CoreError> {
        self.0.set_pixel_layers(on)
    }
    fn apu(&self) -> Result<ApuState, CoreError> {
        self.0.apu()
    }
    fn written(&self, region: Region) -> Result<WrittenView<'_>, CoreError> {
        self.0.written(region)
    }
    fn bus_write(&mut self, addr: u16, value: u8) -> Result<(), CoreError> {
        self.0.bus_write(addr, value)
    }
    fn bus_read(&mut self, addr: u16) -> Result<u8, CoreError> {
        self.0.bus_read(addr)
    }
    fn poke(&mut self, region: Region, offset: u32, bytes: &[u8]) -> Result<(), CoreError> {
        self.0.poke(region, offset, bytes)
    }
    fn patch_rom(&mut self, offset: u32, bytes: &[u8]) -> Result<(), CoreError> {
        self.0.patch_rom(offset, bytes)
    }
    fn set_registers(&mut self, regs: &Registers) {
        self.0.set_registers(regs)
    }
    fn set_keys(&mut self, keys: Keys) {
        self.0.set_keys(keys)
    }
    fn save(&mut self) -> Vec<u8> {
        self.0.save()
    }
    fn load(&mut self, blob: &[u8]) -> Result<(), CoreError> {
        self.0.load(blob)
    }
}

fn counting_machine(rom: &[u8], sym: Option<&str>) -> Machine {
    let backend = Backend::new(
        |config: CoreConfig| Ok(Box::new(Counting(SameBoy::new(config)?))),
        gbcli_sameboy::builtin_boot_rom,
    );
    let mut m = Machine::new(backend, config(rom), sym).expect("machine");
    steps(&mut m, 4);
    m
}

#[test]
fn a_passed_over_breakpoint_evaluates_its_condition_once_per_hit() {
    const RUN: u64 = 200;
    let mut m = counting_machine(V1, Some(V1_SYM));
    // The oracle: how often PC sits at Work after an instruction, before the
    // budget ends.
    let mut truth = m.try_clone().expect("clone");
    let mut hits = 0;
    for _ in 1..RUN {
        truth.core_mut().step(StepUnit::Instruction).unwrap();
        hits += u32::from(truth.core().registers().pc == 0x0163);
    }
    truth.core_mut().step(StepUnit::Instruction).unwrap();
    assert_ne!(
        truth.core().registers().pc,
        0x0163,
        "the run ends off a hit"
    );
    assert!(hits > 10);

    m.add_breakpoint(at(0x0163), Some("[rSCY] == $99")).unwrap();
    SCY_READS.with(|n| n.set(0));
    let stop = m
        .run(&RunOptions::new(Budget::instructions(RUN)))
        .unwrap()
        .stop;
    assert!(matches!(stop.reason, StopReason::BudgetExhausted { .. }));
    assert_eq!(SCY_READS.with(Cell::get), hits);
}

#[test]
fn a_breakpoint_at_a_chunk_boundary_stops() {
    // The run loop hands the core 4,096 instructions at a time, and the core
    // skips its execution filter at the first instruction of a call. busy.gb
    // runs its vblank handler once a frame, more than 4,096 instructions
    // apart, so the run can be placed to reach it exactly at a boundary.
    const CHUNK: u64 = 4096;
    const VBLANK: u16 = 0x0184;
    let mut m = machine(BUSY, None);
    step_to(&mut m, VBLANK);
    steps(&mut m, 1);
    let mut truth = m.try_clone().expect("clone");
    let mut gap = 0u64;
    while truth.core().registers().pc != VBLANK {
        truth.core_mut().step(StepUnit::Instruction).unwrap();
        gap += 1;
        assert!(gap < 100_000, "no vblank handler");
    }
    assert!(gap > CHUNK, "the next vblank is {gap} instructions away");
    for _ in 0..gap - CHUNK {
        m.core_mut().step(StepUnit::Instruction).unwrap();
    }

    let id = m.add_breakpoint(at(VBLANK), None).unwrap();
    let stop = m.run(&RunOptions::new(Budget::frames(3))).unwrap().stop;
    assert_eq!(stop.reason, StopReason::Breakpoint { id });
    assert_eq!(stop.pc, VBLANK);
    assert_eq!(stop.budget.instructions, CHUNK);
}
