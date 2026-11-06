//! Per-pixel layer identity: capture is observation-only, and the tags name the
//! layer SameBoy actually mixed: background, window, or an OAM object.
//!
//! `layers.gb` (crates/sameboy/tests/roms/layers.asm) pins the interesting
//! cases; dmg-acid2 checks that the tags stay consistent with OAM and the
//! window over a real PPU conformance ROM.

mod common;

use common::*;
use gbcli_core::{
    Budget, Core, CoreConfig, CoreError, Feature, Layer, LayerMap, Model, Pixels, Region, Support,
};
use gbcli_sameboy::SameBoy;

const LAYERS: &[u8] = include_bytes!("roms/layers.gb");

// dmg-acid2's screen coordinates.
const SCREEN_WIDTH: usize = 160;

fn tag(map: &LayerMap<'_>, x: usize, y: usize) -> Layer {
    map.layers[x + y * SCREEN_WIDTH]
}

fn shades(core: &SameBoy) -> Vec<u8> {
    match core.last_complete_frame().expect("a frame").pixels {
        Pixels::Shades(shades) => shades.to_vec(),
        Pixels::Rgb(_) => panic!("DMG frame came out as RGB"),
    }
}

#[test]
fn the_mixing_point_tags_sprite_transparency_priority_and_window() {
    for busy in [false, true] {
        let mut core = quick_dmg(LAYERS);
        if busy {
            // The ROM reads VRAM in its loop, which syncs the PPU and forces
            // the slow, per-pixel path; otherwise it spins and mode 3 batches.
            core.poke(Region::Wram, 0, &[1]).unwrap();
        }
        core.set_pixel_layers(true).unwrap();
        let map = core.pixel_layers();
        assert!(
            matches!(map, Err(CoreError::InvalidState { .. })),
            "a map before the first captured frame: {map:?}"
        );
        // The ROM's setup (two 1 KiB map fills and OAM) takes a few frames,
        // and the first frame after it turns the LCD on is the blank one
        // SameBoy substitutes, which has no layer map.
        run(&mut core, Budget::frames(10));
        let map = core.pixel_layers().unwrap();
        assert_eq!((map.width, map.height), (160, 144));
        assert_eq!(map.layers.len(), SCREEN_WIDTH * 144);

        // Object 0 is opaque over the background, for all eight rows.
        for y in 0..8 {
            assert_eq!(tag(&map, 8, y), Layer::Object { index: 0 }, "busy={busy}");
        }
        // Object 1's leftmost pixel is transparent: the background shows.
        assert_eq!(tag(&map, 32, 0), Layer::Background, "busy={busy}");
        assert_eq!(tag(&map, 33, 0), Layer::Object { index: 1 }, "busy={busy}");
        // Object 2 has the BG-priority bit and the BG pixel is color 1, so the
        // background wins the whole sprite.
        for x in 56..64 {
            assert_eq!(tag(&map, x, 0), Layer::Background, "busy={busy}");
        }
        // WX=96 starts the window at x=89; x=88 is still background.
        assert_eq!(tag(&map, 88, 0), Layer::Background, "busy={busy}");
        assert_eq!(tag(&map, 89, 0), Layer::Window, "busy={busy}");
        assert_eq!(tag(&map, 159, 0), Layer::Window, "busy={busy}");
        // Every tag is one of the three kinds, and the map matches the pixels.
        assert_eq!(map.layers.len(), SCREEN_WIDTH * 144);
        assert_eq!(shades(&core).len(), SCREEN_WIDTH * 144);
    }
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
    assert_eq!(map.layers.len(), SCREEN_WIDTH * 144);
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
    let lcdc = core.io_register(gbcli_core::IoReg::LCDC).unwrap();
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
