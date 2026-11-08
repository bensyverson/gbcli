//! Indexed art under `--gbc` (plan question 4, leaf k5Kadv): four-shade
//! art an assignment in `assets/palettes.txt` colors with a named palette.
//! Its tiles serve both machines; its tile map carries the palette's slot.

use crate::common;
use crate::pngs;

use std::fs;

use gbcli_build::{BuildOptions, build};
use gbcli_protocol::{
    BuildMade, BuildTarget, Flip, GbcPalette, ImageArt, ImageAsset, ImageLimit, ImagesMade,
    PaletteSlot, Rgb555, Severity, TileEntry,
};

use common::{Temp, rgbds, starter};
use pngs::*;

const PALETTES: &str = "dmg: #ffffff #aaaaaa #555555 #000000\n\
                        forest: #fff #8c4 #363 #021\n\
                        ember:  #ffe #f80 #a20 #200\n";

fn rgb(v: u32) -> Rgb555 {
    Rgb555::from_rgb888((v >> 16) as u8, (v >> 8) as u8, v as u8)
}

fn forest() -> GbcPalette {
    GbcPalette([0xFFFFFF, 0x88CC44, 0x336633, 0x002211].map(rgb))
}

fn ember() -> GbcPalette {
    GbcPalette([0xFFFFEE, 0xFF8800, 0xAA2200, 0x220000].map(rgb))
}

/// 16x16: four tiles, the left two alike, so the map reuses a tile.
fn sky(temp: &Temp) {
    write_png(&temp.path("demo/assets/bg/sky.png"), 16, 16, |x, y| {
        let shade = if x < 8 { (x + y) % 4 } else { (x * y) % 4 };
        [WHITE, LIGHT, DARK, BLACK][shade as usize]
    });
}

/// A 16x16 frame: a black stroke down column 1 of the left unit and a dark
/// block in the right unit's top-left, on transparent.
fn arrow(x: u32, y: u32) -> Rgba {
    match (x, y) {
        (1, _) => BLACK,
        (8..=10, 0..=2) => DARK,
        _ => CLEAR,
    }
}

/// Three 16x16 frames: the arrow, the arrow mirrored, and nothing.
fn hero(temp: &Temp, name: &str) {
    write_png(
        &temp.path(&format!("demo/assets/sprites/{name}.16x16.png")),
        48,
        16,
        |x, y| match x / 16 {
            0 => arrow(x, y),
            1 => arrow(15 - (x - 16), y),
            _ => CLEAR,
        },
    );
}

fn palettes(temp: &Temp, assignments: &str) {
    fs::write(
        temp.path("demo/assets/palettes.txt"),
        format!("{PALETTES}{assignments}"),
    )
    .unwrap();
}

fn gbc(options: &BuildOptions) -> BuildOptions {
    BuildOptions {
        target: BuildTarget::Gbc,
        ..options.clone()
    }
}

fn images(made: BuildMade) -> ImagesMade {
    let images = made.images.expect("images");
    assert!(images.first_error().is_none(), "{:?}", images.problems);
    images
}

fn image<'a>(images: &'a ImagesMade, source: &str) -> &'a ImageAsset {
    images
        .images
        .iter()
        .find(|i| i.source.ends_with(source))
        .unwrap_or_else(|| panic!("no {source}"))
}

fn slot(n: u8) -> Option<PaletteSlot> {
    PaletteSlot::new(n)
}

/// An indexed background's GBC tile data is its DMG tile data, byte for
/// byte; its tile map is its tilemap, and its attribute map names one
/// slot, 0, the assigned palette's.
#[test]
fn an_indexed_background_shares_its_tiles_and_names_one_slot() {
    let (temp, options) = starter("indexed-bg");
    sky(&temp);
    palettes(&temp, "bg/sky.png = forest\n");
    let read = |f: &str| fs::read(temp.path(&format!("demo/build/images/bg/{f}"))).unwrap();

    let dmg = images(build(&rgbds(), &options).unwrap());
    let sky_dmg = image(&dmg, "sky.png");
    assert_eq!((sky_dmg.gbc.as_ref(), sky_dmg.tile_map.len()), (None, 0));
    assert_eq!(dmg.gbc, None);
    let dmg_tiles = read("sky.2bpp");

    let made = images(build(&rgbds(), &gbc(&options)).unwrap());
    assert_eq!(read("sky.2bpp"), dmg_tiles);
    let mut written: Vec<String> = fs::read_dir(temp.path("demo/build/images/bg"))
        .unwrap()
        .map(|e| e.unwrap().file_name().to_string_lossy().into_owned())
        .collect();
    written.sort();
    assert_eq!(
        written,
        ["sky.2bpp", "sky.preview.png", "sky.tilemap"],
        "no GBC tile data of its own"
    );
    let sky = image(&made, "sky.png");
    let gbc_sky = sky.gbc.as_ref().expect("a GBC version");
    assert_eq!(
        gbc_sky.art,
        ImageArt::Indexed {
            palette: "forest".into()
        }
    );
    assert_eq!(gbc_sky.palettes, [forest()]);
    let tilemap = read("sky.tilemap");
    let expected: Vec<Option<TileEntry>> = tilemap
        .iter()
        .map(|&t| {
            Some(TileEntry {
                index: u32::from(t),
                flip: Flip::None,
                palette: slot(0),
            })
        })
        .collect();
    assert_eq!(sky.tile_map, expected);
    assert_eq!(sky.tile_map.len(), 4);
}

