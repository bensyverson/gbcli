//! Color art's DMG version under `build --gbc` (the GBC images plan,
//! questions 2, 3 and 9; leaf tzEhz3): derived from the color image, each
//! color taking the shade the project's rule gives it (`shades::choose`),
//! unless a `name.dmg.png` beside it overrides it. Two colors in one tile
//! or object that fall to one shade warn, naming it; across tiles they
//! don't.

use crate::common;
use crate::pngs;

use std::fs;

use gbcli_build::{BuildError, BuildOptions, build};
use gbcli_protocol::{BuildTarget, ImageLimit, Severity, TileAt};

use common::{Temp, rgbds, starter};
use pngs::*;

// Nearest shades by lightness (2126 R + 7152 G + 722 B), with white and
// black in the project so the stretch is the plain nearest shade.
const YELLOW: Rgba = [0xFF, 0xFF, 0x00, 0xFF]; // shade 0
const GREEN: Rgba = [0x00, 0xFF, 0x00, 0xFF]; // shade 1
const RED: Rgba = [0xFF, 0x00, 0x00, 0xFF]; // shade 2
const BLUE: Rgba = [0x00, 0x00, 0xFF, 0xFF]; // shade 3

fn gbc(name: &str) -> (Temp, BuildOptions) {
    let (temp, mut options) = starter(name);
    options.target = BuildTarget::Gbc;
    (temp, options)
}

fn read(temp: &Temp, file: &str) -> Vec<u8> {
    fs::read(temp.path(&format!("demo/build/images/{file}"))).unwrap()
}

/// The solid shade of each map cell of `bg/<stem>`, or none where a cell's
/// tile isn't solid.
fn cell_shades(temp: &Temp, stem: &str) -> Vec<Option<u8>> {
    let tiles = read(temp, &format!("bg/{stem}.2bpp"));
    let map = read(temp, &format!("bg/{stem}.tilemap"));
    map.iter()
        .map(|&t| {
            let tile = &tiles[usize::from(t) * 16..][..16];
            let first = index_at(tile, 0, 0);
            let solid = (0..8).all(|row| (0..8).all(|x| index_at(tile, row, x) == first));
            solid.then_some(first)
        })
        .collect()
}

fn layout_error(result: Result<gbcli_protocol::BuildMade, BuildError>) -> (String, String) {
    match result {
        Err(BuildError::Layout { message, remedy }) => (message, remedy),
        other => panic!("expected a layout error, got {other:?}"),
    }
}

#[test]
fn a_color_background_derives_each_colors_nearest_shade() {
    let (temp, options) = gbc("dmg-derived-bg");
    let tiles = [WHITE, YELLOW, GREEN, RED, BLUE, BLACK];
    write_png(&temp.path("demo/assets/bg/town.png"), 48, 8, |x, _| {
        tiles[(x / 8) as usize]
    });
    let made = build(&rgbds(), &options).unwrap();
    // BGP $E4: each index is its shade.
    let inc = String::from_utf8(read(&temp, "images.inc")).unwrap();
    assert_eq!(binary_constant(&inc, "TOWN_BGP"), 0b11_10_01_00);
    assert_eq!(
        cell_shades(&temp, "town"),
        [0, 0, 1, 2, 3, 3].map(Some),
        "{:?}",
        made.images
    );
    let images = made.images.unwrap();
    // Six tiles: white and yellow are one DMG tile but two GBC tiles, and
    // so are blue and black (the joint conversion keeps the pairs).
    assert_eq!(images.images[0].tiles, 6);
    // White and yellow, blue and black share shades, but in different
    // tiles: no detail is lost in any tile, so nothing to warn of.
    let shared: Vec<_> = images
        .problems
        .iter()
        .filter(|p| p.limit == ImageLimit::SharedShade)
        .collect();
    assert!(shared.is_empty(), "{shared:#?}");
}

#[test]
fn two_colors_in_one_background_tile_sharing_a_shade_warn_naming_the_tile() {
    let (temp, options) = gbc("dmg-shared-tile");
    write_png(
        &temp.path("demo/assets/bg/town.png"),
        24,
        8,
        |x, _| match x / 8 {
            0 => WHITE,
            1 => GREEN,
            _ if x % 8 < 4 => BLUE,
            _ => BLACK,
        },
    );
    let made = build(&rgbds(), &options).unwrap();
    let p = problem(&made, ImageLimit::SharedShade);
    assert_eq!(p.severity, Severity::Warning);
    assert_eq!(p.image.as_deref(), Some("demo/assets/bg/town.png"));
    assert_eq!(p.tiles, [TileAt { x: 2, y: 0 }]);
    for part in ["#0000ff", "#000000", "shade 3"] {
        assert!(p.message.contains(part), "{part:?} in {}", p.message);
    }
    assert!(
        p.remedy.contains("demo/assets/bg/town.dmg.png"),
        "{}",
        p.remedy
    );
    // Both draw black on the DMG.
    assert_eq!(cell_shades(&temp, "town"), [Some(0), Some(1), Some(3)]);
}

