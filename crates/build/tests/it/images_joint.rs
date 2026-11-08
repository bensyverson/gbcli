//! The joint conversion under `build --gbc` (plan question 1, leaf 0racny):
//! color art's tiles are deduplicated on the pair (DMG tile, GBC tile), so
//! one tilemap or frame list serves both machines while each has its own
//! tile data. A background never flips; a sheet keeps its flip dedup, with
//! both versions flipped alike. Every image's `tile_map` is filled on every
//! build (Ben's ruling under plan question 3), with a slot only when the
//! image has a GBC version.

use crate::common;
use crate::pngs;

use std::fs;

use gbcli_build::{BuildOptions, build};
use gbcli_protocol::{
    BuildTarget, Flip, GbcPalette, ImageArt, ImageAsset, ImageLimit, ImagesMade, PaletteSlot,
    Rgb555, Severity, TileAt, TileEntry,
};

use common::{Temp, rgbds, starter};
use pngs::*;

const RED: Rgba = [0xFF, 0x00, 0x00, 0xFF];
const GREEN: Rgba = [0x00, 0xFF, 0x00, 0xFF];
const BLUE: Rgba = [0x00, 0x00, 0xFF, 0xFF];
const YELLOW: Rgba = [0xFF, 0xFF, 0x00, 0xFF];
const CYAN: Rgba = [0x00, 0xFF, 0xFF, 0xFF];
const MAGENTA: Rgba = [0xFF, 0x00, 0xFF, 0xFF];
const ORANGE: Rgba = [0xFF, 0x80, 0x00, 0xFF];
const PURPLE: Rgba = [0x80, 0x00, 0xFF, 0xFF];
const GRAYS: [Rgba; 4] = [WHITE, LIGHT, DARK, BLACK];

fn gbc(name: &str) -> (Temp, BuildOptions) {
    let (temp, mut options) = starter(name);
    options.target = BuildTarget::Gbc;
    (temp, options)
}

fn read(temp: &Temp, file: &str) -> Vec<u8> {
    let path = temp.path(&format!("demo/build/images/{file}"));
    fs::read(&path).unwrap_or_else(|e| panic!("{}: {e}", path.display()))
}

/// The images a build made, which read back as themselves: the wire's
/// checks accept every shape the build makes.
fn built(options: &BuildOptions) -> ImagesMade {
    let images = build(&rgbds(), options).unwrap().images.expect("images");
    assert!(images.first_error().is_none(), "{:#?}", images.problems);
    let json = serde_json::to_string(&images).unwrap();
    let read: ImagesMade = serde_json::from_str(&json).unwrap_or_else(|e| panic!("{e}: {json}"));
    assert_eq!(read, images);
    images
}

fn image<'a>(images: &'a ImagesMade, source: &str) -> &'a ImageAsset {
    images
        .images
        .iter()
        .find(|i| i.source.ends_with(source))
        .unwrap_or_else(|| panic!("no {source} in {images:#?}"))
}

fn rgb555(p: Rgba) -> Rgb555 {
    Rgb555::from_rgb888(p[0], p[1], p[2])
}

fn gray_shade(p: Rgba) -> u8 {
    GRAYS.iter().position(|&g| g == p).expect("a gray") as u8
}

fn slot(e: &TileEntry) -> usize {
    usize::from(e.palette.expect("a GBC slot").get())
}

/// A 32x16 color background of 4x2 tiles: eight colors over two packed
/// palettes; tile (3, 1) repeats tile (0, 0) in both versions, and tile
/// (1, 1) repeats tile (1, 0)'s colors but not its grays.
fn color_town(x: u32, y: u32) -> Rgba {
    let (tx, ty) = (x / 8, y / 8);
    let t = match (tx, ty) {
        (3, 1) => 0,
        (1, 1) => 1,
        _ => ty * 4 + tx,
    };
    let colors = [[RED, GREEN, BLUE, BLACK], [YELLOW, CYAN, MAGENTA, WHITE]][(t % 2) as usize];
    colors[((x % 8 * (t / 2 + 1) + y % 8) % 4) as usize]
}

