//! `build` over sprite sheets in `assets/sprites/`: 8x16 units deduplicated
//! with flips, one object list per frame, and a structured problem at each
//! object limit; and the VRAM backgrounds and sprites share.

use crate::common;
use crate::pngs;

use std::fs;

use gbcli_build::build;
use gbcli_protocol::{ImageKind, ImageLimit, Severity, TileAt};

use common::{rgbds, starter};
use pngs::*;

/// A 16x16 frame: a black stroke down column 1 of the left unit and a dark
/// block in the right unit's top-left, on transparent.
fn arrow(x: u32, y: u32) -> Rgba {
    match (x, y) {
        (1, _) => BLACK,
        (8..=10, 0..=2) => DARK,
        _ => CLEAR,
    }
}

#[test]
fn a_sheet_dedups_whole_8x16_units_with_flips_and_lists_objects_per_frame() {
    let (temp, options) = starter("sprites-dedup");
    // Two 16x16 frames: the arrow, then the arrow mirrored left to right,
    // which reuses both units flipped. A third, empty frame has no objects.
    write_png(
        &temp.path("demo/assets/sprites/hero.16x16.png"),
        48,
        16,
        |x, y| match x / 16 {
            0 => arrow(x, y),
            1 => arrow(15 - (x - 16), y),
            _ => CLEAR,
        },
    );
    let made = build(&rgbds(), &options).unwrap();
    let images = made.images.unwrap();
    assert!(images.problems.is_empty(), "{:?}", images.problems);
    let [image] = &images.images[..] else {
        panic!("one image: {images:?}")
    };
    assert_eq!(image.kind, ImageKind::Sprites);
    assert_eq!(image.label, "Hero");
    assert_eq!((image.width, image.height), (6, 2));
    assert_eq!(image.tiles, 4, "two 8x16 units");
    assert_eq!((image.frames, image.objects), (Some(3), Some(2)));
    let tiles = fs::read(temp.path("demo/build/images/sprites/hero.2bpp")).unwrap();
    assert_eq!(tiles.len(), 4 * 16);

    let asm = fs::read_to_string(temp.path("demo/build/images/images.asm")).unwrap();
    for line in [
        "HeroTiles::",
        "HeroFrames::",
        "\tdw HeroFrame0, HeroFrame1, HeroFrame2",
        // y, x, tile, attributes: unit 0 at (0, 0), unit 1 at (0, 8).
        "HeroFrame0::\n\tdb 2\n\tdb 0, 0, 0, 0\n\tdb 0, 8, 2, 0\n",
        // Mirrored: unit 1 flipped at the left, unit 0 flipped at the right.
        "HeroFrame1::\n\tdb 2\n\tdb 0, 0, 2, $20\n\tdb 0, 8, 0, $20\n",
        "HeroFrame2::\n\tdb 0\n",
    ] {
        assert!(asm.contains(line), "{line:?} in\n{asm}");
    }
    let include = fs::read_to_string(temp.path("demo/build/images/images.inc")).unwrap();
    for line in [
        "DEF HERO_TILES EQU 4",
        "DEF HERO_FRAMES EQU 3",
        "DEF HERO_FRAME_WIDTH EQU 16",
        "DEF HERO_FRAME_HEIGHT EQU 16",
        // Dark is index 1, black index 2: OBP maps 1 to shade 2, 2 to 3.
        "DEF HERO_OBP EQU %00_11_10_00",
    ] {
        assert!(include.contains(line), "{line} in\n{include}");
    }
    assert!(temp.path(&image.preview).is_file());
}

#[test]
fn a_frame_wider_than_ten_objects_is_obj_per_line() {
    let (temp, options) = starter("sprites-line");
    // One 96x16 frame (the whole sheet): 12 objects side by side.
    write_png(
        &temp.path("demo/assets/sprites/train.png"),
        96,
        16,
        |x, y| {
            if y == 0 && x % 8 == 0 { BLACK } else { CLEAR }
        },
    );
    let made = refused(&options);
    assert_eq!(errors(&made), [ImageLimit::ObjPerLine]);
    let p = problem(&made, ImageLimit::ObjPerLine);
    assert_eq!(p.image.as_deref(), Some("demo/assets/sprites/train.png"));
    assert_eq!((p.count, p.max), (12, 10));
    assert_eq!(p.tiles.len(), 12);
    assert_eq!(p.tiles[11], TileAt { x: 11, y: 0 });
    assert!(p.remedy.contains("train.WxH.png"), "{}", p.remedy);
}

