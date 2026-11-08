//! What the views promise beyond their pixels: frames labeled for what they
//! are, colors as the core outputs them, refusals, and no effect on the machine.

use crate::common;

use common::*;
use gbcli_core::{Budget, Core, IoReg, Pixels, StepUnit, Ticks, VblankKind};
use gbcli_view::{
    DMG_GRAYS, FrameChoice, FrameSource, MapChoice, Rect, TilePalette, ViewError, capture, diff,
    filmstrip, oam_sheet, sheet, tilemap, tileset,
};

fn text_contains(png: &[u8], needle: &str) -> bool {
    png.windows(needle.len()).any(|w| w == needle.as_bytes())
}

#[test]
fn there_is_no_frame_before_the_first_vblank() {
    let core = quick_dmg(BUSY);
    assert!(matches!(
        capture(&core, FrameChoice::Complete),
        Err(ViewError::NoFrame)
    ));
}

#[test]
fn an_lcd_off_frame_is_refused_unless_any_kind_is_asked_for() {
    let mut core = quick_dmg(BUSY);
    run(&mut core, Budget::frames(2));
    core.bus_write(0xFF40, 0x00).unwrap();
    run(&mut core, Budget::frames(2));
    match capture(&core, FrameChoice::Complete) {
        Err(ViewError::NotNormal { tag }) => assert_eq!(tag.kind, VblankKind::LcdOff),
        other => panic!("expected a refusal, got {other:?}"),
    }
    let frame = capture(&core, FrameChoice::CompleteAnyKind).unwrap();
    assert!(matches!(
        frame.source,
        FrameSource::Complete(tag) if tag.kind == VblankKind::LcdOff
    ));
    assert!(text_contains(&frame.image().png().unwrap(), "lcd_off"));
}

#[test]
fn a_complete_frame_is_labeled_with_its_number_and_vblank_kind() {
    let core = dmg_scene();
    let frame = capture(&core, FrameChoice::Complete).unwrap();
    let FrameSource::Complete(tag) = frame.source else {
        panic!("expected a complete frame")
    };
    let label = frame.source.to_string();
    assert_eq!(
        label,
        format!("complete frame {} (vblank normal)", tag.number)
    );
    assert!(text_contains(&frame.image().png().unwrap(), &label));
}

#[test]
fn a_live_frame_is_labeled_partial_with_the_line_reached() {
    let mut core = dmg_scene();
    run(&mut core, Budget::ticks(Ticks::PER_FRAME / 2));
    let frame = capture(&core, FrameChoice::Live).unwrap();
    let FrameSource::Live { ppu, after, .. } = frame.source else {
        panic!("expected a live frame")
    };
    assert_eq!(ppu.ly, core.ppu().ly);
    assert_eq!(after, core.last_complete_frame().map(|f| f.tag.number));
    let label = frame.source.to_string();
    assert!(label.starts_with("live partial frame"), "{label}");
    assert!(label.contains(&format!("LY {}", ppu.ly)), "{label}");
    assert!(text_contains(&frame.image().png().unwrap(), &label));
    // A filmstrip keeps every frame's label.
    let strip = filmstrip(&[frame.clone(), frame]).unwrap();
    assert!(text_contains(&strip.png().unwrap(), "live partial frame"));
}

#[test]
fn dmg_pixels_are_the_cores_shades_as_grays() {
    let core = dmg_scene();
    let image = capture(&core, FrameChoice::Complete).unwrap().image();
    let Pixels::Shades(shades) = core.last_complete_frame().unwrap().pixels else {
        panic!("expected shades")
    };
    let expected: Vec<u32> = shades.iter().map(|&s| DMG_GRAYS[usize::from(s)]).collect();
    assert_eq!(image.pixels, expected);
}

#[test]
fn cgb_pixels_are_the_cores_rgb_unchanged() {
    let core = cgb_scene();
    let image = capture(&core, FrameChoice::Complete).unwrap().image();
    let Pixels::Rgb(rgb) = core.last_complete_frame().unwrap().pixels else {
        panic!("expected RGB")
    };
    assert_eq!(image.pixels, rgb);
}

/// Where the background shows, the map's pixel at the scrolled position is
/// the frame's pixel: the map is colored as the PPU colors. Line 1, because
/// the viewport outline covers the map under line 0 and columns 0 and 159.
fn map_matches_frame_on_line_1(core: &dyn Core, xs: std::ops::Range<u32>) {
    let map = tilemap(core, MapChoice::Background).unwrap();
    let frame = capture(core, FrameChoice::Complete).unwrap().image();
    let scx = u32::from(core.io_register(gbcli_core::IoReg::SCX).unwrap());
    let scy = u32::from(core.io_register(gbcli_core::IoReg::SCY).unwrap());
    for x in xs {
        assert!(x != 0 && x != 159, "on the outline");
        let (mx, my) = ((x + scx) % 256, (1 + scy) % 256);
        assert_eq!(map.pixel(mx, my), frame.pixel(x, 1), "x {x}");
    }
}

