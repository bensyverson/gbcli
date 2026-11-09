//! Per-pixel layer identity: capture is observation-only, and the tags name the
//! layer SameBoy actually mixed: background, window, or an OAM object.
//!
//! `layers.gb` (crates/sameboy/tests/roms/layers.asm) pins the DMG cases and
//! gives every layer its own shade, so a whole map can be checked against the
//! pixels; `layers_cgb.gb` pins the CGB priority rules. dmg-acid2 checks that
//! the tags stay consistent with OAM and the window over a real PPU
//! conformance ROM.

use crate::common;

use common::*;
use gbcli_core::{
    BootRom, Budget, Core, CoreConfig, CoreError, Feature, IoReg, Layer, LayerMap, Model, Pixels,
    PpuMode, Region, Rendering, Support,
};
use gbcli_sameboy::SameBoy;

const LAYERS: &[u8] = include_bytes!("../roms/layers.gb");
const LAYERS_CGB: &[u8] = include_bytes!("../roms/layers_cgb.gb");

const SCREEN_WIDTH: usize = 160;
const SCREEN_HEIGHT: usize = 144;
/// Frames `layers.gb` needs for its setup (two 1 KiB map fills and OAM), the
/// blank frame SameBoy substitutes after the LCD turns on, and a captured one.
const SETTLE_FRAMES: u64 = 10;
/// The line on which `layers.gb` turns the window off partway through.
const WINDOW_CUT_LINE: usize = 100;

fn tag(map: &LayerMap<'_>, x: usize, y: usize) -> Layer {
    map.layers[x + y * SCREEN_WIDTH]
}

fn shades(core: &SameBoy) -> Vec<u8> {
    match core.last_complete_frame().expect("a frame").pixels {
        Pixels::Shades(shades) => shades.to_vec(),
        Pixels::Rgb(_) => panic!("DMG frame came out as RGB"),
    }
}

/// `layers.gb` gives each layer its own shade: background 0 or 2, window 1,
/// objects 3. Every tag must agree with the pixel it describes.
fn assert_map_matches_pixels(core: &SameBoy, context: &str) {
    let map = core.pixel_layers().unwrap();
    let shades = shades(core);
    assert_eq!((map.width, map.height), (160, 144), "{context}");
    assert_eq!(map.layers.len(), shades.len(), "{context}");
    for (i, (&layer, &shade)) in map.layers.iter().zip(&shades).enumerate() {
        let consistent = match layer {
            Layer::Background => shade == 0 || shade == 2,
            Layer::Window => shade == 1,
            Layer::Object { .. } => shade == 3,
        };
        assert!(
            consistent,
            "{context}: ({},{}) is tagged {layer:?} over shade {shade}",
            i % SCREEN_WIDTH,
            i / SCREEN_WIDTH
        );
    }
}

/// A CGB core on `rom` with the quick boot ROM, already handed over; SameBoy
/// starts CGB models in CGB mode with index-based object priority.
fn quick_cgb(rom: &[u8]) -> SameBoy {
    let mut config = CoreConfig::new(Model::CgbE, rom.to_vec());
    config.boot_rom = BootRom::Custom(QUICK_BOOT.to_vec());
    let mut core = SameBoy::new(config).expect("construct core");
    run(&mut core, Budget::instructions(4));
    assert_eq!(core.registers().pc, 0x0100, "quick boot didn't hand over");
    core
}

/// `layers.gb` past its setup, with capture on.
fn settled_layers(busy: bool) -> SameBoy {
    let mut core = quick_dmg(LAYERS);
    select_loop(&mut core, busy);
    core.set_pixel_layers(true).unwrap();
    run(&mut core, Budget::frames(SETTLE_FRAMES));
    core
}

/// `layers.gb`'s main loop reads $C000, which power-on randomizes: pin it.
/// The busy loop reads VRAM, which syncs the PPU and forces the slow,
/// per-pixel path; the quiet one lets mode 3 batch (render_line).
/// `busy_rom_takes_the_per_pixel_path` checks both.
fn select_loop(core: &mut SameBoy, busy: bool) {
    core.poke(Region::Wram, 0, &[u8::from(busy)]).unwrap();
}

