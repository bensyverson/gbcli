//! Shared helpers for the SameBoy backend's tests: ROMs, construction, and
//! running to a point.

#![allow(dead_code)] // each test binary uses a different subset

use std::path::PathBuf;

use gbcli_core::{BootRom, Budget, Core, CoreConfig, Model, RunRequest, Stop};
use gbcli_sameboy::SameBoy;

pub const BUSY: &[u8] = include_bytes!("../roms/busy.gb");
pub const ILLEGAL: &[u8] = include_bytes!("../roms/illegal.gb");
pub const STOP: &[u8] = include_bytes!("../roms/stop.gb");
/// Hands over in four instructions; not a faithful boot (see quick_boot.asm).
pub const QUICK_BOOT: &[u8] = include_bytes!("../roms/quick_boot.bin");

/// Where the pinned test-ROM corpus lives: `$GBCLI_TEST_ROMS`, else
/// `tests/corpus/fetched/` at the repo root.
fn corpus_dir() -> PathBuf {
    match std::env::var_os("GBCLI_TEST_ROMS") {
        Some(dir) => PathBuf::from(dir),
        None => PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../tests/corpus/fetched"),
    }
}

/// A ROM from the corpus by its path inside it. Fails loudly, never skips.
pub fn corpus_rom(path: &str) -> Vec<u8> {
    let full = corpus_dir().join(path);
    std::fs::read(&full).unwrap_or_else(|err| {
        panic!(
            "test ROM {} is missing ({err}). Fetch the pinned corpus from the repo root with:\n    \
             python3 -I scripts/corpus/fetch.py --only blargg\n\
             (or point GBCLI_TEST_ROMS at a directory holding {path})",
            full.display()
        )
    })
}

/// Blargg's cpu_instrs, the combined ROM (MBC1, 64 KiB).
pub fn cpu_instrs() -> Vec<u8> {
    corpus_rom("blargg/cpu_instrs/cpu_instrs.gb")
}

/// A DMG core on `rom` with the quick boot ROM, already handed over (PC $0100).
pub fn quick_dmg(rom: &[u8]) -> SameBoy {
    let mut config = CoreConfig::new(Model::DmgB, rom.to_vec());
    config.boot_rom = BootRom::Custom(QUICK_BOOT.to_vec());
    let mut core = SameBoy::new(config).expect("construct core");
    for _ in 0..4 {
        core.step(gbcli_core::StepUnit::Instruction)
            .expect("boot step");
    }
    assert_eq!(core.registers().pc, 0x0100, "quick boot didn't hand over");
    core
}

/// A DMG core on `rom` with the built-in boot ROM, at power-on.
pub fn dmg(rom: &[u8], seed: u64) -> SameBoy {
    let mut config = CoreConfig::new(Model::DmgB, rom.to_vec());
    config.seed = seed;
    SameBoy::new(config).expect("construct core")
}

/// Runs with no filters for a budget.
pub fn run(core: &mut dyn Core, budget: Budget) -> Stop {
    core.run(&mut RunRequest::new(budget)).expect("run")
}

/// A tiny deterministic generator (xorshift64*), so property tests need no crate.
pub struct Rng(u64);

impl Rng {
    pub fn new(seed: u64) -> Self {
        Rng(seed.max(1))
    }
    pub fn next(&mut self) -> u64 {
        self.0 ^= self.0 >> 12;
        self.0 ^= self.0 << 25;
        self.0 ^= self.0 >> 27;
        self.0.wrapping_mul(0x2545_F491_4F6C_DD1D)
    }
    pub fn below(&mut self, n: u64) -> u64 {
        self.next() % n
    }
}
