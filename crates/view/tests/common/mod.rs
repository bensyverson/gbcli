//! Scenes from the committed test ROMs, and the golden-image comparison.
//!
//! Regenerate the goldens, after checking that a change to them is intended,
//! with `GBCLI_BLESS=1 cargo test -p gbcli-view --test golden`. Without it a
//! mismatch fails and writes the actual image beside the test's target
//! directory; a missing golden fails too, so none is ever created silently.

#![allow(dead_code)] // each test binary uses a different subset

use std::path::PathBuf;

use gbcli_core::{BootRom, Budget, Core, CoreConfig, IoReg, Model, Region, RunRequest, StopReason};
use gbcli_sameboy::SameBoy;

pub const LAYERS: &[u8] = include_bytes!("../../../sameboy/tests/roms/layers.gb");
pub const LAYERS_CGB: &[u8] = include_bytes!("../../../sameboy/tests/roms/layers_cgb.gb");
pub const BUSY: &[u8] = include_bytes!("../../../sameboy/tests/roms/busy.gb");
const QUICK_BOOT: &[u8] = include_bytes!("../../../sameboy/tests/roms/quick_boot.bin");

/// A DMG core on `rom`, handed over by the quick boot ROM.
pub fn quick_dmg(rom: &[u8]) -> SameBoy {
    let mut config = CoreConfig::new(Model::DmgB, rom.to_vec());
    config.boot_rom = BootRom::Custom(QUICK_BOOT.to_vec());
    let mut core = SameBoy::new(config).expect("construct core");
    for _ in 0..4 {
        core.step(gbcli_core::StepUnit::Instruction).unwrap();
    }
    core
}

/// A CGB core on `rom` with the fast CGB boot ROM, handed over.
pub fn fast_cgb(rom: &[u8]) -> SameBoy {
    let mut config = CoreConfig::new(Model::CgbE, rom.to_vec());
    config.boot_rom = BootRom::Custom(sameboy_sys::boot_roms::CGB_FAST.to_vec());
    let mut core = SameBoy::new(config).expect("construct core");
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

/// Runs to the end of the next frame.
pub fn to_frame_end(core: &mut dyn Core) {
    let mut req = RunRequest::new(Budget::frames(2));
    req.stop_at_frame = true;
    assert_eq!(core.run(&mut req).unwrap().reason, StopReason::Frame);
}

/// layers.gb on a DMG with some map entries changed so the tilemap shows
/// more than tile 0, scrolled so the viewport wraps both ways, at a frame end.
pub fn dmg_scene() -> SameBoy {
    let mut core = quick_dmg(LAYERS);
    run(&mut core, Budget::frames(3));
    // A diagonal of tiles 1-5 down the $9800 map.
    for i in 0..32u32 {
        let tile = (i % 5 + 1) as u8;
        core.poke(Region::Vram, 0x1800 + 33 * i, &[tile]).unwrap();
    }
    core.bus_write(0xFF43, 200).unwrap(); // SCX
    core.bus_write(0xFF42, 150).unwrap(); // SCY
    to_frame_end(&mut core);
    core
}

/// The colors the CGB scene sets: BG palette 0 and 1, object palette 0.
pub const CGB_BG: [[u16; 4]; 2] = [
    [0x7FFF, 0x03E0, 0x7C00, 0x0000],
    [0x001F, 0x3DEF, 0x0210, 0x4210],
];
pub const CGB_OBJ0: [u16; 4] = [0x7FFF, 0x7E00, 0x03FF, 0x1084];

/// Sets palette `index` of a set by poking palette RAM.
pub fn set_palette(core: &mut dyn Core, object: bool, index: u8, colors: [u16; 4]) {
    let region = if object {
        Region::ObjPalette
    } else {
        Region::BgPalette
    };
    let bytes: Vec<u8> = colors.iter().flat_map(|c| c.to_le_bytes()).collect();
    core.poke(region, u32::from(index) * 8, &bytes).unwrap();
}

/// layers_cgb.gb in CGB mode with real palettes, at a frame end.
pub fn cgb_scene() -> SameBoy {
    let mut core = fast_cgb(LAYERS_CGB);
    run(&mut core, Budget::frames(2)); // the ROM's setup
    set_palette(&mut core, false, 0, CGB_BG[0]);
    set_palette(&mut core, false, 1, CGB_BG[1]);
    set_palette(&mut core, true, 0, CGB_OBJ0);
    // BG palette 1 for row 0's cell at x=8 (not in layers_cgb.asm), so the
    // map shows two palettes.
    core.poke(Region::Vram, 0x2000 + 0x1800 + 1, &[1]).unwrap();
    run(&mut core, Budget::frames(2));
    to_frame_end(&mut core);
    core
}

fn golden_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/golden")
}

/// Compares `png` with `tests/golden/<name>.png` byte for byte, or rewrites
/// the golden when `GBCLI_BLESS=1`.
pub fn assert_golden(name: &str, png: &[u8]) {
    let path = golden_dir().join(format!("{name}.png"));
    if std::env::var_os("GBCLI_BLESS").is_some_and(|v| v == "1") {
        std::fs::create_dir_all(golden_dir()).unwrap();
        std::fs::write(&path, png).unwrap();
        return;
    }
    let expected = std::fs::read(&path).unwrap_or_else(|err| {
        panic!(
            "golden {} is missing ({err}); after checking the image, create it with\n    \
             GBCLI_BLESS=1 cargo test -p gbcli-view --test golden",
            path.display()
        )
    });
    if expected != png {
        let actual = PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join(format!("{name}.actual.png"));
        std::fs::write(&actual, png).unwrap();
        panic!(
            "{name}.png differs from its golden; the actual image is at {}. If the change is \
             intended, regenerate with GBCLI_BLESS=1 cargo test -p gbcli-view --test golden",
            actual.display()
        );
    }
}
