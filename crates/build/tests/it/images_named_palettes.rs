//! `assets/palettes.txt` in a DMG build (plan question 10, Ben accepted
//! the recommendation): the `dmg` line drives the shade cascade, the named
//! palettes and assignments are checked, and an assigned image builds as
//! DMG art (leaf WmNCxc).

use crate::common;
use crate::pngs;

use std::fs;

use gbcli_build::build;
use gbcli_protocol::{ImageLimit, ShadeRule};

use common::{Temp, rgbds, starter};
use pngs::*;

const PALETTES: &str = "dmg: #ffffff #aaaaaa #555555 #000000\n\
                        forest: #fff #8c4 #363 #021\n\
                        ember:  #ffe #f80 #a20 #200   # a swap, used by no image\n";

fn sky(temp: &Temp) {
    write_png(&temp.path("demo/assets/bg/sky.png"), 16, 8, |x, y| {
        [WHITE, LIGHT, DARK, BLACK][((x + y) % 4) as usize]
    });
}

fn data(temp: &Temp) -> (Vec<u8>, Vec<u8>) {
    let read = |f: &str| fs::read(temp.path(&format!("demo/build/images/bg/{f}"))).unwrap();
    (read("sky.2bpp"), read("sky.tilemap"))
}

/// The same tile data and map with or without an assignment: on the DMG,
/// indexed art is DMG art.
#[test]
fn an_assigned_image_builds_as_dmg_art() {
    let (temp, options) = starter("named-dmg-art");
    sky(&temp);
    build(&rgbds(), &options).unwrap();
    let plain = data(&temp);

    fs::write(
        temp.path("demo/assets/palettes.txt"),
        format!("{PALETTES}bg/sky.png = forest\n"),
    )
    .unwrap();
    let made = build(&rgbds(), &options).unwrap();
    assert_eq!(data(&temp), plain);
    let images = made.images.unwrap();
    assert_eq!(images.shades.unwrap().rule, ShadeRule::Declared);
    assert!(images.problems.is_empty(), "{:?}", images.problems);
}

/// A named palette is checked though a DMG build doesn't use it.
#[test]
fn a_bad_named_palette_stops_a_dmg_build_at_its_line() {
    let (temp, options) = starter("named-bad");
    sky(&temp);
    fs::write(
        temp.path("demo/assets/palettes.txt"),
        "dmg: #ffffff #aaaaaa #555555 #000000\n\nforest: #fff #8c4\n",
    )
    .unwrap();
    let made = refused(&options);
    let p = problem(&made, ImageLimit::PaletteFile);
    assert_eq!(p.image.as_deref(), Some("demo/assets/palettes.txt"));
    assert_eq!((p.line, p.count, p.max), (Some(3), 2, 4));
    assert!(
        p.message.contains("line 3: forest lists 2 colors"),
        "{}",
        p.message
    );
}

/// An assignment names one of the project's images, by its path under
/// assets/; with no images at all it is still refused.
#[test]
fn an_assignment_to_a_missing_image_is_refused_at_its_line() {
    let (temp, options) = starter("named-missing");
    sky(&temp);
    fs::write(
        temp.path("demo/assets/palettes.txt"),
        format!("{PALETTES}bg/sky.png = forest\nbg/town.png = forest\n"),
    )
    .unwrap();
    let made = refused(&options);
    let p = problem(&made, ImageLimit::PaletteFile);
    assert_eq!(p.line, Some(5));
    for part in ["bg/town.png is no image", "its images are bg/sky.png"] {
        assert!(p.message.contains(part), "{part:?} in {}", p.message);
    }

    let (temp, options) = starter("named-missing-none");
    fs::write(
        temp.path("demo/assets/palettes.txt"),
        format!("{PALETTES}bg/sky.png = forest\n"),
    )
    .unwrap();
    let made = refused(&options);
    let p = problem(&made, ImageLimit::PaletteFile);
    assert_eq!(p.line, Some(4));
    assert!(p.message.contains("has no images"), "{}", p.message);
}

/// A sheet is assigned by its path under assets/ too.
#[test]
fn a_sheet_is_assigned_by_its_path_under_assets() {
    let (temp, options) = starter("named-sheet");
    write_png(&temp.path("demo/assets/sprites/bug.png"), 8, 16, |x, _| {
        if x < 4 { BLACK } else { CLEAR }
    });
    fs::write(
        temp.path("demo/assets/palettes.txt"),
        format!("{PALETTES}sprites/bug.png = ember\n"),
    )
    .unwrap();
    let made = build(&rgbds(), &options).unwrap();
    let shades = made.images.unwrap().shades.unwrap();
    assert_eq!(shades.rule, ShadeRule::Declared, "the file was read");
}
