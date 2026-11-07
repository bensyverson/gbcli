//! The tune ROM on a DMG, and the golden-image comparison.
//!
//! Regenerate the goldens, after checking that a change to them is intended,
//! with `GBCLI_BLESS=1 cargo test -p gbcli-audio --test views`. Without it a
//! mismatch fails and writes the actual bytes beside the test's target
//! directory; a missing golden fails too, so none is ever created silently.

#![allow(dead_code)] // each test binary uses a different subset

use std::path::PathBuf;

use gbcli_core::{BootRom, Budget, Core, CoreConfig, Model, RunRequest, StepUnit};
use gbcli_sameboy::SameBoy;

pub const TUNE: &[u8] = include_bytes!("../roms/tune.gb");
const QUICK_BOOT: &[u8] = include_bytes!("../../../sameboy/tests/roms/quick_boot.bin");

/// Frames the tune takes to play every note and turn the APU off.
pub const TUNE_FRAMES: u64 = 32;

/// A DMG core on the tune, handed over by the quick boot ROM.
pub fn tune_core() -> SameBoy {
    let mut config = CoreConfig::new(Model::DmgB, TUNE.to_vec());
    config.boot_rom = BootRom::Custom(QUICK_BOOT.to_vec());
    let mut core = SameBoy::new(config).expect("construct core");
    for _ in 0..4 {
        core.step(StepUnit::Instruction).unwrap();
    }
    core
}

pub fn run(core: &mut dyn Core, budget: Budget) {
    core.run(&mut RunRequest::new(budget)).expect("run");
}

fn golden_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/golden")
}

/// Compares `bytes` with `tests/golden/<file>` byte for byte, or rewrites the
/// golden when `GBCLI_BLESS=1`.
pub fn assert_golden(file: &str, bytes: &[u8]) {
    let path = golden_dir().join(file);
    if std::env::var_os("GBCLI_BLESS").is_some_and(|v| v == "1") {
        std::fs::create_dir_all(golden_dir()).unwrap();
        std::fs::write(&path, bytes).unwrap();
        return;
    }
    let expected = std::fs::read(&path).unwrap_or_else(|err| {
        panic!(
            "golden {} is missing ({err}); after checking it, create it with\n    \
             GBCLI_BLESS=1 cargo test -p gbcli-audio --test views",
            path.display()
        )
    });
    if expected != bytes {
        let actual = PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join(format!("actual-{file}"));
        std::fs::write(&actual, bytes).unwrap();
        panic!(
            "{file} differs from its golden; the actual bytes are at {}. If the change is \
             intended, regenerate with GBCLI_BLESS=1 cargo test -p gbcli-audio --test views",
            actual.display()
        );
    }
}