#[test]
fn the_dmg_tilemap_is_colored_as_the_frame() {
    let mut core = dmg_scene();
    core.bus_write(0xFF43, 0).unwrap();
    core.bus_write(0xFF42, 1).unwrap();
    core.bus_write(0xFF47, 0x1B).unwrap(); // a non-identity BGP
    to_frame_end(&mut core);
    // Objects cover only x=8-15, 32-39, 56-63 and 72-79 on lines 0-7, and
    // the window starts at x=89.
    map_matches_frame_on_line_1(&core, 16..32);
    map_matches_frame_on_line_1(&core, 80..89);
}

#[test]
fn the_cgb_tilemap_is_colored_as_the_frame() {
    let core = cgb_scene();
    // Lines 0-7: background alone at x=0-7 (palette 0), x=8-15 (palette 1)
    // and x=24-31.
    map_matches_frame_on_line_1(&core, 1..16);
    map_matches_frame_on_line_1(&core, 24..32);
}

#[test]
fn rendering_any_view_never_changes_the_machine() {
    for mut core in [dmg_scene(), cgb_scene()] {
        // Mid-frame, where a live view and the register reads matter most.
        run(&mut core, Budget::ticks(Ticks::PER_FRAME / 3));
        let state = core.save();
        let (now, regs, ppu) = (core.now(), core.registers(), core.ppu());
        let complete = capture(&core, FrameChoice::Complete).unwrap();
        let live = capture(&core, FrameChoice::Live).unwrap();
        complete.image().scaled(4).unwrap().png().unwrap();
        live.image().png().unwrap();
        filmstrip(&[complete.clone(), live.clone()]).unwrap();
        diff(&complete, &live).unwrap();
        tileset(&core, TilePalette::Background(0)).unwrap();
        tileset(&core, TilePalette::Indices).unwrap();
        tilemap(&core, MapChoice::Background).unwrap();
        tilemap(&core, MapChoice::Window).unwrap();
        oam_sheet(&core).unwrap();
        assert_eq!(core.save(), state, "state blob");
        assert_eq!((core.now(), core.registers(), core.ppu()), (now, regs, ppu));
    }
}

#[test]
fn bad_requests_are_refused() {
    let core = dmg_scene();
    let frame = capture(&core, FrameChoice::Complete).unwrap();
    let image = frame.image();
    assert!(matches!(image.scaled(0), Err(ViewError::BadScale { .. })));
    assert!(matches!(image.scaled(17), Err(ViewError::BadScale { .. })));
    let outside = Rect {
        x: 150,
        y: 0,
        width: 20,
        height: 10,
    };
    assert!(matches!(
        image.cropped(outside),
        Err(ViewError::CropOutside { .. })
    ));
    let empty = Rect {
        width: 0,
        ..outside
    };
    assert!(matches!(
        image.cropped(empty),
        Err(ViewError::CropOutside { .. })
    ));
    assert!(matches!(filmstrip(&[]), Err(ViewError::NoFrames)));
    assert!(matches!(
        sheet(std::slice::from_ref(&frame), 0),
        Err(ViewError::BadColumns)
    ));
    let cgb = capture(&cgb_scene(), FrameChoice::Complete).unwrap();
    // Same size, but shades against RGB: not comparable.
    assert!(matches!(
        diff(&frame, &cgb),
        Err(ViewError::Mismatch { .. })
    ));
    assert!(matches!(
        tileset(&core, TilePalette::Object(2)),
        Err(ViewError::Core(_))
    ));
}

#[test]
fn a_frame_diff_counts_and_bounds_the_changed_pixels() {
    let mut core = dmg_scene();
    let before = capture(&core, FrameChoice::Complete).unwrap();
    core.bus_write(0xFF47, 0x1B).unwrap();
    to_frame_end(&mut core);
    to_frame_end(&mut core);
    let after = capture(&core, FrameChoice::Complete).unwrap();
    let (_, same) = diff(&before, &before).unwrap();
    assert_eq!((same.changed, same.bounds), (0, None));
    let (image, summary) = diff(&before, &after).unwrap();
    assert!(summary.changed > 0);
    let bounds = summary.bounds.unwrap();
    assert!(bounds.width <= 160 && bounds.height <= 144);
    assert_eq!((image.width, image.height), (160, 144));
}

#[test]
fn debug_views_draw_while_the_ppu_has_batched_cycles() {
    let mut core = dmg_scene();
    core.bus_write(0xFF47, 0x1B).unwrap();
    // Step into a stretch where the core won't decode BGP without a sync.
    let mut steps = 0;
    while core.io_register(IoReg::BGP).is_ok() {
        assert!(steps < 100_000, "the PPU never batched");
        core.step(StepUnit::Instruction).unwrap();
        steps += 1;
    }
    let views = |core: &dyn Core| {
        (
            tileset(core, TilePalette::Background(0)).unwrap(),
            tilemap(core, MapChoice::Background).unwrap(),
            oam_sheet(core).unwrap(),
        )
    };
    let batched = views(&core);
    // A bus read syncs the PPU; the views must not change.
    core.bus_read(0xFF47).unwrap();
    assert_eq!(core.io_register(IoReg::BGP).unwrap(), 0x1B);
    assert_eq!(views(&core), batched);
}