#[test]
fn the_mixing_point_tags_sprite_transparency_priority_and_window() {
    for busy in [false, true] {
        let mut core = quick_dmg(LAYERS);
        select_loop(&mut core, busy);
        core.set_pixel_layers(true).unwrap();
        let map = core.pixel_layers();
        assert!(
            matches!(map, Err(CoreError::InvalidState { .. })),
            "a map before the first captured frame: {map:?}"
        );
        run(&mut core, Budget::frames(SETTLE_FRAMES));
        let map = core.pixel_layers().unwrap();
        assert_eq!((map.width, map.height), (160, 144));
        assert_eq!(map.layers.len(), SCREEN_WIDTH * SCREEN_HEIGHT);

        // Object 0 is opaque over the background, for all eight rows.
        for y in 0..8 {
            assert_eq!(tag(&map, 8, y), Layer::Object { index: 0 }, "busy={busy}");
        }
        // Object 1's leftmost pixel is transparent: the background shows.
        assert_eq!(tag(&map, 32, 0), Layer::Background, "busy={busy}");
        assert_eq!(tag(&map, 33, 0), Layer::Object { index: 1 }, "busy={busy}");
        // Object 2 has the BG-priority bit and the BG pixel is color 2, so the
        // background wins the whole sprite.
        for x in 56..64 {
            assert_eq!(tag(&map, x, 0), Layer::Background, "busy={busy}");
        }
        // Object 3 has the BG-priority bit too, but over BG color 0 it wins.
        for x in 72..80 {
            assert_eq!(tag(&map, x, 0), Layer::Object { index: 3 }, "busy={busy}");
        }
        // WX=96 starts the window at x=89; x=88 is still background.
        assert_eq!(tag(&map, 88, 0), Layer::Background, "busy={busy}");
        assert_eq!(tag(&map, 89, 0), Layer::Window, "busy={busy}");
        assert_eq!(tag(&map, 159, 0), Layer::Window, "busy={busy}");
        assert_map_matches_pixels(&core, &format!("busy={busy}"));
    }
}

#[test]
fn dmg_overlapping_objects_resolve_by_x_then_oam_index() {
    for busy in [false, true] {
        let core = settled_layers(busy);
        let map = core.pixel_layers().unwrap();
        for y in 8..16 {
            for x in 16..24 {
                // Object 5 (X 16) beats object 4 (X 20) despite its higher index.
                assert_eq!(
                    tag(&map, x, y),
                    Layer::Object { index: 5 },
                    "({x},{y}) busy={busy}"
                );
            }
            for x in 24..28 {
                assert_eq!(
                    tag(&map, x, y),
                    Layer::Object { index: 4 },
                    "({x},{y}) busy={busy}"
                );
            }
            for x in 40..48 {
                // Objects 6 and 7 share X: the lower OAM index wins.
                assert_eq!(
                    tag(&map, x, y),
                    Layer::Object { index: 6 },
                    "({x},{y}) busy={busy}"
                );
            }
        }
    }
}

#[test]
fn turning_the_window_off_mid_line_ends_its_tags_there() {
    for busy in [false, true] {
        let core = settled_layers(busy);
        let map = core.pixel_layers().unwrap();
        let y = WINDOW_CUT_LINE;
        let row: Vec<Layer> = (0..SCREEN_WIDTH).map(|x| tag(&map, x, y)).collect();
        assert!(
            row[..89].iter().all(|&l| l == Layer::Background),
            "busy={busy}: {row:?}"
        );
        assert_eq!(row[89], Layer::Window, "busy={busy}");
        let cut = (89..SCREEN_WIDTH)
            .find(|&x| row[x] != Layer::Window)
            .expect("the window was turned off before the end of the line");
        assert!(cut > 89, "busy={busy}");
        assert!(
            row[cut..].iter().all(|&l| l == Layer::Background),
            "busy={busy}: background after the cut at {cut}: {row:?}"
        );
        // The rest of the frame has no window until the vblank handler.
        for y in WINDOW_CUT_LINE + 1..SCREEN_HEIGHT {
            assert_eq!(tag(&map, 159, y), Layer::Background, "line {y} busy={busy}");
        }
        assert_map_matches_pixels(&core, &format!("busy={busy}"));
    }
}

