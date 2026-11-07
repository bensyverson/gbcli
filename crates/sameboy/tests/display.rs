//! The live framebuffer, the color mode and the core's own CGB color output.

mod common;

use common::*;
use gbcli_core::{
    BootRom, Budget, ColorMode, Core, CoreConfig, CoreError, Feature, IoReg, Model, PixelBuf,
    Pixels, Region, RunRequest, StopReason, Ticks,
};
use gbcli_sameboy::SameBoy;

const LAYERS_CGB: &[u8] = include_bytes!("roms/layers_cgb.gb");

/// Runs to the end of the next frame.
fn to_frame_end(core: &mut SameBoy) {
    let mut req = RunRequest::new(Budget::frames(2));
    req.stop_at_frame = true;
    assert_eq!(core.run(&mut req).unwrap().reason, StopReason::Frame);
}

fn live_shades(core: &SameBoy) -> Vec<u8> {
    match core.live_frame().expect("a live frame").pixels {
        PixelBuf::Shades(shades) => shades,
        PixelBuf::Rgb(_) => panic!("DMG live frame came out as RGB"),
    }
}

/// A CGB core on `rom` with the fast CGB boot ROM, handed over.
fn cgb(rom: &[u8]) -> SameBoy {
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

#[test]
fn at_a_frame_end_the_live_frame_is_the_complete_one() {
    let mut core = quick_dmg(BUSY);
    run(&mut core, Budget::frames(3));
    to_frame_end(&mut core);
    let live = core.live_frame().unwrap();
    assert_eq!((live.width, live.height), (160, 144));
    let Pixels::Shades(complete) = core.last_complete_frame().unwrap().pixels else {
        panic!("expected shades")
    };
    assert_eq!(live_shades(&core), complete);
}

#[test]
fn mid_frame_the_live_frame_has_new_lines_above_ly_and_old_ones_below() {
    let mut core = quick_dmg(BUSY);
    run(&mut core, Budget::frames(3));
    to_frame_end(&mut core);
    let Pixels::Shades(complete) = core.last_complete_frame().unwrap().pixels else {
        panic!("expected shades")
    };
    let complete = complete.to_vec();
    assert!(complete.iter().any(|&s| s != 3), "busy.gb draws a pattern");
    core.bus_write(0xFF47, 0xFF).unwrap(); // every color index to shade 3
    run(&mut core, Budget::ticks(Ticks::PER_FRAME / 2));
    let ly = usize::from(core.ppu().ly);
    assert!((10..130).contains(&ly), "LY {ly}");
    let live = live_shades(&core);
    let drawn = 160 * (ly - 1);
    assert!(live[..drawn].iter().all(|&s| s == 3), "drawn lines");
    let old = 160 * (ly + 1);
    assert_eq!(live[old..], complete[old..], "old lines");
    let Pixels::Shades(kept) = core.last_complete_frame().unwrap().pixels else {
        panic!("expected shades")
    };
    assert_eq!(kept, complete, "the kept frame is untouched");
}

#[test]
fn reading_the_live_frame_changes_no_state() {
    let mut core = quick_dmg(BUSY);
    run(&mut core, Budget::ticks(Ticks::PER_FRAME * 5 / 2));
    let before = core.save();
    core.live_frame().unwrap();
    core.color_mode().unwrap();
    assert_eq!(core.save(), before);
}

#[test]
fn color_modes_follow_the_model_and_the_cartridge() {
    assert_eq!(quick_dmg(BUSY).color_mode().unwrap(), ColorMode::DmgShades);
    assert_eq!(
        cgb(LAYERS_CGB).color_mode().unwrap(),
        ColorMode::CgbPalettes
    );
    assert_eq!(cgb(BUSY).color_mode().unwrap(), ColorMode::CgbDmgCompat);
    let sgb = SameBoy::new(CoreConfig::new(Model::Sgb, BUSY.to_vec())).unwrap();
    assert_eq!(sgb.color_mode().unwrap(), ColorMode::Sgb);
}

#[test]
fn a_cgb_color_converts_as_the_frame_shows_it() {
    let mut core = cgb(LAYERS_CGB);
    let color: u16 = 0b0_00111_10101_01010; // blue 7, green 21, red 10
    let [lo, hi] = color.to_le_bytes();
    // Through BCPS/BCPD, as a game sets it.
    core.bus_write(0xFF68, 0x80).unwrap();
    core.bus_write(0xFF69, lo).unwrap();
    core.bus_write(0xFF69, hi).unwrap();
    // The ROM turns the LCD off for its setup; SameBoy blanks the first frame
    // after it turns back on.
    run(&mut core, Budget::frames(3));
    let Pixels::Rgb(frame) = core.last_complete_frame().unwrap().pixels else {
        panic!("expected RGB")
    };
    // Line 0, x=0: background tile 0, color 0, palette 0 (layers_cgb.asm).
    assert_eq!(core.rgb555_color(color).unwrap(), frame[0]);
    let PixelBuf::Rgb(live) = core.live_frame().unwrap().pixels else {
        panic!("CGB live frame came out as shades")
    };
    assert_eq!(live.len(), 160 * 144);
}

#[test]
fn a_poked_cgb_palette_color_appears_in_the_next_frame() {
    let mut core = cgb(LAYERS_CGB);
    run(&mut core, Budget::frames(3)); // the ROM's setup
    let bg: u16 = 0b0_00111_10101_01010;
    let obj: u16 = 0b0_10100_00011_11001;
    // BG palette 0, color 0; object palette 0, color 3 (tile 2's color).
    core.poke(Region::BgPalette, 0, &bg.to_le_bytes()).unwrap();
    core.poke(Region::ObjPalette, 6, &obj.to_le_bytes())
        .unwrap();
    to_frame_end(&mut core);
    let Pixels::Rgb(frame) = core.last_complete_frame().unwrap().pixels else {
        panic!("expected RGB")
    };
    // Line 0, x=0: background tile 0, color 0, palette 0 (layers_cgb.asm).
    assert_eq!(frame[0], core.rgb555_color(bg).unwrap(), "background");
    // Line 8, x=26: object 3, tile 2, color 3, palette 0.
    assert_eq!(
        frame[8 * 160 + 26],
        core.rgb555_color(obj).unwrap(),
        "object"
    );
}

#[test]
fn a_dmg_core_has_no_cgb_colors() {
    let core = quick_dmg(BUSY);
    assert!(matches!(
        core.rgb555_color(0x7FFF),
        Err(CoreError::Unsupported {
            feature: Feature::DisplayColors,
            ..
        })
    ));
}
