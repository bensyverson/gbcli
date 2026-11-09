//! Edges of the joint conversion under `build --gbc` (leaf mAlE2F, from
//! the review of 8cfa480, 20729c8 and 793da0d): each tile indexed into the
//! palette gbcli gave its unit, and the messages of the limits that count
//! joint tiles.

use crate::common;
use crate::pngs;

use std::fs;

use gbcli_build::BuildOptions;
use gbcli_protocol::{BuildTarget, ImageAsset, ImageLimit, ImageProblem, ImagesMade, Rgb555};

use common::{Temp, rgbds, starter};
use pngs::*;

const RED: Rgba = [0xFF, 0x00, 0x00, 0xFF];
const GREEN: Rgba = [0x00, 0xFF, 0x00, 0xFF];
const BLUE: Rgba = [0x00, 0x00, 0xFF, 0xFF];
const YELLOW: Rgba = [0xFF, 0xFF, 0x00, 0xFF];
const CYAN: Rgba = [0x00, 0xFF, 0xFF, 0xFF];

fn gbc(name: &str) -> (Temp, BuildOptions) {
    let (temp, mut options) = starter(name);
    options.target = BuildTarget::Gbc;
    (temp, options)
}

fn built(options: &BuildOptions) -> ImagesMade {
    let images = gbcli_build::build(&rgbds(), options)
        .unwrap()
        .images
        .expect("images");
    assert!(images.first_error().is_none(), "{:#?}", images.problems);
    images
}

fn image<'a>(images: &'a ImagesMade, source: &str) -> &'a ImageAsset {
    images
        .images
        .iter()
        .find(|i| i.source.ends_with(source))
        .unwrap_or_else(|| panic!("no {source}"))
}

/// A 3-gray DMG version of a color sheet, transparent where it is.
fn gray3(c: Rgba) -> Rgba {
    match c {
        CLEAR => CLEAR,
        RED => BLACK,
        GREEN | YELLOW => DARK,
        _ => LIGHT,
    }
}

/// Unit A holds red, green and blue; unit B red on top and yellow, cyan
/// below. B's top 8x8 tile fits A's palette too, so which palette draws it
/// is a choice; gbcli gives B's unit B's palette.
fn split(x: u32, y: u32) -> Rgba {
    match (x / 8, x % 8, y) {
        (_, 4.., _) => CLEAR,
        (0, 0, _) => RED,
        (0, 1, _) => GREEN,
        (0, 2, _) => BLUE,
        (1, _, 0..=7) => RED,
        (1, 0..=1, _) => YELLOW,
        (1, _, _) => CYAN,
        _ => CLEAR,
    }
}

/// Both units fit more than one palette (B's top 8x8 tile fits A's too),
/// and gbcli indexes every tile into the unit's own slot: every pixel of
/// the GBC data, through that slot, is the color drawn. rgbgfx chose per
/// 8x8 tile and could pick another; gbcli does not.
#[test]
fn a_tiles_colors_are_indexed_into_its_units_slot() {
    let (temp, options) = gbc("joint-reindex");
    write_png(&temp.path("demo/assets/sprites/bat.png"), 16, 16, split);
    write_png(
        &temp.path("demo/assets/sprites/bat.dmg.png"),
        16,
        16,
        |x, y| gray3(split(x, y)),
    );
    let made = built(&options);
    let bat = image(&made, "bat.png");
    let slots: Vec<u8> = bat
        .tile_map
        .iter()
        .map(|e| e.unwrap().palette.unwrap().get())
        .collect();
    assert_ne!(slots[0], slots[1], "two palettes: {slots:?}");

    let obj = &made.gbc.as_ref().unwrap().obj_palettes;
    let data = fs::read(temp.path("demo/build/images/sprites/bat.gbc.2bpp")).unwrap();
    for (unit, e) in bat.tile_map.iter().enumerate() {
        let e = e.unwrap();
        let slot = usize::from(e.palette.unwrap().get());
        for y in 0..16 {
            for x in 0..8 {
                let tile = (e.index + y / 8) as usize * 16;
                let index = index_at(&data[tile..], (y % 8) as usize, x);
                let (px, py) = (unit as u32 * 8 + x, y);
                match split(px, py) {
                    CLEAR => assert_eq!(index, 0, "transparent at ({px}, {py})"),
                    c => assert_eq!(
                        obj[slot].0[usize::from(index)],
                        Rgb555::from_rgb888(c[0], c[1], c[2]),
                        "({px}, {py}): slot {slot}, index {index}"
                    ),
                }
            }
        }
    }
}