#[test]
fn with_objects_off_no_pixel_is_tagged_as_an_object() {
    let mut core = settled_layers(false);
    core.bus_write(0xFF40, 0xF1).unwrap(); // LCDC.1 off; the ROM keeps it so
    run(&mut core, Budget::frames(2));
    let map = core.pixel_layers().unwrap();
    assert!(
        !map.layers.iter().any(|l| matches!(l, Layer::Object { .. })),
        "an object tag with LCDC.1 off"
    );
    assert_eq!(tag(&map, 8, 0), Layer::Background);
    assert_map_matches_pixels(&core, "objects off");
}

#[test]
fn busy_rom_takes_the_per_pixel_path() {
    // While the PPU batches mode 3, its cycles pile up unrun (HDMA5 can't be
    // read without a sync; STAT can, since the batch's mode is known) until
    // render_line draws the whole line at once; at most the instant the batch
    // begins reads as synced. On the per-pixel
    // path every instruction boundary in mode 3 reads as synced. So a line
    // with several synced mode-3 observations was drawn pixel by pixel.
    // Lines from WINDOW_CUT_LINE on are left out: the LCDC write there syncs
    // the PPU in either loop.
    let per_pixel_lines = |busy: bool| {
        let mut core = settled_layers(busy);
        let mut synced_in_mode_3 = [0u32; WINDOW_CUT_LINE];
        let start = core.now();
        while core.now().0 - start.0 < 140_448 {
            run(&mut core, Budget::instructions(1));
            let ppu = core.ppu();
            let synced = core.io_register(IoReg::HDMA5).is_ok();
            if synced && ppu.mode == Some(PpuMode::Drawing) && usize::from(ppu.ly) < WINDOW_CUT_LINE
            {
                synced_in_mode_3[usize::from(ppu.ly)] += 1;
            }
        }
        synced_in_mode_3.iter().filter(|&&n| n >= 3).count()
    };
    assert_eq!(
        per_pixel_lines(false),
        0,
        "the quiet loop lets mode 3 batch"
    );
    assert_eq!(
        per_pixel_lines(true),
        WINDOW_CUT_LINE,
        "the VRAM-reading loop forces the per-pixel path on every line"
    );
}