/// A sheet's tile map carries each unit's flip, its top tile and the
/// assigned palette's OBJ slot; a transparent unit has none.
#[test]
fn an_indexed_sheet_carries_flips_and_its_slot() {
    let (temp, options) = starter("indexed-sheet");
    hero(&temp, "hero");
    palettes(&temp, "sprites/hero.16x16.png = ember\n");
    let made = images(build(&rgbds(), &gbc(&options)).unwrap());
    let hero = image(&made, "hero.16x16.png");
    let at = |index, flip| {
        Some(TileEntry {
            index,
            flip,
            palette: slot(0),
        })
    };
    assert_eq!(
        hero.tile_map,
        [
            at(0, Flip::None),
            at(2, Flip::None),
            at(2, Flip::X),
            at(0, Flip::X),
            None,
            None
        ]
    );
    let gbc = hero.gbc.as_ref().unwrap();
    assert!(gbc.palettes.is_empty(), "a sheet's are the shared ones");
    assert_eq!(made.gbc.unwrap().obj_palettes, [ember()]);
}

/// Two sheets assigned one palette share its slot; the slots follow the
/// palettes' order in the file, not the images'.
#[test]
fn two_sheets_assigned_one_palette_share_its_slot() {
    let (temp, options) = starter("indexed-share");
    for name in ["coin", "gem", "hero"] {
        hero(&temp, name);
    }
    palettes(
        &temp,
        "sprites/coin.16x16.png = ember\n\
         sprites/hero.16x16.png = ember\n\
         sprites/gem.16x16.png = forest\n",
    );
    let made = images(build(&rgbds(), &gbc(&options)).unwrap());
    let first_slot = |name: &str| {
        image(&made, name).tile_map[0]
            .expect("an opaque unit")
            .palette
    };
    assert_eq!(first_slot("coin.16x16.png"), slot(1));
    assert_eq!(first_slot("hero.16x16.png"), slot(1));
    assert_eq!(first_slot("gem.16x16.png"), slot(0));
    assert_eq!(made.gbc.unwrap().obj_palettes, [forest(), ember()]);
}

/// Nine palettes assigned to sheets can't fit the GBC's eight OBJ
/// palettes: `gbc_palettes`, naming the count.
#[test]
fn nine_assigned_obj_palettes_overflow_the_eight() {
    let (temp, options) = starter("indexed-nine");
    let mut text = String::from("dmg: #ffffff #aaaaaa #555555 #000000\n");
    for i in 0..9 {
        hero(&temp, &format!("h{i}"));
        text += &format!("p{i}: #fff #8c4 #363 #02{i}\n");
    }
    for i in 0..9 {
        text += &format!("sprites/h{i}.16x16.png = p{i}\n");
    }
    fs::write(temp.path("demo/assets/palettes.txt"), text).unwrap();
    let made = match build(&rgbds(), &gbc(&options)) {
        Err(gbcli_build::BuildError::Failed { made, .. }) => made,
        other => panic!("{other:?}"),
    };
    let p = problem(&made, ImageLimit::GbcPalettes);
    assert_eq!((p.severity, p.count, p.max), (Severity::Error, 9, 8));
    assert!(p.message.contains("p8"), "{}", p.message);
}

/// Without `--gbc`, nothing GBC is reported (the DMG JSON is unchanged).
#[test]
fn a_dmg_build_reports_no_gbc_for_indexed_art() {
    let (temp, options) = starter("indexed-dmg");
    sky(&temp);
    hero(&temp, "hero");
    palettes(
        &temp,
        "bg/sky.png = forest\nsprites/hero.16x16.png = ember\n",
    );
    let made = images(build(&rgbds(), &options).unwrap());
    assert_eq!(made.gbc, None);
    for i in &made.images {
        assert_eq!(
            (i.gbc.as_ref(), i.tile_map.len()),
            (None, 0),
            "{}",
            i.source
        );
    }
}
