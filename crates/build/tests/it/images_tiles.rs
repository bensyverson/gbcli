//! Tilesets (`assets/tiles/*.png`, leaf 7gGMeZ): unique 8x8 background
//! tiles and no map, deduplicated as a background is (on the DMG and GBC
//! pair under `--gbc`, never by a flip). The program builds its own maps,
//! from `<Name>Tiles` and the result's `tile_map`; under `--gbc` it also
//! gets each tile's BG palette (`<Name>Attrs`) and the palettes.

use crate::common;
use crate::pngs;

use std::fs;

use gbcli_build::{BuildOptions, build};
use gbcli_protocol::{BuildTarget, ImageArt, ImageAsset, ImageKind, ImagesMade, TileEntry};

use common::{Temp, rgbds, starter};
use pngs::*;

const RED: Rgba = [0xFF, 0x00, 0x00, 0xFF];
const BLUE: Rgba = [0x00, 0x00, 0xFF, 0xFF];

fn built(options: &BuildOptions) -> ImagesMade {
    let images = build(&rgbds(), options).unwrap().images.expect("images");
    assert!(images.first_error().is_none(), "{:#?}", images.problems);
    let json = serde_json::to_string(&images).unwrap();
    let read: ImagesMade = serde_json::from_str(&json).unwrap_or_else(|e| panic!("{e}: {json}"));
    assert_eq!(read, images);
    images
}

fn text(temp: &Temp, file: &str) -> String {
    fs::read_to_string(temp.path(&format!("demo/build/images/{file}"))).unwrap()
}

fn font(images: &ImagesMade) -> &ImageAsset {
    images
        .images
        .iter()
        .find(|i| i.kind == ImageKind::Tiles)
        .unwrap_or_else(|| panic!("no tileset in {images:#?}"))
}

/// A 32x8 tileset of four tiles: tiles 0 and 2 alike, tile 3 tile 1
/// mirrored, which a tileset keeps (no flips).
fn font_png(temp: &Temp) {
    write_png(&temp.path("demo/assets/tiles/font.png"), 32, 8, |x, y| {
        let (t, x) = (x / 8, x % 8);
        let lit = match t {
            0 | 2 => x == y,
            1 => x < 2,
            _ => x >= 6,
        };
        if lit { BLACK } else { WHITE }
    });
}

/// A source that names the labels and constants a tileset gets, and
/// `more`, so the build proves they resolve.
fn uses(temp: &Temp, more: &str) {
    let asm = format!(
        "INCLUDE \"images.inc\"\nSECTION \"uses font\", ROMX\n\
         dw FontTiles, FontTilesEnd\ndb FONT_TILES\n{more}"
    );
    fs::write(temp.path("demo/src/uses_font.asm"), asm).unwrap();
}

/// What a GBC tileset adds; color art adds its own tiles too.
const GBC_USES: &str =
    "dw FontAttrs, FontAttrsEnd, FontPalettes, FontPalettesEnd\ndb FONT_PALETTES\n";
const COLOR_USES: &str = "dw FontTilesGbc, FontTilesGbcEnd\n";

#[test]
fn a_tileset_is_its_unique_tiles_and_a_tile_map_with_no_map_of_its_own() {
    let (temp, options) = starter("tiles-dmg");
    font_png(&temp);
    uses(&temp, "");
    let images = built(&options);
    let f = font(&images);
    assert_eq!(
        (f.label.as_str(), f.width, f.height, f.tiles),
        ("Font", 4, 1, 3)
    );
    let indices: Vec<u32> = f.tile_map.iter().map(|e| e.unwrap().index).collect();
    assert_eq!(indices, [0, 1, 0, 2]);
    let plain = |e: &Option<TileEntry>| e.is_some_and(|e| e.flip.is_none() && e.palette.is_none());
    assert!(f.tile_map.iter().all(plain), "{:?}", f.tile_map);
    assert!(f.gbc.is_none());
    assert!(
        f.preview
            .ends_with("demo/build/images/tiles/font.preview.png"),
        "{}",
        f.preview
    );
    assert_eq!(
        fs::read(temp.path("demo/build/images/tiles/font.2bpp"))
            .unwrap()
            .len(),
        3 * 16
    );
    assert!(!temp.path("demo/build/images/tiles/font.tilemap").exists());
    let asm = text(&temp, "images.asm");
    assert!(
        asm.contains("FontTiles::\n\tINCBIN \"build/images/tiles/font.2bpp\"\nFontTilesEnd::"),
        "{asm}"
    );
    assert!(!asm.contains("FontTilemap"), "{asm}");
    let inc = text(&temp, "images.inc");
    assert!(inc.contains("DEF FONT_TILES EQU 3"), "{inc}");
}