/// A CGB core on `rom` with the fast CGB boot ROM, handed over; unlike the
/// quick boot ROM, it hands over with the LCD on.
fn fast_cgb(rom: &[u8]) -> SameBoy {
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
fn cgb_priorities_attribute_bit_oam_flag_and_index() {
    for (boot, core) in [
        ("quick", quick_cgb(LAYERS_CGB)),
        ("fast", fast_cgb(LAYERS_CGB)),
    ] {
        assert_cgb_priorities(core, boot);
    }
}

fn assert_cgb_priorities(mut core: SameBoy, boot: &str) {
    core.set_pixel_layers(true).unwrap();
    run(&mut core, Budget::frames(SETTLE_FRAMES));
    let map = core.pixel_layers().unwrap();
    for y in 0..8 {
        for x in 16..24 {
            // The BG attribute's priority bit hides object 0 (BG color 2).
            assert_eq!(tag(&map, x, y), Layer::Background, "{boot} boot: ({x},{y})");
        }
        for x in 32..40 {
            // The attribute priority bit loses to an object over BG color 0.
            assert_eq!(
                tag(&map, x, y),
                Layer::Object { index: 1 },
                "{boot} boot: ({x},{y})"
            );
        }
        for x in 48..56 {
            // Object 2's own BG-priority flag, over BG color 2.
            assert_eq!(tag(&map, x, y), Layer::Background, "{boot} boot: ({x},{y})");
        }
    }
    for y in 8..16 {
        for x in 20..24 {
            assert_eq!(
                tag(&map, x, y),
                Layer::Object { index: 4 },
                "{boot} boot: ({x},{y})"
            );
        }
        for x in 24..32 {
            // CGB priority is by OAM index alone: object 3 beats object 4,
            // whose X is lower.
            assert_eq!(
                tag(&map, x, y),
                Layer::Object { index: 3 },
                "{boot} boot: ({x},{y})"
            );
        }
    }
}

#[test]
fn cgb_lcdc0_off_gives_objects_master_priority() {
    let mut core = quick_cgb(LAYERS_CGB);
    core.set_pixel_layers(true).unwrap();
    run(&mut core, Budget::frames(SETTLE_FRAMES));
    core.bus_write(0xFF40, 0x92).unwrap(); // LCDC.0 off
    run(&mut core, Budget::frames(2));
    let map = core.pixel_layers().unwrap();
    for y in 0..8 {
        for (x0, index) in [(16, 0), (32, 1), (48, 2)] {
            for x in x0..x0 + 8 {
                assert_eq!(tag(&map, x, y), Layer::Object { index }, "({x},{y})");
            }
        }
        // Where no object is, the background is still drawn and tagged.
        assert_eq!(tag(&map, 100, y), Layer::Background);
    }
}

#[test]
fn a_mid_frame_load_publishes_no_stale_tags() {
    let mut saved = settled_layers(false);
    // Into the middle of a frame, past object 0's rows.
    loop {
        run(&mut saved, Budget::instructions(1));
        let ppu = saved.ppu();
        if (72..SCREEN_HEIGHT as u8).contains(&ppu.ly) && ppu.mode != Some(PpuMode::VBlank) {
            break;
        }
    }
    let state = saved.save();

    // Another core, capturing, whose buffer holds another timeline: here the
    // LCD-off frames before `layers.gb` turns the LCD on.
    let mut core = quick_dmg(LAYERS);
    core.set_pixel_layers(true).unwrap();
    run(&mut core, Budget::frames(1));
    core.load(&state).unwrap();
    run(&mut core, Budget::frames(1));
    // The frame the load landed in was only partly drawn on this timeline: it
    // may have no map, but a map it has must describe its pixels.
    if core.pixel_layers().is_ok() {
        assert_map_matches_pixels(&core, "the frame the load landed in");
    }
    run(&mut core, Budget::frames(1));
    assert_map_matches_pixels(&core, "the frame after it");
    assert_eq!(
        tag(&core.pixel_layers().unwrap(), 8, 0),
        Layer::Object { index: 0 }
    );
}

#[test]
fn a_load_at_a_frame_boundary_maps_the_next_frame() {
    let mut saved = settled_layers(false);
    run(&mut saved, Budget::frames(1));
    assert_eq!(saved.ppu().mode, Some(PpuMode::VBlank));
    let state = saved.save();

    let mut core = quick_dmg(LAYERS);
    core.set_pixel_layers(true).unwrap();
    run(&mut core, Budget::frames(1));
    core.load(&state).unwrap();
    run(&mut core, Budget::frames(1));
    assert_map_matches_pixels(&core, "the first frame after a load at vblank");
}

#[test]
fn capture_turned_on_at_a_vblank_maps_the_next_frame() {
    let mut core = quick_dmg(LAYERS);
    select_loop(&mut core, false);
    run(&mut core, Budget::frames(SETTLE_FRAMES));
    assert_eq!(core.ppu().mode, Some(PpuMode::VBlank));
    core.set_pixel_layers(true).unwrap();
    run(&mut core, Budget::frames(1));
    assert_map_matches_pixels(&core, "the first frame captured whole");
}

#[test]
fn turning_capture_on_again_keeps_the_current_map() {
    let mut core = settled_layers(false);
    let before = core.pixel_layers().unwrap().layers.to_vec();
    core.set_pixel_layers(true).unwrap();
    let after = core.pixel_layers().expect("capture was already on");
    assert_eq!(after.layers, &before[..]);
}

#[test]
fn rendering_off_refuses_capture_with_a_remedy() {
    let mut config = CoreConfig::new(Model::DmgB, LAYERS.to_vec());
    config.rendering = Rendering::Off;
    let mut core = SameBoy::new(config).unwrap();
    assert!(matches!(
        core.supports(Feature::PixelLayers),
        Support::No { reason } if reason.contains("Rendering::On")
    ));
    match core.set_pixel_layers(true) {
        Err(CoreError::Unsupported {
            feature: Feature::PixelLayers,
            reason,
        }) => assert!(reason.contains("Rendering::On"), "{reason}"),
        other => panic!("capture with rendering off: {other:?}"),
    }
    // Turning it off is always allowed.
    core.set_pixel_layers(false).unwrap();
}

#[test]
fn cgb_repeated_frame_after_lcd_restart_does_not_drop_the_next_map() {
    let mut core = quick_cgb(LAYERS_CGB);
    run(&mut core, Budget::frames(SETTLE_FRAMES));
    let before = core.last_complete_frame().unwrap().tag.number;
    // Off and straight back on at vblank: the CGB repeats the previous frame
    // at the next vblank instead of showing the one it renders.
    core.bus_write(0xFF40, 0x13).unwrap();
    core.bus_write(0xFF40, 0x93).unwrap();
    // Turn capture on partway through that repeated frame.
    run(&mut core, Budget::instructions(2_000));
    core.set_pixel_layers(true).unwrap();
    run(&mut core, Budget::frames(1));
    assert_eq!(
        core.last_complete_frame().unwrap().tag.number,
        before,
        "the vblank after the restart repeats the previous frame"
    );
    // The next frame was rendered whole, with capture on throughout.
    run(&mut core, Budget::frames(1));
    let map = core
        .pixel_layers()
        .expect("a map for the first whole frame");
    assert_eq!(tag(&map, 32, 0), Layer::Object { index: 1 });
}

#[test]
fn capture_changes_no_state_or_frame_on_the_busy_rom() {
    assert_capture_is_observation_only(BUSY, 120);
}

#[test]
fn capture_changes_no_state_or_frame_on_cpu_instrs() {
    assert_capture_is_observation_only(&cpu_instrs(), 60);
}

fn assert_capture_is_observation_only(rom: &[u8], frames: u64) {
    let mut off = quick_dmg(rom);
    let mut on = quick_dmg(rom);
    on.set_pixel_layers(true).unwrap();
    for i in 0..frames {
        let a = run(&mut off, Budget::frames(1));
        let b = run(&mut on, Budget::frames(1));
        assert_eq!(a, b, "stop differs at frame {i}");
        assert!(off.save() == on.save(), "state differs at frame {i}");
        match (off.last_complete_frame(), on.last_complete_frame()) {
            (Some(a), Some(b)) => {
                assert_eq!(a.tag, b.tag, "frame tag differs at frame {i}");
                assert_eq!((a.width, a.height), (b.width, b.height));
                match (a.pixels, b.pixels) {
                    (Pixels::Shades(a), Pixels::Shades(b)) => assert_eq!(a, b),
                    (Pixels::Rgb(a), Pixels::Rgb(b)) => assert_eq!(a, b),
                    _ => panic!("pixel kinds differ at frame {i}"),
                }
            }
            (None, None) => {}
            _ => panic!("one core has a frame and the other doesn't at frame {i}"),
        }
    }
    // The capture really did produce a map by the end.
    assert!(on.pixel_layers().is_ok());
    assert_eq!(on.save(), off.save());
}

#[test]
fn the_blank_frame_after_the_lcd_turns_on_has_no_map() {
    let mut core = quick_dmg(BUSY);
    core.set_pixel_layers(true).unwrap();
    run(&mut core, Budget::frames(3));
    assert!(core.pixel_layers().is_ok(), "a rendered frame has a map");

    core.bus_write(0xFF40, 0x00).unwrap(); // LCD off
    run(&mut core, Budget::frames(2));
    assert!(
        matches!(core.pixel_layers(), Err(CoreError::InvalidState { .. })),
        "an LCD-off frame has no layer map"
    );

    core.bus_write(0xFF40, 0x91).unwrap(); // LCD on
    run(&mut core, Budget::frames(1));
    assert!(
        matches!(core.pixel_layers(), Err(CoreError::InvalidState { .. })),
        "SameBoy substitutes a blank frame for the first one after LCD turn-on"
    );
    run(&mut core, Budget::frames(1));
    assert!(
        core.pixel_layers().is_ok(),
        "the frame after the blank one has a map again"
    );
}

#[test]
fn reading_the_map_changes_no_state() {
    let mut core = quick_dmg(BUSY);
    core.set_pixel_layers(true).unwrap();
    run(&mut core, Budget::frames(2));
    let before = core.save();
    let map = core.pixel_layers().unwrap();
    assert_eq!(map.layers.len(), SCREEN_WIDTH * SCREEN_HEIGHT);
    assert!(core.save() == before);
}

#[test]
fn sgb_refuses_capture_rather_than_guessing_a_layer() {
    let mut core = SameBoy::new(CoreConfig::new(Model::Sgb, BUSY.to_vec())).unwrap();
    assert!(matches!(
        core.supports(Feature::PixelLayers),
        Support::No { reason } if !reason.is_empty()
    ));
    assert!(matches!(
        core.set_pixel_layers(true),
        Err(CoreError::Unsupported {
            feature: Feature::PixelLayers,
            ..
        })
    ));
}

#[test]
fn dmg_acid2_layer_tags_agree_with_its_oam() {
    let mut core = dmg(&corpus_rom("dmg-acid2/dmg-acid2.gb"), 0);
    core.set_pixel_layers(true).unwrap();
    // Past the boot ROM and dmg-acid2's own settling.
    run(&mut core, Budget::frames(200));
    let map = core.pixel_layers().unwrap();
    assert_eq!((map.width, map.height), (160, 144));

    let oam = core.region(Region::Oam).unwrap().bytes.to_vec();
    let vram = core.region(Region::Vram).unwrap().bytes.to_vec();
    let lcdc = core.io_register(IoReg::LCDC).unwrap();
    let ly = core.ppu().ly.min(144);
    assert!(lcdc & 0x80 != 0, "LCD is on");
    // dmg-acid2 rewrites LCDC from LY=LYC interrupts, so its value after the
    // frame is the footer's: OBJ enable and OBJ size there say nothing about
    // the rows above. An object tag must instead be consistent with *some* of
    // the two sizes a line could have been drawn at.
    let sizes = [8usize, 16];

    let mut counts = [0usize; 3]; // background, window, object
    for y in 0..usize::from(ly) {
        for x in 0..SCREEN_WIDTH {
            match tag(&map, x, y) {
                Layer::Background => counts[0] += 1,
                Layer::Window => counts[1] += 1,
                Layer::Object { index } => {
                    counts[2] += 1;
                    let index = usize::from(index);
                    assert!(index < 40, "OAM index {index} out of range");
                    let entry = &oam[index * 4..index * 4 + 4];
                    let (sprite_y, sprite_x, tile, flags) =
                        (entry[0], entry[1], entry[2], entry[3]);
                    let consistent = sizes.iter().any(|&height| {
                        let row = y as i32 - (sprite_y as i32 - 16);
                        let column = x as i32 - (sprite_x as i32 - 8);
                        if row < 0 || row >= height as i32 || !(0..8).contains(&column) {
                            return false;
                        }
                        let row = if flags & 0x40 != 0 {
                            height - 1 - row as usize
                        } else {
                            row as usize
                        };
                        let column = if flags & 0x20 != 0 {
                            7 - column as usize
                        } else {
                            column as usize
                        };
                        let tile = usize::from(tile) & if height == 16 { 0xFE } else { 0xFF };
                        let address = tile * 16 + row * 2;
                        let bit = 7 - column;
                        let pixel =
                            ((vram[address] >> bit) & 1) | (((vram[address + 1] >> bit) & 1) << 1);
                        // The tag must lie on an opaque pixel of that object's tile.
                        pixel != 0
                    });
                    assert!(
                        consistent,
                        "object {index} does not cover an opaque pixel at ({x},{y})"
                    );
                }
            }
        }
    }
    assert!(counts.iter().all(|&n| n > 0), "missing a layer: {counts:?}");
}

#[test]
fn dmg_acid2_window_is_a_suffix_of_each_line() {
    let mut core = dmg(&corpus_rom("dmg-acid2/dmg-acid2.gb"), 0);
    core.set_pixel_layers(true).unwrap();
    run(&mut core, Budget::frames(200));
    let map = core.pixel_layers().unwrap();
    let ly = core.ppu().ly.min(144);

    // The window starts at WX-7 and stays for the rest of the line, so once a
    // row has a window pixel no background pixel can follow it (objects may
    // overlay the window). This catches the window flag failing to carry over
    // from the tile where it was activated to the tiles after it.
    let mut window_pixels = 0;
    for y in 0..usize::from(ly) {
        let mut in_window = false;
        for x in 0..SCREEN_WIDTH {
            match tag(&map, x, y) {
                Layer::Window => {
                    in_window = true;
                    window_pixels += 1;
                }
                Layer::Background => {
                    assert!(!in_window, "background after the window began at ({x},{y})")
                }
                Layer::Object { .. } => {}
            }
        }
    }
    assert!(
        window_pixels > 0,
        "dmg-acid2 draws a window and none was tagged"
    );
}
