//! Shared helpers for the engine's tests over the SameBoy backend: ROMs,
//! their symbols, cores and machines.

#![allow(dead_code)] // each test binary uses a different subset

use gbcli_core::{BootRom, Budget, Core, CoreConfig, IoReg, Model, RunRequest, StepUnit};
use gbcli_engine::{Backend, Machine};
use gbcli_sameboy::SameBoy;

pub const BUSY: &[u8] = include_bytes!("../../../sameboy/tests/roms/busy.gb");
/// MBC5, 4 ROM banks, 4 cart RAM banks (see banked.asm).
pub const BANKED: &[u8] = include_bytes!("../../../sameboy/tests/roms/banked.gb");
/// Hands over in four instructions; not a faithful boot (see its .asm).
pub const QUICK_BOOT: &[u8] = include_bytes!("../../../sameboy/tests/roms/quick_boot.bin");

/// A DMG core on `rom`, handed over to the cartridge (PC $0100) by the quick boot ROM.
pub fn quick_dmg(rom: &[u8]) -> SameBoy {
    let mut config = CoreConfig::new(Model::DmgB, rom.to_vec());
    config.boot_rom = BootRom::Custom(QUICK_BOOT.to_vec());
    let mut core = SameBoy::new(config).expect("construct core");
    for _ in 0..4 {
        core.step(StepUnit::Instruction).expect("boot step");
    }
    assert_eq!(core.registers().pc, 0x0100, "quick boot didn't hand over");
    core
}

/// A CGB core on `rom` marked CGB-only (so it runs in CGB mode), after the
/// built-in boot ROM has handed over.
pub fn booted_cgb(rom: &[u8]) -> SameBoy {
    let mut rom = rom.to_vec();
    rom[0x143] = 0xC0;
    rom[0x14D] = rom[0x134..0x14D]
        .iter()
        .fold(0u8, |x, b| x.wrapping_sub(*b).wrapping_sub(1));
    let mut core = SameBoy::new(CoreConfig::new(Model::CgbE, rom)).expect("construct core");
    let mut frames = 0;
    while core.io_register(IoReg::BANK).unwrap() & 1 == 0 {
        assert!(frames < 600, "no handover after {frames} frames");
        run(&mut core, Budget::frames(1));
        frames += 1;
    }
    core
}

pub fn run(core: &mut dyn Core, budget: Budget) {
    core.run(&mut RunRequest::new(budget)).expect("run");
}

/// Restores the state after `f`, for checks that need a bus read.
pub fn on_a_copy<T>(core: &mut dyn Core, f: impl FnOnce(&mut dyn Core) -> T) -> T {
    let state = core.save();
    let out = f(core);
    core.load(&state).unwrap();
    out
}

/// A tiny deterministic generator (xorshift64*).
pub struct Rng(u64);

impl Rng {
    pub fn new(seed: u64) -> Self {
        Rng(seed.max(1))
    }
    pub fn below(&mut self, n: u64) -> u64 {
        self.0 ^= self.0 >> 12;
        self.0 ^= self.0 << 25;
        self.0 ^= self.0 >> 27;
        self.0.wrapping_mul(0x2545_F491_4F6C_DD1D) % n
    }
}

pub const V1: &[u8] = include_bytes!("../roms/rebuild_v1.gb");
pub const V2: &[u8] = include_bytes!("../roms/rebuild_v2.gb");
pub const V3: &[u8] = include_bytes!("../roms/rebuild_v3.gb");
pub const V4: &[u8] = include_bytes!("../roms/rebuild_v4.gb");
pub const V1_SYM: &str = include_str!("../roms/rebuild_v1.sym");
pub const V2_SYM: &str = include_str!("../roms/rebuild_v2.sym");
pub const V3_SYM: &str = include_str!("../roms/rebuild_v3.sym");
pub const V4_SYM: &str = include_str!("../roms/rebuild_v4.sym");

/// A DMG config on `rom` with the quick boot ROM.
pub fn config(rom: &[u8]) -> CoreConfig {
    let mut config = CoreConfig::new(Model::DmgB, rom.to_vec());
    config.boot_rom = BootRom::Custom(QUICK_BOOT.to_vec());
    config
}

/// A DMG machine on `rom` with the quick boot ROM, handed over (PC $0100).
pub fn machine(rom: &[u8], sym: Option<&str>) -> Machine {
    let mut m = Machine::new(Backend::sameboy(), config(rom), sym).expect("machine");
    steps(&mut m, 4);
    assert_eq!(
        m.core().registers().pc,
        0x0100,
        "quick boot didn't hand over"
    );
    m
}

pub fn steps(m: &mut Machine, n: usize) {
    for _ in 0..n {
        m.core_mut().step(StepUnit::Instruction).expect("step");
    }
}

/// Steps until PC is `pc`, within a bound.
pub fn step_to(m: &mut Machine, pc: u16) {
    for _ in 0..100_000 {
        if m.core().registers().pc == pc {
            return;
        }
        steps(m, 1);
    }
    panic!("PC never reached {pc:#06x}");
}
