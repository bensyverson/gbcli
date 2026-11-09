//! Sheets of 8x8 objects (the examples' DX review, item 11; leaf GQFHVi):
//! a frame size whose height is a multiple of 8 but not 16
//! (`shot.8x8.png`, `ship.16x8.png`) makes 8x8 objects, one tile each,
//! deduplicated with flips, with the OBP and OBJ palette data 8x16 sheets
//! have.

use crate::common;
use crate::pngs;

use std::fs;

use gbcli_build::{BuildOptions, build};
use gbcli_protocol::{BuildTarget, ImageLimit, ImagesMade, ObjectSize};

use common::{Temp, rgbds, starter};
use pngs::*;

const RED: Rgba = [0xFF, 0x00, 0x00, 0xFF];

/// A 16x8 frame: a black stroke down column 1 of the left object and a
/// dark block in the right one's top-left, on transparent.
fn arrow(x: u32, y: u32) -> Rgba {
    match (x, y) {
        (1, _) => BLACK,
        (8..=10, 0..=2) => DARK,
        _ => CLEAR,
    }
}

fn text(temp: &Temp, file: &str) -> String {
    fs::read_to_string(temp.path(&format!("demo/build/images/{file}"))).unwrap()
}

fn built(options: &BuildOptions) -> ImagesMade {
    let made = build(&rgbds(), options).unwrap().images.unwrap();
    assert!(made.problems.is_empty(), "{:#?}", made.problems);
    made
}

#[test]
fn a_frame_eight_high_makes_8x8_objects_deduplicated_with_flips() {
    let (temp, options) = starter("sprites-8x8");
    // Two 16x8 frames: the arrow, then the arrow mirrored, which reuses
    // both objects flipped.
    write_png(
        &temp.path("demo/assets/sprites/ship.16x8.png"),
        32,
        8,
        |x, y| match x / 16 {
            0 => arrow(x, y),
            _ => arrow(15 - (x - 16), y),
        },
    );
    let made = built(&options);
    let [image] = &made.images[..] else {
        panic!("one image: {made:#?}")
    };
    assert_eq!(image.object_size, Some(ObjectSize::Square));
    assert_eq!((image.width, image.height), (4, 1));
    assert_eq!(image.tiles, 2, "two 8x8 objects, a tile each");
    assert_eq!((image.frames, image.objects), (Some(2), Some(2)));
    // One entry an 8x8 object, its index its tile.
    let indices: Vec<u32> = image.tile_map.iter().flatten().map(|e| e.index).collect();
    assert_eq!(indices, [0, 1, 1, 0]);
    let tiles = fs::read(temp.path("demo/build/images/sprites/ship.2bpp")).unwrap();
    assert_eq!(tiles.len(), 2 * 16);

    let asm = text(&temp, "images.asm");
    for line in [
        "unique 8x8 objects, 16 bytes each",
        // y, x, tile, attributes: object 0 at (0, 0), object 1 at (0, 8).
        "ShipFrame0::\n\tdb 2\n\tdb 0, 0, 0, 0\n\tdb 0, 8, 1, 0\n",
        // Mirrored: object 1 flipped at the left, object 0 at the right.
        "ShipFrame1::\n\tdb 2\n\tdb 0, 0, 1, $20\n\tdb 0, 8, 0, $20\n",
    ] {
        assert!(asm.contains(line), "{line:?} in\n{asm}");
    }
    let inc = text(&temp, "images.inc");
    for line in [
        "8x8 objects (clear LCDC_OBJ_16)",
        "DEF SHIP_TILES EQU 2 ; tiles, one per object",
        "DEF SHIP_FRAME_HEIGHT EQU 8",
        "DEF SHIP_OBP EQU %00_11_10_00",
    ] {
        assert!(inc.contains(line), "{line:?} in\n{inc}");
    }
    // The protocol reads an 8x8 sheet's odd tile indices back.
    let json = serde_json::to_string(&made).unwrap();
    let back: ImagesMade = serde_json::from_str(&json).unwrap();
    assert_eq!(back, made);
}

