//! Every inspection call leaves the state unchanged (plan, fidelity rule 1).
//!
//! The criterion says "hash"; comparing the whole state blob byte for byte is
//! the stronger check and needs no hash function.

mod common;

use common::*;
use gbcli_core::{Budget, Core, Feature, IoReg, Region};

const REGIONS: [Region; 11] = [
    Region::Rom,
    Region::Wram,
    Region::Sram,
    Region::Vram,
    Region::Hram,
    Region::Oam,
    Region::Io,
    Region::Ie,
    Region::BootRom,
    Region::BgPalette,
    Region::ObjPalette,
];

/// Calls every inspection method, asserting the state blob is unchanged after each.
fn assert_inspection_is_pure(core: &mut dyn Core, at: &str) {
    let mut check = |what: &str, call: &dyn Fn(&dyn Core)| {
        let before = core.save();
        call(core);
        assert!(core.save() == before, "{what} changed the state ({at})");
    };
    check("info", &|core| {
        core.info();
    });
    check("supports", &|core| {
        core.supports(Feature::PpuDot);
    });
    check("now", &|core| {
        core.now();
    });
    check("registers", &|core| {
        core.registers();
    });
    check("ppu", &|core| {
        core.ppu();
    });
    check("last_complete_frame", &|core| {
        core.last_complete_frame();
    });
    check("pixel_layers", &|core| {
        let _ = core.pixel_layers();
    });
    check("apu", &|core| {
        let _ = core.apu();
    });
    for region in REGIONS {
        check(&format!("region {region:?}"), &|core| {
            let _ = core.region(region);
        });
        check(&format!("written {region:?}"), &|core| {
            let _ = core.written(region);
        });
    }
    for reg in 0x00..=0x7F {
        check(&format!("io_register {reg:#04x}"), &|core| {
            let _ = core.io_register(IoReg(reg));
        });
    }
}

#[test]
fn inspection_never_changes_the_state_at_random_points() {
    let mut core = quick_dmg(BUSY);
    let mut rng = Rng::new(0x1A5);
    run(&mut core, Budget::frames(2));
    for i in 0..12 {
        run(&mut core, Budget::instructions(1 + rng.below(3_000)));
        let at = format!("point {i}, ly {}", core.ppu().ly);
        assert_inspection_is_pure(&mut core, &at);
    }
}

#[test]
fn inspection_never_changes_the_state_during_the_boot_rom() {
    let mut core = dmg(BUSY, 3);
    let mut rng = Rng::new(0xB007);
    for i in 0..4 {
        run(&mut core, Budget::instructions(1 + rng.below(100_000)));
        assert_inspection_is_pure(&mut core, &format!("boot point {i}"));
    }
}