#[test]
fn two_colors_in_one_object_sharing_a_shade_warn_and_across_objects_dont() {
    let (temp, options) = gbc("dmg-shared-object");
    // White beside each, so the project's stretch is the plain nearest
    // shade. One object, blue over black: two tiles, one object.
    write_png(
        &temp.path("demo/assets/sprites/bat.png"),
        8,
        16,
        |x, y| match (x, y < 8) {
            (0..4, true) => BLUE,
            (0..4, false) => BLACK,
            (4, _) => WHITE,
            _ => CLEAR,
        },
    );
    // Two objects, one blue, one black.
    write_png(
        &temp.path("demo/assets/sprites/owl.png"),
        16,
        16,
        |x, _| match x {
            0..4 => BLUE,
            8..12 => BLACK,
            4 | 12 => WHITE,
            _ => CLEAR,
        },
    );
    let made = build(&rgbds(), &options).unwrap();
    let p = problem(&made, ImageLimit::SharedShade);
    assert_eq!(p.image.as_deref(), Some("demo/assets/sprites/bat.png"));
    assert_eq!(p.tiles, [TileAt { x: 0, y: 0 }]);
    assert!(
        p.remedy.contains("demo/assets/sprites/bat.dmg.png"),
        "{}",
        p.remedy
    );
}

#[test]
fn a_derived_sheet_of_four_opaque_shades_is_dmg_palette_naming_the_override() {
    let (temp, options) = gbc("dmg-derived-four");
    write_png(
        &temp.path("demo/assets/sprites/hero.16x16.png"),
        16,
        16,
        |x, _| [WHITE, GREEN, RED, BLACK, CLEAR][(x / 3).min(4) as usize],
    );
    let made = refused(&options);
    let p = problem(&made, ImageLimit::DmgPalette);
    assert_eq!(
        p.image.as_deref(),
        Some("demo/assets/sprites/hero.16x16.png")
    );
    assert_eq!((p.count, p.max), (4, 3));
    assert!(
        p.remedy.contains("demo/assets/sprites/hero.16x16.dmg.png"),
        "{}",
        p.remedy
    );
}

#[test]
fn an_override_wins_over_the_derived_version() {
    let (temp, options) = gbc("dmg-override");
    // Derived, these would be shades 2 and 1, and the blue and black tile
    // would warn.
    write_png(
        &temp.path("demo/assets/bg/town.png"),
        24,
        8,
        |x, _| match x / 8 {
            0 => RED,
            1 => GREEN,
            _ if x % 8 < 4 => BLUE,
            _ => BLACK,
        },
    );
    write_png(&temp.path("demo/assets/bg/town.dmg.png"), 24, 8, |x, _| {
        [WHITE, BLACK, DARK][(x / 8) as usize]
    });
    let made = build(&rgbds(), &options).unwrap();
    assert_eq!(cell_shades(&temp, "town"), [Some(0), Some(3), Some(2)]);
    let images = made.images.unwrap();
    assert!(images.problems.is_empty(), "{:#?}", images.problems);
    // Only the color image is an image; the override is its DMG version.
    assert_eq!(images.images.len(), 1);
    assert_eq!(images.images[0].source, "demo/assets/bg/town.png");
}

#[test]
fn an_override_of_another_size_is_dmg_override_size() {
    let (temp, options) = gbc("dmg-override-size");
    write_png(&temp.path("demo/assets/bg/town.png"), 16, 8, |x, _| {
        [RED, GREEN][(x / 8) as usize]
    });
    write_png(&temp.path("demo/assets/bg/town.dmg.png"), 8, 8, |_, _| {
        WHITE
    });
    let made = refused(&options);
    let p = problem(&made, ImageLimit::DmgOverrideSize);
    assert_eq!(p.severity, Severity::Error);
    assert_eq!(p.image.as_deref(), Some("demo/assets/bg/town.dmg.png"));
    assert_eq!((p.count, p.max), (8, 16));
    for part in ["8x8", "16x8", "demo/assets/bg/town.png"] {
        assert!(p.message.contains(part), "{part:?} in {}", p.message);
    }
}

#[test]
fn an_override_without_gbc_is_an_error_naming_gbc() {
    let (temp, options) = starter("dmg-override-no-gbc");
    write_png(&temp.path("demo/assets/bg/town.png"), 8, 8, |_, _| WHITE);
    write_png(&temp.path("demo/assets/bg/town.dmg.png"), 8, 8, |_, _| {
        WHITE
    });
    let (message, remedy) = layout_error(build(&rgbds(), &options));
    assert!(message.contains("demo/assets/bg/town.dmg.png"), "{message}");
    assert!(remedy.contains("--gbc"), "{remedy}");
}

#[test]
fn an_override_beside_indexed_art_is_an_error() {
    let (temp, options) = gbc("dmg-override-indexed");
    write_png(&temp.path("demo/assets/bg/town.png"), 8, 8, |x, _| {
        [WHITE, BLACK][(x / 4) as usize]
    });
    write_png(&temp.path("demo/assets/bg/town.dmg.png"), 8, 8, |_, _| {
        WHITE
    });
    fs::write(
        temp.path("demo/assets/palettes.txt"),
        "forest: #fff #8c4 #363 #021\nbg/town.png = forest\n",
    )
    .unwrap();
    let (message, remedy) = layout_error(build(&rgbds(), &options));
    for part in ["demo/assets/bg/town.dmg.png", "forest"] {
        assert!(message.contains(part), "{part:?} in {message}");
    }
    assert!(remedy.contains("remove"), "{remedy}");
}
