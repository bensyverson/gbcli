//! Colors that share a DMG shade (Ben's rulings, 2026-11-07): each color
//! takes the shade the project's rule gives it even when another already
//! has it, both draw in that shade, and the build warns, naming the colors,
//! the shade, the rule and the tiles, so an artist who meant the difference
//! can redraw or declare the `dmg` line of `assets/palettes.txt`. The art
//! here is indexed (`pngs::indexed`), its own DMG version, whose sharing
//! warns for the whole image; color art's warns within a tile or object
//! (`images_dmg_version.rs`).

use crate::common;
use crate::pngs;

use std::fs;

use gbcli_build::build;
use gbcli_protocol::{ImageLimit, Severity, TileAt};

use common::{rgbds, starter};
use pngs::*;

/// Black's neighbor: a distinct color in RGB555 (2 of 31 a channel), whose
/// nearest shade is black too.
const NEAR_BLACK: Rgba = [0x10, 0x10, 0x10, 0xFF];

#[test]
fn black_and_near_black_build_both_black_with_a_warning_naming_both() {
    let (temp, options) = starter("shades-shared-bg");
    // Tiles: white, black, near-black, and black with two near-black
    // columns. Black is the more used (112 px to 80).
    write_png(
        &temp.path("demo/assets/bg/cave.png"),
        32,
        8,
        |x, _| match (x / 8, x % 8) {
            (0, _) => WHITE,
            (1, _) | (3, 0..6) => BLACK,
            _ => NEAR_BLACK,
        },
    );
    indexed(&temp.path("demo"), &["bg/cave.png"]);
    let made = build(&rgbds(), &options).unwrap();
    let images = made.images.as_ref().unwrap();
    assert_eq!(images.images[0].tiles, 2, "white and black: {images:?}");

    let tiles = fs::read(temp.path("demo/build/images/bg/cave.2bpp")).unwrap();
    let map = fs::read(temp.path("demo/build/images/bg/cave.tilemap")).unwrap();
    assert_eq!(
        (map[1], map[2], map[3]),
        (map[1], map[1], map[1]),
        "{map:?}"
    );
    let black = &tiles[usize::from(map[2]) * 16..][..16];
    assert!(black.iter().all(|&b| b == 0xFF), "index 3: {black:02x?}");
    let inc = fs::read_to_string(temp.path("demo/build/images/images.inc")).unwrap();
    assert_eq!(binary_constant(&inc, "CAVE_BGP"), 0b11_10_01_00);

    let p = problem(&made, ImageLimit::SharedShade);
    assert_eq!(p.image.as_deref(), Some("demo/assets/bg/cave.png"));
    assert_eq!(p.severity, Severity::Warning);
    assert_eq!((p.count, p.max), (2, 1));
    // The less used color's tiles: where to look if the difference matters.
    assert_eq!(p.tiles, [TileAt { x: 2, y: 0 }, TileAt { x: 3, y: 0 }]);
    for part in ["#000000", "#101010", "shade 3", "black"] {
        assert!(p.message.contains(part), "{part:?} in {}", p.message);
    }
    assert!(
        p.remedy.contains("distinct if the difference matters"),
        "{}",
        p.remedy
    );
    assert!(p.remedy.contains("assets/palettes.txt"), "{}", p.remedy);
    // Not one known palette (#101010), so the project's lightness is
    // stretched, white to black, and the warning says so.
    assert_eq!(
        images.shades.map(|s| s.rule),
        Some(gbcli_protocol::ShadeRule::Stretched)
    );
    assert!(p.message.contains("stretched"), "{}", p.message);

    // The preview draws the near-black tile black.
    let preview = temp.path(&images.images[0].preview);
    let (width, height, pixels) = read_png(&preview);
    let scale = height / 8;
    assert_eq!(pixels[(16 * scale) as usize], 0x000000, "{width}x{height}");
    // Nothing but the data and the preview is left beside them.
    let mut left: Vec<String> = fs::read_dir(temp.path("demo/build/images/bg"))
        .unwrap()
        .map(|e| e.unwrap().file_name().to_string_lossy().into_owned())
        .collect();
    left.sort();
    assert_eq!(left, ["cave.2bpp", "cave.preview.png", "cave.tilemap"]);
}

