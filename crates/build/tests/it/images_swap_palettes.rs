//! Swap palettes (leaf mAlE2F, the form of Ben's ruling under plan
//! question 4): the GBC has no OBP, so an indexed sheet's slot holds its
//! palette reordered by its OBP, and a program swapping another named
//! palette into that slot needs it reordered the same way. Each indexed
//! sheet gets `<Sheet>Palette<Name>..End` for every named palette. And the
//! labels palettes make, these among them, never collide silently.

use crate::common;
use crate::pngs;

use std::fs;

use gbcli_build::{BuildError, BuildOptions, build};
use gbcli_protocol::{
    BuildTarget, GbcPalette, ImageAsset, ImageLimit, ImagesMade, Rgb555, Severity,
};

use common::{Temp, rgbds, starter};
use pngs::*;

const PALETTES: &str = "dmg: #ffffff #aaaaaa #555555 #000000\n\
                        ember: #ffe #f80 #a20 #200\n\
                        ice: #fff #8cf #36a #013\n";

fn rgb(v: u32) -> Rgb555 {
    Rgb555::from_rgb888((v >> 16) as u8, (v >> 8) as u8, v as u8)
}

fn ember() -> GbcPalette {
    GbcPalette([0xFFFFEE, 0xFF8800, 0xAA2200, 0x220000].map(rgb))
}

fn ice() -> GbcPalette {
    GbcPalette([0xFFFFFF, 0x88CCFF, 0x3366AA, 0x001133].map(rgb))
}

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

fn image<'a>(images: &'a ImagesMade, source: &str) -> &'a ImageAsset {
    images
        .images
        .iter()
        .find(|i| i.source.ends_with(source))
        .unwrap_or_else(|| panic!("no {source}"))
}

fn asm(temp: &Temp) -> String {
    fs::read_to_string(temp.path("demo/build/images/images.asm")).unwrap()
}

/// The palette between `label::` and `<label>End::` in `asm`: one `dw`
/// line of four RGB555 words.
fn palette_at(asm: &str, label: &str) -> [u16; 4] {
    let lines: Vec<&str> = asm
        .lines()
        .skip_while(|l| *l != format!("{label}::"))
        .skip(1)
        .take_while(|l| *l != format!("{label}End::"))
        .collect();
    let [line] = lines[..] else {
        panic!("{label}: one dw line, got {lines:?} in\n{asm}")
    };
    let words: Vec<u16> = line
        .trim()
        .strip_prefix("dw ")
        .unwrap_or_else(|| panic!("{line}"))
        .split(", ")
        .map(|w| u16::from_str_radix(w.strip_prefix('$').unwrap(), 16).unwrap())
        .collect();
    words.try_into().unwrap()
}

fn words(GbcPalette(colors): GbcPalette) -> [u16; 4] {
    colors.map(Rgb555::word)
}

fn shade(c: Rgba) -> usize {
    [WHITE, LIGHT, DARK, BLACK]
        .iter()
        .position(|&g| g == c)
        .unwrap_or_else(|| panic!("{c:?} is no declared shade"))
}

/// White, dark and black on transparent (OBP %11_10_00_00): index 1 is
/// shade 0, so a palette loaded as written would draw it in entry 1.
fn coin(x: u32, _y: u32) -> Rgba {
    match x {
        0 => WHITE,
        1 => DARK,
        2 => BLACK,
        _ => CLEAR,
    }
}

fn coin_project(name: &str) -> (Temp, BuildOptions) {
    let (temp, options) = starter(name);
    write_png(&temp.path("demo/assets/sprites/coin.png"), 8, 16, coin);
    fs::write(
        temp.path("demo/assets/palettes.txt"),
        format!("{PALETTES}sprites/coin.png = ember\n"),
    )
    .unwrap();
    (temp, options)
}