#[test]
fn a_gbc_tileset_has_its_gbc_tiles_each_tiles_palette_and_the_palettes() {
    let (temp, mut options) = starter("tiles-gbc");
    options.target = BuildTarget::Gbc;
    // Tiles 0 and 1 drawn alike, in red and in blue: two joint tiles
    // whatever their grays; tile 2 repeats tile 0 in both versions.
    write_png(&temp.path("demo/assets/tiles/font.png"), 24, 8, |x, y| {
        let lit = x % 8 == y;
        match (x / 8, lit) {
            (_, false) => WHITE,
            (1, true) => BLUE,
            (_, true) => RED,
        }
    });
    uses(&temp, &format!("{GBC_USES}{COLOR_USES}"));
    let images = built(&options);
    let f = font(&images);
    assert_eq!(f.tiles, 2);
    let indices: Vec<u32> = f.tile_map.iter().map(|e| e.unwrap().index).collect();
    assert_eq!(indices, [0, 1, 0]);
    let g = f.gbc.as_ref().expect("a GBC version");
    assert_eq!(g.art, ImageArt::Color);
    assert!(
        g.preview.ends_with("tiles/font.gbc.preview.png"),
        "{}",
        g.preview
    );
    let slots: Vec<u8> = f
        .tile_map
        .iter()
        .map(|e| e.unwrap().palette.unwrap().get())
        .collect();
    let asm = text(&temp, "images.asm");
    let attrs = format!(
        "FontAttrs::\n\tdb {}, {}\nFontAttrsEnd::",
        slots[0], slots[1]
    );
    assert!(asm.contains(&attrs), "{attrs} in\n{asm}");
    assert!(
        asm.contains("FontTilesGbc::\n\tINCBIN \"build/images/tiles/font.gbc.2bpp\""),
        "{asm}"
    );
    assert!(!temp.path("demo/build/images/tiles/font.attrmap").exists());
    let inc = text(&temp, "images.inc");
    assert!(
        inc.contains(&format!("DEF FONT_PALETTES EQU {}", g.palettes.len())),
        "{inc}"
    );
}

#[test]
fn an_assigned_tileset_is_indexed_art_in_its_palettes_slot() {
    let (temp, mut options) = starter("tiles-indexed");
    options.target = BuildTarget::Gbc;
    font_png(&temp);
    uses(&temp, GBC_USES);
    fs::write(
        temp.path("demo/assets/palettes.txt"),
        "forest: #fff #8c4 #363 #021\ntiles/font.png = forest\n",
    )
    .unwrap();
    let images = built(&options);
    let f = font(&images);
    assert_eq!(
        f.gbc.as_ref().map(|g| &g.art),
        Some(&ImageArt::Indexed {
            palette: "forest".into()
        })
    );
    assert!(
        f.tile_map
            .iter()
            .all(|e| e.unwrap().palette.unwrap().get() == 0)
    );
    let asm = text(&temp, "images.asm");
    assert!(
        asm.contains("FontAttrs::\n\tdb 0, 0, 0\nFontAttrsEnd::"),
        "{asm}"
    );
    // Indexed art draws its DMG tiles on both machines: no GBC tiles of
    // its own, so the label points at them.
    assert!(!temp.path("demo/build/images/tiles/font.gbc.2bpp").exists());
}

/// A tileset and a background can't share a name: both would be
/// `TownTiles`.
#[test]
fn a_tileset_named_like_a_background_is_a_layout_error() {
    let (temp, options) = starter("tiles-collide");
    write_png(&temp.path("demo/assets/tiles/town.png"), 8, 8, |_, _| WHITE);
    write_png(&temp.path("demo/assets/bg/town.png"), 8, 8, |_, _| WHITE);
    let err = build(&rgbds(), &options).unwrap_err().body();
    assert!(
        err.message.contains("TownTiles") || err.message.contains("`Town`"),
        "{}",
        err.message
    );
    assert!(
        err.message.contains("assets/tiles/town.png"),
        "{}",
        err.message
    );
}