/// A joint-counted limit's problem names the color image, not its DMG
/// version, and says tiles alike on the DMG count apart when their colors
/// differ.
fn assert_joint(p: &ImageProblem, color: &str, what: &str) {
    let image = p.image.as_deref().unwrap_or("");
    assert!(
        image.ends_with(color) && !image.contains(".dmg."),
        "{:?}: image={image}",
        p.limit
    );
    for part in [what, "distinct in color"] {
        assert!(p.message.contains(part), "{part:?} in {}", p.message);
    }
    assert!(!p.message.contains(".dmg.png"), "{}", p.message);
    assert!(!p.remedy.contains(".dmg.png"), "{}", p.remedy);
}

/// 257 tiles, each its own color, over a DMG version of one shade: one
/// tile on the DMG, 257 joint tiles.
#[test]
fn too_many_joint_tiles_for_a_map_names_the_color_image() {
    let (temp, options) = gbc("joint-257");
    let solid = |x: u32, _y: u32| {
        let n = x / 8;
        [(n % 32 * 8) as u8, (n / 32 * 8) as u8, 0, 0xFF]
    };
    write_png(&temp.path("demo/assets/bg/town.png"), 257 * 8, 8, solid);
    write_png(
        &temp.path("demo/assets/bg/town.dmg.png"),
        257 * 8,
        8,
        |_, _| DARK,
    );
    let made = refused(&options);
    let p = problem(&made, ImageLimit::BgTilesMap);
    assert_joint(&p, "bg/town.png", "town.png needs 257 unique tiles");
}

/// Unit `n` of a sheet: its first `n` pixels in reading order red, the
/// rest green; no two units are alike, flipped or not.
fn units(x: u32, y: u32) -> Rgba {
    let n = x / 8;
    if y * 8 + x % 8 < n { RED } else { GREEN }
}

/// 129 distinct color units over a DMG version of one shade: 1 object on
/// the DMG, 258 joint tiles.
#[test]
fn too_many_joint_objects_names_the_color_image() {
    let (temp, options) = gbc("joint-objects");
    write_png(
        &temp.path("demo/assets/sprites/hero.8x16.png"),
        129 * 8,
        16,
        units,
    );
    write_png(
        &temp.path("demo/assets/sprites/hero.8x16.dmg.png"),
        129 * 8,
        16,
        |_, _| DARK,
    );
    let made = refused(&options);
    let p = problem(&made, ImageLimit::ObjTiles);
    assert_joint(&p, "sprites/hero.8x16.png", "hero.8x16.png needs 258 tiles");
}

/// A background of 200 joint tiles and a sheet of 100 joint units: 400 of
/// VRAM's 384, though the DMG versions are a tile and an object.
#[test]
fn too_many_joint_tiles_for_vram_say_they_count_color() {
    let (temp, options) = gbc("joint-vram");
    let solid = |x: u32, _y: u32| {
        let n = x / 8;
        [(n % 32 * 8) as u8, (n / 32 * 8) as u8, 0, 0xFF]
    };
    write_png(&temp.path("demo/assets/bg/town.png"), 200 * 8, 8, solid);
    write_png(
        &temp.path("demo/assets/bg/town.dmg.png"),
        200 * 8,
        8,
        |_, _| DARK,
    );
    write_png(
        &temp.path("demo/assets/sprites/hero.8x16.png"),
        100 * 8,
        16,
        units,
    );
    write_png(
        &temp.path("demo/assets/sprites/hero.8x16.dmg.png"),
        100 * 8,
        16,
        |_, _| DARK,
    );
    let made = refused(&options);
    let p = problem(&made, ImageLimit::VramTiles);
    for part in [
        "town.png uses 200 tiles",
        "hero.8x16.png 200",
        "distinct in color",
    ] {
        assert!(p.message.contains(part), "{part:?} in {}", p.message);
    }
    assert!(!p.message.contains(".dmg.png"), "{}", p.message);
}
