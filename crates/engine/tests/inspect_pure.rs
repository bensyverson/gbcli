//! Every engine inspection call leaves the core's state blob byte-for-byte
//! unchanged, and the CPU and APU views carry what the core reports.

mod common;

use common::*;
use gbcli_core::{ApuChannel, Budget, Core};
use gbcli_engine::inspect::{
    apu_state, cpu,
    hwreg::read_all,
    regions::{ALL_REGIONS, read, region_info},
    video::{
        MapArea, ObjectHeight, PaletteSet, TileAddressing, cgb_palettes, oam, object_height, tile,
        tilemap,
    },
};

fn assert_pure(core: &mut dyn Core, at: &str) {
    let mut check = |what: &str, call: &dyn Fn(&dyn Core)| {
        let before = core.save();
        call(core);
        assert!(core.save() == before, "{what} changed the state ({at})");
    };
    check("cpu", &|core| {
        cpu(core);
    });
    check("apu_state", &|core| {
        let _ = apu_state(core);
    });
    check("hwreg read_all", &|core| {
        read_all(core);
    });
    for region in ALL_REGIONS {
        check(&format!("region_info {region:?}"), &|core| {
            let _ = region_info(core, region);
        });
        check(&format!("read {region:?}"), &|core| {
            let _ = read(core, region, None, 0, 1);
        });
    }
    check("video", &|core| {
        let _ = tile(core, 0, 1);
        let _ = tilemap(core, MapArea::Map9800, TileAddressing::Signed8800);
        let _ = object_height(core);
        let _ = oam(core, ObjectHeight::Sixteen);
        let _ = cgb_palettes(core, PaletteSet::Object);
    });
}

#[test]
fn engine_inspection_never_changes_the_state() {
    let mut dmg = quick_dmg(BUSY);
    let mut cgb = booted_cgb(BUSY);
    let mut rng = Rng::new(0x9E7E);
    for i in 0..8 {
        run(&mut dmg, Budget::instructions(1 + rng.below(3_000)));
        assert_pure(&mut dmg, &format!("dmg point {i}"));
        run(&mut cgb, Budget::instructions(1 + rng.below(3_000)));
        assert_pure(&mut cgb, &format!("cgb point {i}"));
    }
}

#[test]
fn the_cpu_view_decodes_flags_and_pairs() {
    let mut core = quick_dmg(BUSY);
    let mut regs = core.registers();
    regs.a = 0x12;
    regs.f = 0b1010_0000; // Z and H
    regs.h = 0xC0;
    regs.l = 0x01;
    regs.ime = true;
    core.set_registers(&regs);
    let view = cpu(&core);
    assert_eq!((view.af, view.hl), (0x12A0, 0xC001));
    assert!(view.flags.z && !view.flags.n && view.flags.h && !view.flags.c);
    assert!(view.registers.ime && !view.registers.halted);
}

#[test]
fn the_apu_view_is_the_cores_channel_state() {
    let mut core = quick_dmg(BUSY);
    core.bus_write(0xFF26, 0x80).unwrap();
    core.bus_write(0xFF22, 0x25).unwrap();
    let apu = apu_state(&core).unwrap();
    assert_eq!(apu, core.apu().unwrap());
    assert_eq!(apu.channel(ApuChannel::Noise).period, 20);
}
