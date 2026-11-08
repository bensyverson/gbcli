//! Which DMG shade each color takes (Ben's second ruling, 2026-11-07): the
//! first rule that applies, decided once for the whole project, so one
//! color is one shade in every image. 1. Declared: the `dmg` line of
//! `assets/palettes.txt` lists the colors of shades 0-3. 2. Known: every color belongs to one
//! known palette. 3. Otherwise lightness is stretched over the project's
//! colors, then each takes the nearest shade.

use crate::common;
use crate::pngs;

use std::fs;

use gbcli_build::build;
use gbcli_protocol::{BuildMade, ImageLimit, KnownPalette, Severity, ShadeChoice, ShadeRule};

use common::{rgbds, starter};
use pngs::*;

/// The DMG-01's pea greens, shade 0 first.
const PEA: [Rgba; 4] = [
    [0x9B, 0xBC, 0x0F, 0xFF],
    [0x8B, 0xAC, 0x0F, 0xFF],
    [0x30, 0x62, 0x30, 0xFF],
    [0x0F, 0x38, 0x0F, 0xFF],
];

fn shades(made: &BuildMade) -> ShadeChoice {
    made.images
        .as_ref()
        .and_then(|i| i.shades)
        .expect("the images result names its shade rule")
}

/// Each pixel's color index along row 0 of a one-tile background's data.
fn row0(temp: &common::Temp, name: &str) -> Vec<u8> {
    let tiles = fs::read(temp.path(&format!("demo/build/images/bg/{name}.2bpp"))).unwrap();
    (0..8).map(|x| index_at(&tiles, 0, x)).collect()
}

#[test]
fn pea_green_art_takes_all_four_shades_as_a_known_palette() {
    let (temp, options) = starter("palette-pea");
    write_png(&temp.path("demo/assets/bg/field.png"), 8, 8, |x, _| {
        PEA[(x / 2) as usize]
    });
    let made = build(&rgbds(), &options).unwrap();
    assert_eq!(row0(&temp, "field"), [0, 0, 1, 1, 2, 2, 3, 3]);
    assert_eq!(
        shades(&made),
        ShadeChoice {
            rule: ShadeRule::Known,
            palette: Some(KnownPalette::PeaGreens),
        }
    );
    assert!(problems(&made, ImageLimit::SharedShade).is_empty());
}

/// One color, one shade, in every image: the two dark greens alone in one
/// image are shades 2 and 3, as in the image that has all four.
#[test]
fn two_dark_pea_greens_alone_take_the_shades_they_have_beside_the_rest() {
    let (temp, options) = starter("palette-consistent");
    write_png(&temp.path("demo/assets/bg/field.png"), 8, 8, |x, _| {
        PEA[(x / 2) as usize]
    });
    write_png(&temp.path("demo/assets/bg/cave.png"), 8, 8, |x, _| {
        PEA[2 + (x / 4) as usize]
    });
    let made = build(&rgbds(), &options).unwrap();
    assert_eq!(row0(&temp, "cave"), [2, 2, 2, 2, 3, 3, 3, 3]);
    assert_eq!(row0(&temp, "field"), [0, 0, 1, 1, 2, 2, 3, 3]);
    assert_eq!(shades(&made).rule, ShadeRule::Known);
}

/// A sheet takes the project's shades too: its OBP maps the same greens to
/// the same shades as the background.
#[test]
fn a_sheet_shares_the_projects_shades() {
    let (temp, options) = starter("palette-sheet");
    write_png(&temp.path("demo/assets/bg/field.png"), 8, 8, |x, _| {
        PEA[(x / 2) as usize]
    });
    write_png(
        &temp.path("demo/assets/sprites/bug.png"),
        8,
        16,
        |x, _| match x {
            0 => PEA[2],
            1 => PEA[3],
            _ => CLEAR,
        },
    );
    build(&rgbds(), &options).unwrap();
    let data = fs::read(temp.path("demo/build/images/sprites/bug.2bpp")).unwrap();
    let inc = fs::read_to_string(temp.path("demo/build/images/images.inc")).unwrap();
    let obp = binary_constant(&inc, "BUG_OBP");
    let shade = |x| (obp >> (2 * index_at(&data, 0, x))) & 3;
    assert_eq!((shade(0), shade(1)), (2, 3), "{obp:08b}");
}

#[test]
fn a_declared_palette_wins_over_a_known_one() {
    let (temp, options) = starter("palette-declared");
    // Declared darkest first, so the grays take the shades backward.
    fs::write(
        temp.path("demo/assets/palettes.txt"),
        "# shade 0 first\n\ndmg: #000000 #555555 #aaaaaa #ffffff  # dark first\n",
    )
    .unwrap();
    write_png(&temp.path("demo/assets/bg/grays.png"), 8, 8, |x, _| {
        [WHITE, LIGHT, DARK, BLACK][(x / 2) as usize]
    });
    let made = build(&rgbds(), &options).unwrap();
    assert_eq!(row0(&temp, "grays"), [3, 3, 2, 2, 1, 1, 0, 0]);
    assert_eq!(
        shades(&made),
        ShadeChoice {
            rule: ShadeRule::Declared,
            palette: None,
        }
    );
}