fn gray_town(x: u32, y: u32) -> Rgba {
    let (tx, ty) = (x / 8, y / 8);
    let t = match (tx, ty) {
        (3, 1) => 0,
        _ => ty * 4 + tx,
    };
    GRAYS[((x % 8 / 2 + y % 8 + t % 4 + t / 4) % 4) as usize]
}

/// Rebuilding each version from the generated data: the DMG tiles through
/// the tilemap and BGP, the GBC tiles through the tile map's slots and the
/// image's palettes, reproduce the override and the color image.
#[test]
fn a_background_rebuilt_from_both_versions_data_is_both_pngs() {
    let (temp, options) = gbc("joint-bg-rebuild");
    let (w, h) = (32, 16);
    write_png(&temp.path("demo/assets/bg/town.png"), w, h, color_town);
    write_png(&temp.path("demo/assets/bg/town.dmg.png"), w, h, gray_town);
    let images = built(&options);
    let town = image(&images, "bg/town.png");
    assert_eq!(town.tiles, 7, "one repeat in both versions, of 8 cells");
    let gbc = town.gbc.as_ref().expect("a GBC version");
    assert_eq!(gbc.art, ImageArt::Color);
    assert_eq!(gbc.palettes.len(), 2);

    let dmg_tiles = read(&temp, "bg/town.2bpp");
    let gbc_tiles = read(&temp, "bg/town.gbc.2bpp");
    let map = read(&temp, "bg/town.tilemap");
    assert_eq!(dmg_tiles.len(), 7 * 16);
    assert_eq!(gbc_tiles.len(), 7 * 16, "the GBC data in the same order");
    assert_eq!(town.tile_map.len(), 8);
    let inc = String::from_utf8(read(&temp, "images.inc")).unwrap();
    let bgp = binary_constant(&inc, "TOWN_BGP");
    for (i, entry) in town.tile_map.iter().enumerate() {
        let entry = entry.expect("a background's every cell has a tile");
        assert_eq!(entry.flip, Flip::None, "a background never flips");
        assert_eq!(u32::from(map[i]), entry.index, "one map for both machines");
        let (tx, ty) = (i as u32 % 4, i as u32 / 4);
        let at = usize::from(map[i]) * 16;
        for y in 0..8 {
            for x in 0..8 {
                let (px, py) = (tx * 8 + x, ty * 8 + y);
                let index = index_at(&dmg_tiles[at..], y as usize, x);
                let shade = bgp >> (2 * index) & 3;
                assert_eq!(shade, gray_shade(gray_town(px, py)), "DMG at ({px}, {py})");
                let index = index_at(&gbc_tiles[at..], y as usize, x);
                let color = gbc.palettes[slot(&entry)].0[usize::from(index)];
                assert_eq!(color, rgb555(color_town(px, py)), "GBC at ({px}, {py})");
            }
        }
    }
}

/// Two 16x16 frames, the second the first mirrored left to right in both
/// versions; each unit three colors (two in one gray) on transparent.
fn color_hero(x: u32, y: u32) -> Rgba {
    let x = if x >= 16 { 31 - x } else { x };
    match (x, y) {
        (0..=2, _) => RED,
        (3, 0..=7) => GREEN,
        (3, _) => BLUE,
        (8..=10, 0..=3) => ORANGE,
        (11, _) => PURPLE,
        (12, 2..=5) => YELLOW,
        _ => CLEAR,
    }
}

fn gray_hero(x: u32, y: u32) -> Rgba {
    let x = if x >= 16 { 31 - x } else { x };
    match color_hero(x, y) {
        CLEAR => CLEAR,
        RED | ORANGE => BLACK,
        GREEN | BLUE | PURPLE => DARK,
        _ => LIGHT,
    }
}