/// Loading `CoinPaletteIce` into coin's slot draws every opaque pixel in
/// its shade's color from ice: the tile data's index, through the swap
/// palette, is ice's entry for the pixel's shade. `CoinPaletteEmber`, its
/// assigned palette's, is what the slot already holds.
#[test]
fn loading_a_sheets_swap_palette_draws_that_palettes_colors_for_its_shades() {
    let (temp, options) = coin_project("swap-coin");
    let made = built(&gbc(&options));
    let asm = asm(&temp);
    let swap = palette_at(&asm, "CoinPaletteIce");
    let GbcPalette(i) = ice();
    assert_eq!(
        swap,
        words(GbcPalette([i[0], i[0], i[2], i[3]])),
        "ice by OBP"
    );
    let entry = image(&made, "coin.png").tile_map[0].expect("an opaque unit");
    let slot = usize::from(entry.palette.expect("a slot").get());
    let obj = &made.gbc.as_ref().unwrap().obj_palettes;
    assert_eq!(palette_at(&asm, "CoinPaletteEmber"), words(obj[slot]));

    let data = fs::read(temp.path("demo/build/images/sprites/coin.2bpp")).unwrap();
    for y in 0..16 {
        for x in 0..8 {
            let index = index_at(&data[(y / 8) as usize * 16..], (y % 8) as usize, x);
            match coin(x, y) {
                CLEAR => assert_eq!(index, 0, "({x}, {y})"),
                c => assert_eq!(
                    swap[usize::from(index)],
                    ice().0[shade(c)].word(),
                    "({x}, {y}): shade {} through CoinPaletteIce",
                    shade(c)
                ),
            }
        }
    }
    // The named palettes as written are still there, for backgrounds.
    assert_eq!(palette_at(&asm, "PaletteIce"), words(ice()));
    assert_eq!(palette_at(&asm, "PaletteEmber"), words(ember()));
}

/// Only an indexed sheet under `--gbc` needs one: a background's index is
/// its shade, a color sheet has no named palette, and a DMG build has no
/// GBC palettes at all.
#[test]
fn only_an_indexed_sheet_on_the_gbc_gets_swap_palettes() {
    let (temp, options) = coin_project("swap-only");
    write_png(&temp.path("demo/assets/bg/sky.png"), 8, 8, |x, _| {
        [WHITE, LIGHT, DARK, BLACK][(x % 4) as usize]
    });
    write_png(&temp.path("demo/assets/sprites/bat.png"), 8, 16, |x, _| {
        if x < 2 { [0xFF, 0, 0, 0xFF] } else { CLEAR }
    });
    fs::write(
        temp.path("demo/assets/palettes.txt"),
        format!("{PALETTES}sprites/coin.png = ember\nbg/sky.png = ice\n"),
    )
    .unwrap();
    built(&gbc(&options));
    let asm = asm(&temp);
    assert!(asm.contains("CoinPaletteIce::"), "{asm}");
    for label in [
        "SkyPaletteIce",
        "SkyPaletteEmber",
        "BatPaletteIce",
        "BatPaletteEmber",
    ] {
        assert!(!asm.contains(label), "{label} in\n{asm}");
    }
    built(&options);
    let asm = self::asm(&temp);
    assert!(!asm.contains("CoinPaletteIce"), "a DMG build:\n{asm}");
    assert!(asm.contains("PaletteIce::"), "{asm}");
}

/// `forest2` and `forest_2` are both `PaletteForest2`: the file's problem,
/// at the second line, naming the first, not an rgbasm failure.
#[test]
fn two_palettes_with_one_label_are_a_palette_file_problem() {
    let (temp, options) = starter("swap-labels");
    write_png(&temp.path("demo/assets/bg/town.png"), 8, 8, |_, _| WHITE);
    fs::write(
        temp.path("demo/assets/palettes.txt"),
        "forest2: #fff #8c4 #363 #021\n# a comment\nforest_2: #fff #8c4 #363 #021\n",
    )
    .unwrap();
    let made = refused(&options);
    let p = problem(&made, ImageLimit::PaletteFile);
    assert_eq!(p.line, Some(3));
    for part in ["line 3", "forest_2", "forest2", "line 1", "PaletteForest2"] {
        assert!(p.message.contains(part), "{part:?} in {}", p.message);
    }
    assert!(p.remedy.contains("rename"), "{}", p.remedy);
}

/// With no image converted no palette's label is emitted, so two names
/// that make one are harmless: the project built before the check, and
/// still does, with a warning that says what would collide.
#[test]
fn colliding_palette_labels_only_warn_when_no_image_is_converted() {
    let (temp, options) = starter("swap-labels-no-images");
    fs::write(
        temp.path("demo/assets/palettes.txt"),
        "forest2: #fff #8c4 #363 #021\nforest_2: #fff #8c4 #363 #021\n",
    )
    .unwrap();
    let made = build(&rgbds(), &options).unwrap();
    let problems = made.images.map(|i| i.problems).unwrap_or_default();
    let p = problems
        .iter()
        .find(|p| p.limit == ImageLimit::PaletteFile)
        .unwrap_or_else(|| panic!("{problems:?}"));
    assert_eq!(p.severity, Severity::Warning);
    assert_eq!(p.line, Some(2));
    assert!(p.message.contains("PaletteForest2"), "{}", p.message);
}