/// Each color takes the nearest declared color's shade.
#[test]
fn colors_take_the_shade_of_the_nearest_declared_color() {
    let (temp, options) = starter("palette-declared-near");
    fs::write(
        temp.path("demo/assets/palettes.txt"),
        "dmg: #e0f0e0 #a0b0a0 #506050 #102010\n",
    )
    .unwrap();
    write_png(&temp.path("demo/assets/bg/moss.png"), 8, 8, |x, _| {
        [WHITE, LIGHT, DARK, BLACK][(x / 2) as usize]
    });
    build(&rgbds(), &options).unwrap();
    assert_eq!(row0(&temp, "moss"), [0, 0, 1, 1, 2, 2, 3, 3]);
}

/// A dim palette no rule knows: by plain lightness its colors would crowd
/// into shades 1-3, so lightness is stretched over the project's colors
/// first, the lightest becoming shade 0 and the darkest shade 3.
#[test]
fn an_unknown_dim_palette_is_stretched() {
    let (temp, options) = starter("palette-stretched");
    let dim: [Rgba; 4] = [
        [0x80, 0x80, 0x80, 0xFF],
        [0x66, 0x66, 0x66, 0xFF],
        [0x4D, 0x4D, 0x4D, 0xFF],
        [0x33, 0x33, 0x33, 0xFF],
    ];
    write_png(&temp.path("demo/assets/bg/dusk.png"), 8, 8, |x, _| {
        dim[(x / 2) as usize]
    });
    let made = build(&rgbds(), &options).unwrap();
    assert_eq!(row0(&temp, "dusk"), [0, 0, 1, 1, 2, 2, 3, 3]);
    assert_eq!(
        shades(&made),
        ShadeChoice {
            rule: ShadeRule::Stretched,
            palette: None,
        }
    );
    // Each moved one shade at most from its nearest: no warning.
    let far = problems(&made, ImageLimit::ShadeStretched);
    assert!(far.is_empty(), "{far:#?}");
}

/// Black and #101010 alone: the stretch draws #101010 white, two shades or
/// more from its nearest (black), so the build warns (Ben's ruling,
/// 2026-11-07).
#[test]
fn a_color_the_stretch_moves_far_is_drawn_as_given_with_a_warning() {
    let (temp, options) = starter("palette-stretched-far");
    let near_black: Rgba = [0x10, 0x10, 0x10, 0xFF];
    write_png(&temp.path("demo/assets/bg/night.png"), 16, 8, |x, _| {
        if x < 12 { BLACK } else { near_black }
    });
    let made = build(&rgbds(), &options).unwrap();
    let tiles = fs::read(temp.path("demo/build/images/bg/night.2bpp")).unwrap();
    let map = fs::read(temp.path("demo/build/images/bg/night.tilemap")).unwrap();
    let second = &tiles[usize::from(map[1]) * 16..][..16];
    let row: Vec<u8> = (0..8).map(|x| index_at(second, 0, x)).collect();
    assert_eq!(row, [3, 3, 3, 3, 0, 0, 0, 0]);
    assert_eq!(shades(&made).rule, ShadeRule::Stretched);

    let p = problem(&made, ImageLimit::ShadeStretched);
    assert_eq!(p.image.as_deref(), Some("demo/assets/bg/night.png"));
    assert_eq!(p.severity, Severity::Warning);
    assert_eq!(p.tiles, [gbcli_protocol::TileAt { x: 1, y: 0 }]);
    for part in ["#101010", "nearest shade 3 (black), drawn as 0 (white)"] {
        assert!(p.message.contains(part), "{part:?} in {}", p.message);
    }
    assert!(
        p.remedy
            .contains("declare the dmg line of assets/palettes.txt to pick the shades yourself"),
        "{}",
        p.remedy
    );
}

/// One lightness in the whole project has nothing to stretch: it keeps
/// its plain nearest shade.
#[test]
fn a_project_of_one_lightness_keeps_the_nearest_shade() {
    let (temp, options) = starter("palette-one");
    write_png(&temp.path("demo/assets/bg/fog.png"), 8, 8, |_, _| {
        [0x60, 0x60, 0x60, 0xFF]
    });
    let made = build(&rgbds(), &options).unwrap();
    assert_eq!(row0(&temp, "fog"), [2; 8]);
    assert_eq!(shades(&made).rule, ShadeRule::Nearest);
}

#[test]
fn a_malformed_palette_file_is_an_error_naming_its_line() {
    let (temp, options) = starter("palette-bad");
    fs::write(
        temp.path("demo/assets/palettes.txt"),
        "# lightest first\n\ndmg: #ffffff #aaaa #555555 #000000\n",
    )
    .unwrap();
    write_png(&temp.path("demo/assets/bg/sky.png"), 8, 8, |_, _| WHITE);
    let made = refused(&options);
    let p = problem(&made, ImageLimit::PaletteFile);
    assert_eq!(p.image.as_deref(), Some("demo/assets/palettes.txt"));
    assert_eq!(p.severity, Severity::Error);
    assert_eq!(p.line, Some(3));
    assert!(p.message.contains("#aaaa"), "{}", p.message);
    assert!(p.message.contains("line 3"), "{}", p.message);
    assert!(p.remedy.contains("#rrggbb"), "{}", p.remedy);
}

#[test]
fn a_palette_file_without_four_colors_is_an_error() {
    let (temp, options) = starter("palette-three");
    fs::write(
        temp.path("demo/assets/palettes.txt"),
        "dmg: #ffffff #aaaaaa #000000\n",
    )
    .unwrap();
    write_png(&temp.path("demo/assets/bg/sky.png"), 8, 8, |_, _| WHITE);
    let made = refused(&options);
    let p = problem(&made, ImageLimit::PaletteFile);
    assert_eq!((p.count, p.max), (3, 4));
    assert!(p.message.contains("3 colors"), "{}", p.message);
}