#[test]
fn a_frame_of_more_than_forty_objects_is_obj_total() {
    let (temp, options) = starter("sprites-total");
    // 64x96: 8 objects a line (fine), 48 in all.
    write_png(
        &temp.path("demo/assets/sprites/boss.png"),
        64,
        96,
        |x, y| {
            if x % 8 == 0 && y % 16 == 0 {
                BLACK
            } else {
                CLEAR
            }
        },
    );
    let made = refused(&options);
    assert_eq!(errors(&made), [ImageLimit::ObjTotal]);
    let p = problem(&made, ImageLimit::ObjTotal);
    assert_eq!((p.count, p.max), (48, 40));
}

#[test]
fn an_object_palette_holds_three_shades_and_a_sheet_rows_sixteen_high() {
    let (temp, options) = starter("sprites-shades");
    write_png(&temp.path("demo/assets/sprites/gem.png"), 8, 16, |x, _| {
        [WHITE, LIGHT, DARK, BLACK][(x % 4) as usize]
    });
    write_png(
        &temp.path("demo/assets/sprites/short.png"),
        8,
        24,
        |_, _| BLACK,
    );
    let made = refused(&options);
    let p = problem(&made, ImageLimit::DmgPalette);
    assert_eq!(p.image.as_deref(), Some("demo/assets/sprites/gem.png"));
    assert_eq!((p.count, p.max), (4, 3));
    // Its DMG version is derived, in a plain build too (leaf GQFHVi): the
    // four shades used alike, so the lightest, and its tiles.
    assert!(p.message.contains("shade 0 (white"), "{}", p.message);
    assert_eq!(p.tiles, [TileAt { x: 0, y: 0 }, TileAt { x: 0, y: 1 }]);
    let p = problem(&made, ImageLimit::ObjPair8x16);
    assert_eq!(p.image.as_deref(), Some("demo/assets/sprites/short.png"));
    assert!(p.message.contains("24"), "{}", p.message);
}

#[test]
fn a_sheet_with_no_transparent_pixel_builds_with_a_warning() {
    let (temp, options) = starter("sprites-opaque");
    write_png(&temp.path("demo/assets/sprites/box.png"), 8, 16, |_, _| {
        BLACK
    });
    let made = build(&rgbds(), &options).unwrap();
    let p = problem(&made, ImageLimit::ObjNoTransparency);
    assert_eq!(p.severity, Severity::Warning);
}

#[test]
fn backgrounds_and_sprites_share_384_tiles_of_vram() {
    let (temp, options) = starter("sprites-vram");
    // A background of 250 unique tiles and sheets of 70 unique units (140
    // tiles): 390 of 384.
    write_png(&temp.path("demo/assets/bg/map.png"), 200, 80, |x, y| {
        numbered_tile(1 + (y / 8) * 25 + x / 8, x % 8, y % 8)
    });
    // Each unit numbered, with a black bottom-left corner so that no flip
    // of one unit is another.
    write_png(
        &temp.path("demo/assets/sprites/crowd.8x16.png"),
        80,
        112,
        |x, y| {
            let n = 1 + (y / 16) * 10 + x / 8;
            let corner = x % 8 == 0 && y % 16 == 15;
            if corner || numbered_tile(n, x % 8, y % 16) == BLACK {
                BLACK
            } else {
                CLEAR
            }
        },
    );
    let made = refused(&options);
    let p = problem(&made, ImageLimit::VramTiles);
    assert_eq!((p.count, p.max), (390, 384));
    assert_eq!(p.image, None);
    assert!(
        p.message.contains("demo/assets/bg/map.png"),
        "{}",
        p.message
    );
    assert!(
        p.message.contains("demo/assets/sprites/crowd.8x16.png"),
        "{}",
        p.message
    );
}