/// The convention's own example: `shot.8x8.png`, a frame an object.
#[test]
fn an_8x8_frame_is_one_8x8_object() {
    let (temp, options) = starter("sprites-8x8-shot");
    // Two frames: a dot, and a bar.
    write_png(
        &temp.path("demo/assets/sprites/shot.8x8.png"),
        16,
        8,
        |x, y| match (x, y) {
            (3..=4, 3..=4) => BLACK,
            (8..=15, 4) => DARK,
            _ => CLEAR,
        },
    );
    let made = built(&options);
    let image = &made.images[0];
    assert_eq!(image.object_size, Some(ObjectSize::Square));
    assert_eq!(
        (image.tiles, image.frames, image.objects),
        (2, Some(2), Some(1))
    );
    let asm = text(&temp, "images.asm");
    assert!(
        asm.contains("ShotFrame1::\n\tdb 1\n\tdb 0, 0, 1, 0\n"),
        "{asm}"
    );
}

/// 8x16 sheets say so, and a sheet with no frame size stays 8x16.
#[test]
fn a_sheet_without_a_frame_size_and_a_16_high_frame_stay_8x16() {
    let (temp, options) = starter("sprites-8x16-kept");
    write_png(&temp.path("demo/assets/sprites/a.png"), 8, 16, |x, _| {
        if x == 0 { BLACK } else { CLEAR }
    });
    write_png(
        &temp.path("demo/assets/sprites/b.16x16.png"),
        16,
        16,
        |x, _| {
            if x == 0 { BLACK } else { CLEAR }
        },
    );
    let made = built(&options);
    for image in &made.images {
        assert_eq!(
            image.object_size,
            Some(ObjectSize::Tall),
            "{}",
            image.source
        );
    }
    let inc = text(&temp, "images.inc");
    assert!(inc.contains("8x16 objects (set LCDC_OBJ_16)"), "{inc}");
}

/// A sheet eight high with no frame size is still `obj_pair_8x16`, and its
/// remedy offers the 8x8 frame size.
#[test]
fn a_sheet_eight_high_without_a_frame_size_offers_8x8_frames() {
    let (temp, options) = starter("sprites-8-high");
    write_png(&temp.path("demo/assets/sprites/dot.png"), 16, 8, |x, _| {
        if x == 0 { BLACK } else { CLEAR }
    });
    let made = refused(&options);
    let p = problem(&made, ImageLimit::ObjPair8x16);
    assert!(p.remedy.contains("dot.8x8.png"), "{}", p.remedy);
}

/// Under --gbc an 8x8 color sheet gets its GBC tiles, one per object, and
/// the OBJ palettes; an indexed one its palette's reordered copy.
#[test]
fn an_8x8_sheet_has_its_gbc_tiles_and_obj_palettes() {
    let (temp, mut options) = starter("sprites-8x8-gbc");
    options.target = BuildTarget::Gbc;
    // Color art: red on white, and black.
    write_png(
        &temp.path("demo/assets/sprites/shot.8x8.png"),
        16,
        8,
        |x, y| match (x, y) {
            (1..=2, _) => RED,
            (3, _) => WHITE,
            (9..=10, _) => BLACK,
            _ => CLEAR,
        },
    );
    // Indexed art, assigned `ice`.
    write_png(
        &temp.path("demo/assets/sprites/gem.8x8.png"),
        8,
        8,
        |x, _| {
            if x < 4 { DARK } else { CLEAR }
        },
    );
    fs::write(
        temp.path("demo/assets/palettes.txt"),
        "dmg: #ffffff #aaaaaa #555555 #000000\nice: #fff #8cf #46a #013\nsprites/gem.8x8.png = ice\n",
    )
    .unwrap();
    let made = built(&options);
    let shot = made.images.iter().find(|i| i.label == "Shot").unwrap();
    assert_eq!(shot.tiles, 2);
    assert!(shot.gbc.is_some());
    assert!(shot.tile_map.iter().flatten().all(|e| e.palette.is_some()));
    let gbc_tiles = fs::read(temp.path("demo/build/images/sprites/shot.gbc.2bpp")).unwrap();
    assert_eq!(gbc_tiles.len(), 2 * 16);
    let asm = text(&temp, "images.asm");
    for label in ["ShotTilesGbc::", "ObjPalettesGbc::", "GemPaletteIce::"] {
        assert!(asm.contains(label), "{label} in\n{asm}");
    }
    let palettes = made.gbc.as_ref().unwrap().obj_palettes.len();
    assert_eq!(palettes, 2, "ice for gem, one packed for shot");
}
