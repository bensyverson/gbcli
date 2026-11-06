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
fn assert_inspection_is_pure(core: &dyn Core, at: &str) {
    let check = |what: &str, call: &dyn Fn()| {
        let before = core.save();
        call();
        assert!(core.save() == before, "{what} changed the state ({at})");
    };
    check("info", &|| {
        core.info();
    });
    check("supports", &|| {
        core.supports(Feature::PpuDot);
    });
    check("now", &|| {
        core.now();
    });
    check("registers", &|| {
        core.registers();
    });
    check("ppu", &|| {
        core.ppu();
    });
    check("last_complete_frame", &|| {
        core.last_complete_frame();
    });
    check("pixel_layers", &|| {
        let _ = core.pixel_layers();
    });
    for region in REGIONS {
        check(&format!("region {region:?}"), &|| {
            let _ = core.region(region);
        });
        check(&format!("written {region:?}"), &|| {
            let _ = core.written(region);
        });
    }
    for reg in 0x00..=0x7F {
        check(&format!("io_register {reg:#04x}"), &|| {
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
        assert_inspection_is_pure(&core, &format!("point {i}, ly {}", core.ppu().ly));
    }
}

#[test]
fn inspection_never_changes_the_state_during_the_boot_rom() {
    let mut core = dmg(BUSY, 3);
    let mut rng = Rng::new(0xB007);
    for i in 0..4 {
        run(&mut core, Budget::instructions(1 + rng.below(100_000)));
        assert_inspection_is_pure(&core, &format!("boot point {i}"));
    }
}
