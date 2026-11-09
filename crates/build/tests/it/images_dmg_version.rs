//! Color art's DMG version (the GBC images plan, questions 2, 3 and 9;
//! leaf tzEhz3): derived from the color image, each color taking the shade
//! the project's rule gives it (`shades::choose`), unless a `name.dmg.png`
//! beside it overrides it. Under --gbc, two colors in one tile or object
//! that fall to one shade warn, naming it, and across tiles they don't; a
//! plain build warns for the whole image (leaf Vho2Gc). A plain build
//! derives it exactly as `--gbc` does (Ben's ruling, 2026-11-08,
//! superseding question 9; leaf GQFHVi), so one project builds both ways.

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

const BOTH: [BuildTarget; 2] = [BuildTarget::Dmg, BuildTarget::Gbc];

/// A project for `target`, named after it.
fn for_target(name: &str, target: BuildTarget) -> (Temp, BuildOptions) {
    let (temp, mut options) = starter(&format!("{name}-{}", target.name()));
    options.target = target;
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

/// Color art whose colors share shades only across tiles or objects: a
/// plain build warns `shared_shade` image-wide, `rows` warnings (one a
/// shared shade) and nothing else; --gbc, which keeps the colors apart,
/// says nothing (Ben's ruling, 2026-11-08, leaf Vho2Gc).
fn shares_only_when_plain(target: BuildTarget, images: &gbcli_protocol::ImagesMade, rows: usize) {
    let (shared, rest): (Vec<_>, Vec<_>) = images
        .problems
        .iter()
        .partition(|p| p.limit == ImageLimit::SharedShade);
    assert!(rest.is_empty(), "{target:?}: {rest:#?}");
    let want = if target == BuildTarget::Dmg { rows } else { 0 };
    assert_eq!(shared.len(), want, "{target:?}: {shared:#?}");
    assert!(shared.iter().all(|p| p.severity == Severity::Warning));
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

/// The racer's and the shooter's first art (the examples' DX review, item
/// 2): a background in six colors, which a plain build refused as
/// `dmg_palette` while `--gbc` derived its DMG version.
#[test]
fn a_color_background_over_four_colors_builds_clean_both_ways() {
    for target in BOTH {
        let (temp, options) = for_target("dmg-derived-both", target);
        let tiles = [WHITE, YELLOW, GREEN, RED, BLUE, BLACK];
        write_png(&temp.path("demo/assets/bg/town.png"), 48, 8, |x, _| {
            tiles[(x / 8) as usize]
        });
        let made = build(&rgbds(), &options).unwrap();
        let images = made.images.unwrap();
        // White and yellow, blue and black share shades: a plain build warns
        // image-wide, one row a shade; --gbc keeps them apart.
        shares_only_when_plain(target, &images, 2);
        assert_eq!(
            cell_shades(&temp, "town"),
            [0, 0, 1, 2, 3, 3].map(Some),
            "{target:?}"
        );
        // Only --gbc makes a GBC version.
        let gbc = images.images[0].gbc.is_some();
        assert_eq!(gbc, target == BuildTarget::Gbc, "{target:?}");
    }
}

/// A color sheet in four colors that fall to three shades: as DMG art, a
/// plain build refused it, over an object palette's 3 colors.
#[test]
fn a_color_sheet_whose_colors_fall_to_three_shades_builds_both_ways() {
    for target in BOTH {
        let (temp, options) = for_target("dmg-derived-sheet", target);
        // White and yellow are both shade 0, in different objects.
        write_png(
            &temp.path("demo/assets/sprites/bat.png"),
            16,
            16,
            |x, _| match x {
                0 => WHITE,
                1 | 9 => GREEN,
                2 | 10 => BLACK,
                8 => YELLOW,
                _ => CLEAR,
            },
        );
        let made = build(&rgbds(), &options).unwrap();
        let images = made.images.unwrap();
        shares_only_when_plain(target, &images, 1);
        let inc = String::from_utf8(read(&temp, "images.inc")).unwrap();
        // Index 1 white, 2 light gray, 3 black.
        assert_eq!(
            binary_constant(&inc, "BAT_OBP"),
            0b11_01_00_00,
            "{target:?}"
        );
    }
}

/// A DMG-only project (a plain build, no palettes.txt) drawn in grays and
/// one near-black (review-img14 of 372d772): the near-black falls to black
/// in another tile, and a plain build still warns `shared_shade` for the
/// image, naming both colors and the hand-drawn DMG version, as before
/// 372d772 (Ben's ruling, 2026-11-08). Under --gbc the color version keeps
/// them apart, so only a share within one tile would warn.
#[test]
fn a_plain_builds_near_black_across_tiles_still_warns() {
    const NEAR_BLACK: Rgba = [0x10, 0x10, 0x10, 0xFF];
    for target in BOTH {
        let (temp, options) = for_target("dmg-near-black", target);
        write_png(&temp.path("demo/assets/bg/cave.png"), 24, 8, |x, _| {
            [WHITE, BLACK, NEAR_BLACK][(x / 8) as usize]
        });
        let made = build(&rgbds(), &options).unwrap();
        let shared = problems(&made, ImageLimit::SharedShade);
        if target == BuildTarget::Gbc {
            assert!(shared.is_empty(), "{shared:#?}");
            continue;
        }
        let [p] = &shared[..] else {
            panic!("one shared_shade: {:#?}", made.images.unwrap().problems)
        };
        assert_eq!(p.severity, Severity::Warning);
        assert_eq!(p.image.as_deref(), Some("demo/assets/bg/cave.png"));
        // The less used of the two is listed: the near-black's tile.
        assert_eq!(p.tiles, [TileAt { x: 2, y: 0 }]);
        for part in ["#000000", "#101010", "shade 3"] {
            assert!(p.message.contains(part), "{part:?} in {}", p.message);
        }
        assert!(
            p.remedy.contains("demo/assets/bg/cave.dmg.png"),
            "{}",
            p.remedy
        );
    }
}

#[test]
fn an_override_is_used_by_a_plain_build_too() {
    for target in BOTH {
        let (temp, options) = for_target("dmg-override-both", target);
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
        assert_eq!(
            cell_shades(&temp, "town"),
            [Some(0), Some(3), Some(2)],
            "{target:?}"
        );
        let images = made.images.unwrap();
        assert!(
            images.problems.is_empty(),
            "{target:?}: {:#?}",
            images.problems
        );
        assert_eq!(images.images.len(), 1, "{target:?}");
    }
}

#[test]
fn an_override_of_another_size_is_dmg_override_size_in_a_plain_build_too() {
    let (temp, options) = starter("dmg-override-size-plain");
    write_png(&temp.path("demo/assets/bg/town.png"), 16, 8, |x, _| {
        [RED, GREEN][(x / 8) as usize]
    });
    write_png(&temp.path("demo/assets/bg/town.dmg.png"), 8, 8, |_, _| {
        WHITE
    });
    let made = refused(&options);
    let p = problem(&made, ImageLimit::DmgOverrideSize);
    assert_eq!(p.image.as_deref(), Some("demo/assets/bg/town.dmg.png"));
}

#[test]
fn an_override_beside_indexed_art_is_an_error() {
    for target in BOTH {
        let (temp, options) = for_target("dmg-override-indexed", target);
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
            assert!(message.contains(part), "{target:?}: {part:?} in {message}");
        }
        assert!(remedy.contains("remove"), "{target:?}: {remedy}");
    }
}