/// A sheet's units are deduplicated with flips on both versions at once
/// (the mirrored frame costs no tiles), rgbgfx accepts its transparent
/// pixels under the OBJ palettes' `-c` spec, and its units rebuilt from
/// both tile data, their flips and slots, are both PNGs.
#[test]
fn a_sheet_rebuilt_from_both_versions_data_keeps_its_flips() {
    let (temp, options) = gbc("joint-sheet-rebuild");
    let (w, h) = (32, 16);
    write_png(
        &temp.path("demo/assets/sprites/hero.16x16.png"),
        w,
        h,
        color_hero,
    );
    write_png(
        &temp.path("demo/assets/sprites/hero.16x16.dmg.png"),
        w,
        h,
        gray_hero,
    );
    let images = built(&options);
    let hero = image(&images, "hero.16x16.png");
    assert_eq!(hero.tiles, 4, "two 8x16 units; the mirror reuses them");
    assert_eq!(
        hero.tile_map
            .iter()
            .map(|e| e.unwrap().flip)
            .collect::<Vec<_>>(),
        [Flip::None, Flip::None, Flip::X, Flip::X]
    );
    let gbc = hero.gbc.as_ref().expect("a GBC version");
    assert_eq!(gbc.art, ImageArt::Color);
    assert!(gbc.palettes.is_empty(), "a sheet's are the shared ones");
    let obj = images
        .gbc
        .as_ref()
        .expect("GBC images")
        .obj_palettes
        .clone();
    assert!(!obj.is_empty());

    let dmg_tiles = read(&temp, "sprites/hero.2bpp");
    let gbc_tiles = read(&temp, "sprites/hero.gbc.2bpp");
    assert_eq!((dmg_tiles.len(), gbc_tiles.len()), (4 * 16, 4 * 16));
    let inc = String::from_utf8(read(&temp, "images.inc")).unwrap();
    let obp = binary_constant(&inc, "HERO_OBP");
    for (i, entry) in hero.tile_map.iter().enumerate() {
        let entry = entry.unwrap();
        let (ux, uy) = (i as u32 % 4 * 8, i as u32 / 4 * 16);
        for y in 0..16 {
            for x in 0..8 {
                let sx = if entry.flip.x() { 7 - x } else { x };
                let sy = if entry.flip.y() { 15 - y } else { y };
                let tile = (entry.index + sy / 8) as usize * 16;
                let (px, py) = (ux + x, uy + y);
                let index = index_at(&dmg_tiles[tile..], (sy % 8) as usize, sx);
                match gray_hero(px, py) {
                    CLEAR => assert_eq!(index, 0, "DMG at ({px}, {py})"),
                    g => assert_eq!(obp >> (2 * index) & 3, gray_shade(g), "DMG ({px}, {py})"),
                }
                let index = index_at(&gbc_tiles[tile..], (sy % 8) as usize, sx);
                match color_hero(px, py) {
                    CLEAR => assert_eq!(index, 0, "GBC at ({px}, {py})"),
                    c => assert_eq!(
                        obj[slot(&entry)].0[usize::from(index)],
                        rgb555(c),
                        "GBC at ({px}, {py})"
                    ),
                }
            }
        }
    }
}

/// Red and blue in the same gray: one tile on the DMG, two on the GBC, so
/// the joint data keeps both; a cell alike in both versions is one tile.
#[test]
fn a_tile_alike_in_gray_but_not_in_color_is_kept_twice() {
    let (temp, options) = gbc("joint-twice");
    write_png(&temp.path("demo/assets/bg/town.png"), 24, 8, |x, _| {
        [RED, BLUE, RED][(x / 8) as usize]
    });
    write_png(&temp.path("demo/assets/bg/town.dmg.png"), 24, 8, |_, _| {
        DARK
    });
    let images = built(&options);
    let town = image(&images, "bg/town.png");
    assert_eq!(town.tiles, 2);
    assert_eq!(read(&temp, "bg/town.tilemap"), [0, 1, 0]);
    let indices: Vec<u32> = town.tile_map.iter().map(|e| e.unwrap().index).collect();
    assert_eq!(indices, [0, 1, 0]);
    // The DMG data holds the dark tile twice: it is the GBC's order.
    let dmg = read(&temp, "bg/town.2bpp");
    assert_eq!(dmg.len(), 32);
    assert_eq!(dmg[..16], dmg[16..]);
}

