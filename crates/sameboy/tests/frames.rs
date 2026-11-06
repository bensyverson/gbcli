//! The last complete frame: kept apart from the live buffer, tagged, raw shades on DMG.

mod common;

use common::*;
use gbcli_core::{
    Budget, Core, CoreConfig, IoReg, Model, Pixels, Rendering, StopReason, VblankKind,
};
use gbcli_sameboy::SameBoy;

fn shades(core: &SameBoy) -> Vec<u8> {
    match core.last_complete_frame().expect("a frame").pixels {
        Pixels::Shades(shades) => shades.to_vec(),
        Pixels::Rgb(_) => panic!("DMG frame came out as RGB"),
    }
}

#[test]
fn there_is_no_frame_before_the_first_vblank() {
    let core = dmg(BUSY, 0);
    assert!(core.last_complete_frame().is_none());
}

#[test]
fn a_dmg_frame_is_160_by_144_raw_shades_tagged_normal() {
    let mut core = quick_dmg(BUSY);
    let stop = run(&mut core, Budget::frames(3));
    let frame = core.last_complete_frame().unwrap();
    assert_eq!((frame.width, frame.height), (160, 144));
    assert_eq!(frame.tag.kind, VblankKind::Normal);
    assert_eq!(frame.tag.number, 3);
    assert!(frame.tag.at <= stop.at);
    let Pixels::Shades(pixels) = frame.pixels else {
        panic!("expected shades")
    };
    assert_eq!(pixels.len(), 160 * 144);
    assert!(pixels.iter().all(|&s| s <= 3));
}

#[test]
fn shades_follow_bgp_not_a_display_palette() {
    let mut core = quick_dmg(BUSY);
    run(&mut core, Budget::frames(2));
    core.bus_write(0xFF47, 0x00).unwrap(); // every color index to shade 0
    run(&mut core, Budget::frames(2));
    assert!(shades(&core).iter().all(|&s| s == 0));
    core.bus_write(0xFF47, 0xFF).unwrap(); // every color index to shade 3
    run(&mut core, Budget::frames(2));
    assert!(shades(&core).iter().all(|&s| s == 3));
}

#[test]
fn a_frame_with_the_lcd_off_is_tagged_lcd_off() {
    let mut core = quick_dmg(BUSY);
    run(&mut core, Budget::frames(2));
    core.bus_write(0xFF40, 0x00).unwrap();
    run(&mut core, Budget::frames(2));
    assert_eq!(
        core.last_complete_frame().unwrap().tag.kind,
        VblankKind::LcdOff
    );
}

#[test]
fn the_kept_frame_is_not_the_live_buffer() {
    let mut core = quick_dmg(BUSY);
    let mut req = gbcli_core::RunRequest::new(Budget::frames(5));
    req.stop_at_frame = true;
    assert_eq!(core.run(&mut req).unwrap().reason, StopReason::Frame);
    let kept = shades(&core);
    let number = core.last_complete_frame().unwrap().tag.number;
    core.bus_write(0xFF47, if kept[0] == 0 { 0xFF } else { 0x00 })
        .unwrap();
    // Half a frame later the live buffer has new lines; the kept frame hasn't.
    run(&mut core, Budget::ticks(gbcli_core::Ticks::PER_FRAME / 2));
    assert_eq!(core.last_complete_frame().unwrap().tag.number, number);
    assert_eq!(shades(&core), kept);
}

#[test]
fn a_cgb_frame_is_rgb() {
    let mut core = SameBoy::new(CoreConfig::new(Model::CgbE, BUSY.to_vec())).unwrap();
    run(&mut core, Budget::frames(400));
    assert_eq!(core.io_register(IoReg::BANK).unwrap() & 1, 1);
    let frame = core.last_complete_frame().unwrap();
    assert_eq!((frame.width, frame.height), (160, 144));
    assert!(matches!(frame.pixels, Pixels::Rgb(p) if p.len() == 160 * 144));
}

#[test]
fn rendering_off_keeps_no_frame_but_still_counts_vblanks() {
    let mut config = CoreConfig::new(Model::DmgB, BUSY.to_vec());
    config.rendering = Rendering::Off;
    let mut core = SameBoy::new(config).unwrap();
    let stop = run(&mut core, Budget::frames(3));
    assert_eq!(stop.used.frames, 3);
    assert!(core.last_complete_frame().is_none());
}
