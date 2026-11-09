//! The GBC include API (leaf zcxvjc), generated from the build's results:
//! per image its GBC tiles (color art only; indexed art shares its DMG
//! tiles), attribute map and BG palettes; the OBJ palettes every sheet
//! shares; every named palette, with or without `--gbc`; and frame lists
//! whose attribute bytes carry the GBC palette beside the DMG bits.

use crate::common;
use crate::pngs;

use std::fs;

use gbcli_build::{BuildOptions, build};
use gbcli_protocol::{BuildTarget, GbcPalette, ImagesMade};

use common::{Temp, rgbds, starter};
use pngs::*;

const RED: Rgba = [0xFF, 0x00, 0x00, 0xFF];
const BLUE: Rgba = [0x00, 0x00, 0xFF, 0xFF];
const GREEN: Rgba = [0x00, 0xFF, 0x00, 0xFF];

fn gbc(options: &BuildOptions) -> BuildOptions {
    BuildOptions {
        target: BuildTarget::Gbc,
        ..options.clone()
    }
}

fn built(options: &BuildOptions) -> ImagesMade {
    let images = build(&rgbds(), options).unwrap().images.expect("images");
    assert!(images.first_error().is_none(), "{:#?}", images.problems);
    images
}

fn text(temp: &Temp, file: &str) -> String {
    fs::read_to_string(temp.path(&format!("demo/build/images/{file}"))).unwrap()
}

/// A color background, a color sheet, an indexed background and an
/// indexed sheet, with two named palettes, one unused.
fn project(temp: &Temp) {
    write_png(&temp.path("demo/assets/bg/town.png"), 16, 8, |x, _| {
        [RED, BLUE][(x / 8) as usize]
    });
    write_png(&temp.path("demo/assets/bg/sky.png"), 8, 8, |x, _| {
        if x < 4 { WHITE } else { BLACK }
    });
    write_png(
        &temp.path("demo/assets/sprites/bat.png"),
        16,
        16,
        |x, _| match x {
            0..=3 => GREEN,
            8..=11 => RED,
            _ => CLEAR,
        },
    );
    write_png(&temp.path("demo/assets/sprites/coin.png"), 8, 16, |x, _| {
        if x < 4 { BLACK } else { CLEAR }
    });
    fs::write(
        temp.path("demo/assets/palettes.txt"),
        "forest: #fff #8c4 #363 #021\nember_2: #ffe #f80 #a20 #200\n\
         bg/sky.png = forest\nsprites/coin.png = forest\n",
    )
    .unwrap();
}

fn dw(GbcPalette(p): &GbcPalette) -> String {
    let words: Vec<String> = p.iter().map(|c| format!("${:04X}", c.word())).collect();
    format!("\tdw {}\n", words.join(", "))
}

#[test]
fn a_gbc_build_labels_each_images_gbc_data() {
    let (temp, options) = starter("include-gbc");
    project(&temp);
    let images = built(&gbc(&options));
    let asm = text(&temp, "images.asm");
    let inc = text(&temp, "images.inc");
    for line in [
        "TownTilesGbc::\n\tINCBIN \"build/images/bg/town.gbc.2bpp\"\nTownTilesGbcEnd::",
        "TownAttrmap::\n\tINCBIN \"build/images/bg/town.attrmap\"\nTownAttrmapEnd::",
        "TownPalettes::\n",
        "TownPalettesEnd::",
        "SkyAttrmap::",
        "SkyPalettes::",
        "BatTilesGbc::\n\tINCBIN \"build/images/sprites/bat.gbc.2bpp\"\nBatTilesGbcEnd::",
        "ObjPalettesGbc::\n",
        "ObjPalettesGbcEnd::",
        "PaletteForest::\n",
        "PaletteForestEnd::",
        "PaletteEmber2::\n",
    ] {
        assert!(asm.contains(line), "{line:?} in\n{asm}");
    }
    // Indexed art's tiles are its DMG tiles: no GBC tile data of its own.
    for absent in ["SkyTilesGbc", "CoinTilesGbc"] {
        assert!(!asm.contains(absent), "{absent} in\n{asm}");
    }
    let made = images.gbc.as_ref().unwrap();
    let town = images.images.iter().find(|i| i.label == "Town").unwrap();
    let palettes = &town.gbc.as_ref().unwrap().palettes;
    let block: String = palettes.iter().map(dw).collect();
    assert!(
        asm.contains(&format!("TownPalettes::\n{block}TownPalettesEnd::")),
        "{block} in\n{asm}"
    );
    let obj: String = made.obj_palettes.iter().map(dw).collect();
    assert!(
        asm.contains(&format!("ObjPalettesGbc::\n{obj}ObjPalettesGbcEnd::")),
        "{obj} in\n{asm}"
    );
    for line in [
        format!("DEF TOWN_PALETTES EQU {}", palettes.len()),
        "DEF SKY_PALETTES EQU 1".into(),
        format!("DEF OBJ_PALETTES EQU {}", made.obj_palettes.len()),
        "DEF PALETTE_FOREST EQU 0".into(),
        "DEF PALETTE_EMBER_2 EQU 1".into(),
    ] {
        assert!(inc.contains(&line), "{line} in\n{inc}");
    }
    // The build defines GBCLI_GBC for every source (rgbasm -D), so the
    // include must not define it again.
    assert!(!inc.contains("DEF GBCLI_GBC "), "{inc}");
    // The attribute map: each map entry's slot, one byte each.
    let attr = fs::read(temp.path("demo/build/images/bg/town.attrmap")).unwrap();
    let slots: Vec<u8> = town
        .tile_map
        .iter()
        .map(|e| e.unwrap().palette.unwrap().get())
        .collect();
    assert_eq!(attr, slots);
    // Frame lists: each object's GBC slot in bits 0-2 (OBP0, no flip).
    let bat = images.images.iter().find(|i| i.label == "Bat").unwrap();
    let slot = |unit: usize| bat.tile_map[unit].unwrap().palette.unwrap().get();
    let frame = format!(
        "BatFrame0::\n\tdb 2\n\tdb 0, 0, 0, {}\n\tdb 0, 8, 2, {}\n",
        attr_byte(slot(0)),
        attr_byte(slot(1))
    );
    assert!(asm.contains(&frame), "{frame:?} in\n{asm}");
    // The coin's slot is the assigned palette's, slot 0, first.
    assert!(
        asm.contains("CoinFrame0::\n\tdb 1\n\tdb 0, 0, 0, 0\n"),
        "{asm}"
    );
    // Every constant images.inc defines is exported, so it is in the .sym
    // (a GBC build's, in build/gbc/).
    let sym = fs::read_to_string(temp.path("demo/build/gbc/demo.sym")).unwrap();
    for name in ["TOWN_PALETTES", "OBJ_PALETTES", "PALETTE_FOREST"] {
        assert!(
            sym.lines().any(|l| l.ends_with(&format!(" {name}"))),
            "{name} in\n{sym}"
        );
    }
    for label in [
        "TownTilesGbc",
        "TownAttrmap",
        "ObjPalettesGbc",
        "PaletteForest",
    ] {
        assert!(
            sym.lines().any(|l| l.ends_with(&format!(" {label}"))),
            "{label}"
        );
    }
}