#[test]
fn a_sheet_draws_colors_sharing_a_shade_in_one_index_with_a_warning() {
    let (temp, options) = starter("shades-shared-sheet");
    // White elsewhere in the project, so the stretch runs white to black
    // and the sheet's dark gray stays shade 2.
    write_png(&temp.path("demo/assets/bg/sky.png"), 8, 8, |_, _| WHITE);
    write_png(
        &temp.path("demo/assets/sprites/bat.png"),
        8,
        16,
        |x, _| match x {
            0 | 1 => BLACK,
            2 => NEAR_BLACK,
            3 => DARK,
            _ => CLEAR,
        },
    );
    indexed(&temp.path("demo"), &["bg/sky.png", "sprites/bat.png"]);
    let made = build(&rgbds(), &options).unwrap();
    let data = fs::read(temp.path("demo/build/images/sprites/bat.2bpp")).unwrap();
    let (black, near, dark) = (
        index_at(&data, 0, 0),
        index_at(&data, 0, 2),
        index_at(&data, 0, 3),
    );
    assert_eq!(black, near, "{data:02x?}");
    assert_ne!(black, dark);
    let inc = fs::read_to_string(temp.path("demo/build/images/images.inc")).unwrap();
    let obp = binary_constant(&inc, "BAT_OBP");
    assert_eq!((obp >> (2 * black)) & 3, 3, "{obp:08b}");
    assert_eq!((obp >> (2 * dark)) & 3, 2, "{obp:08b}");

    let p = problem(&made, ImageLimit::SharedShade);
    assert_eq!(p.image.as_deref(), Some("demo/assets/sprites/bat.png"));
    assert_eq!(p.severity, Severity::Warning);
    assert_eq!(p.tiles, [TileAt { x: 0, y: 0 }, TileAt { x: 0, y: 1 }]);
    for part in ["#000000", "#101010", "shade 3"] {
        assert!(p.message.contains(part), "{part:?} in {}", p.message);
    }
}

/// Two pairs share two shades: one warning each.
#[test]
fn each_shared_shade_is_its_own_warning() {
    let (temp, options) = starter("shades-two-pairs");
    let near_white: Rgba = [0xF0, 0xF0, 0xF0, 0xFF];
    write_png(&temp.path("demo/assets/bg/fog.png"), 8, 8, |x, _| {
        [
            WHITE, WHITE, near_white, BLACK, BLACK, NEAR_BLACK, WHITE, BLACK,
        ][x as usize]
    });
    indexed(&temp.path("demo"), &["bg/fog.png"]);
    let made = build(&rgbds(), &options).unwrap();
    let found = problems(&made, ImageLimit::SharedShade);
    assert_eq!(found.len(), 2, "{found:#?}");
    assert!(found[0].message.contains("#f0f0f0"), "{}", found[0].message);
    assert!(found[1].message.contains("#101010"), "{}", found[1].message);
    let tiles = fs::read(temp.path("demo/build/images/bg/fog.2bpp")).unwrap();
    let row: Vec<u8> = (0..8).map(|x| index_at(&tiles, 0, x)).collect();
    assert_eq!(row, [0, 0, 0, 3, 3, 3, 0, 3]);
}

/// Distinct shades, as before: no warning.
#[test]
fn the_four_grays_build_without_a_shared_shade_warning() {
    let (temp, options) = starter("shades-four-grays");
    write_png(&temp.path("demo/assets/bg/grays.png"), 8, 8, |x, _| {
        [WHITE, LIGHT, DARK, BLACK][(x % 4) as usize]
    });
    let made = build(&rgbds(), &options).unwrap();
    assert!(
        problems(&made, ImageLimit::SharedShade).is_empty(),
        "{:?}",
        made.images
    );
}