/// The layout error a build must stop with, for every part in `parts`.
fn layout_error(options: &BuildOptions, parts: &[&str]) {
    match build(&rgbds(), options) {
        Err(BuildError::Layout { message, remedy }) => {
            for part in parts {
                assert!(message.contains(part), "{part:?} in {message}");
            }
            assert!(remedy.contains("rename"), "{remedy}");
        }
        other => panic!("expected a layout error, got {other:?}"),
    }
}

fn sheet(temp: &Temp, file: &str) {
    write_png(
        &temp.path(&format!("demo/assets/sprites/{file}")),
        8,
        16,
        coin,
    );
}

/// A sheet's swap labels are reserved on either target, assigned or not,
/// so neither `--gbc` nor an assignment can bring a collision in. Each
/// case is a layout error naming what collides.
#[test]
fn swap_labels_that_collide_are_a_layout_error() {
    // A sheet `palette` with `ice` is `PalettePaletteIce`, the palette
    // `palette_ice`'s label.
    for target in [BuildTarget::Dmg, BuildTarget::Gbc] {
        let (temp, options) = starter(&format!("swap-vs-palette-{target:?}"));
        sheet(&temp, "palette.png");
        fs::write(
            temp.path("demo/assets/palettes.txt"),
            "ice: #fff #8cf #36a #013\npalette_ice: #fff #8cf #36a #013\n",
        )
        .unwrap();
        let options = BuildOptions { target, ..options };
        layout_error(
            &options,
            &["sprites/palette.png", "palette_ice", "PalettePaletteIce"],
        );
    }

    // A sheet `hero` with `ice_tiles` is `HeroPaletteIceTiles`, the
    // background `hero_palette_ice`'s tiles.
    let (temp, options) = starter("swap-vs-image");
    sheet(&temp, "hero.png");
    write_png(
        &temp.path("demo/assets/bg/hero_palette_ice.png"),
        8,
        8,
        |_, _| WHITE,
    );
    fs::write(
        temp.path("demo/assets/palettes.txt"),
        "ice_tiles: #fff #8cf #36a #013\n",
    )
    .unwrap();
    layout_error(
        &options,
        &[
            "sprites/hero.png",
            "bg/hero_palette_ice.png",
            "ice_tiles",
            "HeroPaletteIceTiles",
        ],
    );

    // Sheets `a` and `a_palette_b` with `b_palette_c` and `c`: both
    // `APaletteBPaletteC`.
    let (temp, options) = starter("swap-vs-swap");
    sheet(&temp, "a.png");
    sheet(&temp, "a_palette_b.png");
    fs::write(
        temp.path("demo/assets/palettes.txt"),
        "b_palette_c: #fff #8cf #36a #013\nc: #fff #8cf #36a #013\n",
    )
    .unwrap();
    layout_error(
        &options,
        &[
            "sprites/a.png",
            "sprites/a_palette_b.png",
            "b_palette_c",
            "APaletteBPaletteC",
        ],
    );
}

/// A frame's label ends in `Frame<n>`, which a palette's name can make
/// too: `x_frame0` is `PaletteXFrame0`, the sheet `palette_x`'s first
/// frame, and with a sheet `hero`, `HeroPaletteXFrame0`, the sheet
/// `hero_palette_x`'s.
#[test]
fn a_palette_label_that_is_a_frames_is_a_layout_error() {
    let (temp, options) = starter("swap-vs-frame-palette");
    sheet(&temp, "palette_x.png");
    fs::write(
        temp.path("demo/assets/palettes.txt"),
        "x_frame0: #fff #8cf #36a #013\n",
    )
    .unwrap();
    layout_error(
        &options,
        &["sprites/palette_x.png", "x_frame0", "PaletteXFrame0"],
    );

    let (temp, options) = starter("swap-vs-frame-swap");
    sheet(&temp, "hero.png");
    sheet(&temp, "hero_palette_x.png");
    fs::write(
        temp.path("demo/assets/palettes.txt"),
        "x_frame0: #fff #8cf #36a #013\n",
    )
    .unwrap();
    layout_error(
        &options,
        &[
            "sprites/hero.png",
            "sprites/hero_palette_x.png",
            "x_frame0",
            "HeroPaletteXFrame0",
        ],
    );
}