fn attr_byte(slot: u8) -> String {
    match slot {
        0 => "0".into(),
        s => format!("${s:02X}"),
    }
}

/// Without --gbc: the named palettes still (plan question 10), and
/// nothing else of the GBC's.
#[test]
fn a_dmg_build_labels_the_named_palettes_and_nothing_else_gbc() {
    let (temp, options) = starter("include-dmg");
    project(&temp);
    built(&options);
    let asm = text(&temp, "images.asm");
    let inc = text(&temp, "images.inc");
    assert!(asm.contains("PaletteForest::\n\tdw $7FFF, "), "{asm}");
    assert!(inc.contains("DEF PALETTE_EMBER_2 EQU 1"), "{inc}");
    for absent in ["TilesGbc", "Attrmap", "TownPalettes", "ObjPalettesGbc"] {
        assert!(!asm.contains(absent), "{absent} in\n{asm}");
    }
    for absent in ["GBCLI_GBC", "OBJ_PALETTES", "TOWN_PALETTES"] {
        let def = format!("DEF {absent} ");
        assert!(!inc.contains(&def), "{def} in\n{inc}");
    }
    assert!(!temp.path("demo/build/images/bg/town.attrmap").exists());
    assert!(
        !temp
            .path("demo/build/images/bg/town.gbc.preview.png")
            .exists()
    );
}

/// Under --gbc each image has a GBC preview beside its DMG one, drawn in
/// palette RAM's colors as SameBoy shows them, and named in its result.
#[test]
fn a_gbc_build_previews_each_image_in_color() {
    let (temp, options) = starter("include-preview");
    project(&temp);
    let images = built(&gbc(&options));
    for image in &images.images {
        let preview = &image.gbc.as_ref().unwrap().preview;
        assert!(preview.ends_with(".gbc.preview.png"), "{preview}");
        assert!(temp.path(preview).is_file(), "{preview}");
    }
    let town = images.images.iter().find(|i| i.label == "Town").unwrap();
    let (_, _, pixels) = read_png(&temp.path(&town.gbc.as_ref().unwrap().preview));
    // Red as SameBoy draws $001F: (31 << 3) | (31 >> 2) = $FF.
    assert_eq!(pixels[0], 0xFF0000);
}

/// A palette's labels come from its name and an image's from its file
/// name: `forest_tiles` and `palette_forest.png` would both define
/// `PaletteForestTiles` (and `PALETTE_FOREST_TILES`), which gbcli says
/// itself, naming both, on either target.
#[test]
fn a_palette_and_an_image_that_would_share_a_symbol_are_a_layout_error() {
    for target in [BuildTarget::Dmg, BuildTarget::Gbc] {
        let (temp, options) = starter(&format!("include-collide-{target:?}"));
        write_png(
            &temp.path("demo/assets/bg/palette_forest.png"),
            8,
            8,
            |_, _| WHITE,
        );
        fs::write(
            temp.path("demo/assets/palettes.txt"),
            "forest_tiles: #fff #8c4 #363 #021\n",
        )
        .unwrap();
        let options = BuildOptions { target, ..options };
        match build(&rgbds(), &options) {
            Err(gbcli_build::BuildError::Layout { message, remedy }) => {
                for part in [
                    "demo/assets/bg/palette_forest.png",
                    "forest_tiles",
                    "PaletteForestTiles",
                ] {
                    assert!(message.contains(part), "{part:?} in {message}");
                }
                assert!(remedy.contains("rename"), "{remedy}");
            }
            other => panic!("expected a layout error, got {other:?}"),
        }
    }
}