/// The same for a sheet's units: alike in gray, different in color.
#[test]
fn a_unit_alike_in_gray_but_not_in_color_is_kept_twice() {
    let (temp, options) = gbc("joint-twice-sheet");
    let color = |x: u32, _| match x % 8 {
        0..=3 => [RED, BLUE][(x / 8) as usize],
        _ => CLEAR,
    };
    write_png(&temp.path("demo/assets/sprites/bat.png"), 16, 16, color);
    write_png(
        &temp.path("demo/assets/sprites/bat.dmg.png"),
        16,
        16,
        |x, y| if color(x, y) == CLEAR { CLEAR } else { DARK },
    );
    let images = built(&options);
    assert_eq!(image(&images, "bat.png").tiles, 4, "two units");
}

/// Without --gbc, and for color art's DMG data alike, every image has a
/// tile map: its tiles and flips, with no slot (no GBC version).
#[test]
fn a_dmg_build_fills_the_tile_map_without_slots() {
    let (temp, options) = starter("joint-dmg-map");
    write_png(&temp.path("demo/assets/bg/town.png"), 24, 8, |x, _| {
        [WHITE, BLACK, WHITE][(x / 8) as usize]
    });
    write_png(
        &temp.path("demo/assets/sprites/hero.16x16.png"),
        32,
        16,
        |x, y| {
            let x = if x >= 16 { 31 - x } else { x };
            match (x, y) {
                (1, _) => BLACK,
                (8..=10, 0..=2) => DARK,
                _ => CLEAR,
            }
        },
    );
    let images = built(&options);
    let at = |index, flip| {
        Some(TileEntry {
            index,
            flip,
            palette: None,
        })
    };
    let town = image(&images, "bg/town.png");
    assert_eq!(town.gbc, None);
    assert_eq!(
        town.tile_map,
        [at(0, Flip::None), at(1, Flip::None), at(0, Flip::None)]
    );
    let hero = image(&images, "hero.16x16.png");
    assert_eq!(
        hero.tile_map,
        [
            at(0, Flip::None),
            at(2, Flip::None),
            at(2, Flip::X),
            at(0, Flip::X)
        ]
    );
    assert_eq!(read(&temp, "bg/town.tilemap"), [0, 1, 0]);
}

/// Indexed sheets' slots come first; a color sheet's packed palettes
/// follow them in the shared OBJ palettes, and its units name those.
#[test]
fn a_color_sheets_slots_follow_the_assigned_ones() {
    let (temp, options) = gbc("joint-after-assigned");
    write_png(&temp.path("demo/assets/sprites/coin.png"), 8, 16, |x, _| {
        if x < 4 { BLACK } else { CLEAR }
    });
    write_png(&temp.path("demo/assets/sprites/bat.png"), 8, 16, |x, _| {
        if x < 4 { RED } else { CLEAR }
    });
    fs::write(
        temp.path("demo/assets/palettes.txt"),
        "ember: #ffe #f80 #a20 #200\nsprites/coin.png = ember\n",
    )
    .unwrap();
    let images = built(&options);
    let obj = &images.gbc.as_ref().unwrap().obj_palettes;
    assert_eq!(obj.len(), 2, "{obj:?}");
    let bat = image(&images, "bat.png");
    assert_eq!(bat.tile_map[0].unwrap().palette, PaletteSlot::new(1));
    assert_eq!(obj[1].0[1], rgb555(RED));
    assert_eq!(
        image(&images, "coin.png").tile_map[0].unwrap().palette,
        PaletteSlot::new(0)
    );
}

/// A color background's palettes come from gbcli's packing in slot order:
/// a solid tile whose color two palettes hold still draws its color.
#[test]
fn a_tile_two_palettes_could_hold_draws_its_color() {
    let (temp, options) = gbc("joint-two-fit");
    let tiles = [
        [RED, GREEN, BLUE, YELLOW],
        [RED, CYAN, MAGENTA, PURPLE],
        [RED; 4],
    ];
    let color = |x: u32, y: u32| tiles[(x / 8) as usize][((x + y) % 4) as usize];
    write_png(&temp.path("demo/assets/bg/town.png"), 24, 8, color);
    let images = built(&options);
    let town = image(&images, "bg/town.png");
    let gbc = town.gbc.as_ref().unwrap();
    let data = read(&temp, "bg/town.gbc.2bpp");
    for (i, e) in town.tile_map.iter().enumerate() {
        let e = e.unwrap();
        for y in 0..8 {
            for x in 0..8 {
                let index = index_at(&data[e.index as usize * 16..], y as usize, x);
                let drawn = gbc.palettes[slot(&e)].0[usize::from(index)];
                assert_eq!(drawn, rgb555(color(i as u32 * 8 + x, y)));
            }
        }
    }
}

