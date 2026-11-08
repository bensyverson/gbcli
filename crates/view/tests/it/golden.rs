//! Golden images: every view's PNG bytes, pinned. See `common` for how to
//! regenerate them.

use crate::common;

use common::*;
use gbcli_core::{Budget, Core, Region, Ticks};
use gbcli_view::{
    DEFAULT_SCALE, Frame, FrameChoice, MapChoice, Rect, TilePalette, capture, diff, filmstrip,
    oam_sheet, sheet, tilemap, tileset,
};

fn complete(core: &dyn Core) -> Frame {
    capture(core, FrameChoice::Complete).unwrap()
}

#[test]
fn dmg_frame_at_native_and_default_scale() {
    let core = dmg_scene();
    let frame = complete(&core);
    assert_golden("dmg_frame_1x", &frame.image().png().unwrap());
    let scaled = frame.image().scaled(DEFAULT_SCALE).unwrap();
    assert_eq!((scaled.width, scaled.height), (640, 576));
    assert_golden("dmg_frame_4x", &scaled.png().unwrap());
}

#[test]
fn dmg_frame_cropped_then_scaled() {
    let core = dmg_scene();
    let crop = Rect {
        x: 0,
        y: 0,
        width: 96,
        height: 16,
    };
    let image = complete(&core).image().cropped(crop).unwrap();
    assert_golden("dmg_crop_4x", &image.scaled(4).unwrap().png().unwrap());
}

#[test]
fn dmg_live_partial_frame() {
    let mut core = dmg_scene();
    core.bus_write(0xFF47, 0x1B).unwrap(); // BGP reversed for the frame in progress
    run(&mut core, Budget::ticks(Ticks::PER_FRAME / 2));
    let live = capture(&core, FrameChoice::Live).unwrap();
    assert_golden(
        "dmg_live_2x",
        &live.image().scaled(2).unwrap().png().unwrap(),
    );
}

/// Four frames with object 0 moving right, collected by the test: the view
/// never runs the core.
fn moving_frames() -> Vec<Frame> {
    let mut core = dmg_scene();
    (0..4u8)
        .map(|i| {
            core.poke(Region::Oam, 1, &[8 + 8 + 12 * i]).unwrap(); // object 0's X
            run(&mut core, Budget::frames(1));
            to_frame_end(&mut core);
            complete(&core)
        })
        .collect()
}

#[test]
fn dmg_filmstrip_and_atlas() {
    let frames = moving_frames();
    assert_golden("dmg_filmstrip", &filmstrip(&frames).unwrap().png().unwrap());
    assert_golden("dmg_atlas_2x2", &sheet(&frames, 2).unwrap().png().unwrap());
}

#[test]
fn dmg_frame_diff() {
    let frames = moving_frames();
    let (image, summary) = diff(&frames[0], &frames[3]).unwrap();
    assert!(summary.changed > 0);
    assert_golden("dmg_diff_2x", &image.scaled(2).unwrap().png().unwrap());
}

#[test]
fn dmg_tileset_tilemap_and_oam() {
    let core = dmg_scene();
    let tiles = tileset(&core, TilePalette::Background(0)).unwrap();
    assert_eq!((tiles.width, tiles.height), (128, 192));
    assert_golden("dmg_tileset_2x", &tiles.scaled(2).unwrap().png().unwrap());
    let map = tilemap(&core, MapChoice::Background).unwrap();
    assert_eq!((map.width, map.height), (256, 256));
    assert_golden("dmg_tilemap", &map.png().unwrap());
    let oam = oam_sheet(&core).unwrap();
    assert_golden("dmg_oam_4x", &oam.scaled(4).unwrap().png().unwrap());
}

#[test]
fn cgb_frame_tileset_tilemap_and_oam() {
    let core = cgb_scene();
    assert_golden("cgb_frame_1x", &complete(&core).image().png().unwrap());
    let tiles = tileset(&core, TilePalette::Background(1)).unwrap();
    assert_eq!((tiles.width, tiles.height), (256, 192), "both VRAM banks");
    assert_golden("cgb_tileset", &tiles.png().unwrap());
    assert_golden(
        "cgb_tilemap",
        &tilemap(&core, MapChoice::Background)
            .unwrap()
            .png()
            .unwrap(),
    );
    assert_golden(
        "cgb_oam_4x",
        &oam_sheet(&core).unwrap().scaled(4).unwrap().png().unwrap(),
    );
}