/// With an override, the DMG checks read the override; the color image's
/// own alpha is checked before its palettes are packed. A color background
/// must be opaque.
#[test]
fn a_color_background_with_transparency_is_alpha_even_with_an_override() {
    let (temp, options) = gbc("joint-bg-alpha");
    write_png(&temp.path("demo/assets/bg/town.png"), 16, 8, |x, _| {
        if x == 9 { CLEAR } else { RED }
    });
    write_png(&temp.path("demo/assets/bg/town.dmg.png"), 16, 8, |_, _| {
        WHITE
    });
    let made = refused(&options);
    assert_eq!(errors(&made), [ImageLimit::Alpha]);
    let p = problem(&made, ImageLimit::Alpha);
    assert_eq!(p.image.as_deref(), Some("demo/assets/bg/town.png"));
    assert_eq!(p.tiles, [TileAt { x: 1, y: 0 }]);
}

/// A half-transparent pixel in a color sheet is `alpha`, before the packer
/// (which would count it opaque) runs.
#[test]
fn a_half_transparent_color_pixel_is_alpha_before_packing() {
    for over in [false, true] {
        let (temp, options) = gbc(&format!("joint-half-alpha-{over}"));
        let color = |x: u32, y: u32| match (x, y) {
            (0, 0) => [0xFF, 0, 0, 0x80],
            (0..=3, _) => RED,
            _ => CLEAR,
        };
        write_png(&temp.path("demo/assets/sprites/bat.png"), 8, 16, color);
        if over {
            write_png(
                &temp.path("demo/assets/sprites/bat.dmg.png"),
                8,
                16,
                |x, y| if color(x, y) == CLEAR { CLEAR } else { BLACK },
            );
        }
        let made = refused(&options);
        assert_eq!(errors(&made), [ImageLimit::Alpha], "override: {over}");
    }
}

/// A sheet's override must be transparent exactly where its color image
/// is: the frame lists, which say which objects exist, serve both.
#[test]
fn a_sheet_override_transparent_elsewhere_than_its_color_image_is_alpha() {
    let (temp, options) = gbc("joint-sheet-alpha");
    write_png(&temp.path("demo/assets/sprites/bat.png"), 16, 16, |x, _| {
        if x < 4 { RED } else { CLEAR }
    });
    write_png(
        &temp.path("demo/assets/sprites/bat.dmg.png"),
        16,
        16,
        |x, y| match (x, y) {
            (0..=3, _) => BLACK,
            (9, 12) => BLACK,
            _ => CLEAR,
        },
    );
    let made = refused(&options);
    assert_eq!(errors(&made), [ImageLimit::Alpha]);
    let p = problem(&made, ImageLimit::Alpha);
    assert_eq!(p.severity, Severity::Error);
    assert_eq!(p.image.as_deref(), Some("demo/assets/sprites/bat.png"));
    assert_eq!(p.tiles, [TileAt { x: 1, y: 1 }]);
    assert!(p.message.contains("bat.dmg.png"), "{}", p.message);
}

/// The packed palettes are what the GBC version's slots index; a fresh
/// palette per background, BG slot 0 up.
#[test]
fn a_color_backgrounds_slots_index_its_own_palettes() {
    let (temp, options) = gbc("joint-own-palettes");
    write_png(&temp.path("demo/assets/bg/a.png"), 8, 8, |_, _| RED);
    write_png(&temp.path("demo/assets/bg/b.png"), 8, 8, |_, _| BLUE);
    let images = built(&options);
    for (name, c) in [("bg/a.png", RED), ("bg/b.png", BLUE)] {
        let a = image(&images, name);
        assert_eq!(a.tile_map[0].unwrap().palette, PaletteSlot::new(0));
        let GbcPalette(p) = a.gbc.as_ref().unwrap().palettes[0];
        assert_eq!(p[0], rgb555(c));
    }
}
