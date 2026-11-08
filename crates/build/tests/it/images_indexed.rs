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
fn hero_pixel(x: u32, y: u32) -> Rgba {
    match x / 16 {
        0 => arrow(x, y),
        1 => arrow(15 - (x - 16), y),
        _ => CLEAR,
    }
}

fn hero(temp: &Temp, name: &str) {
    write_png(
        &temp.path(&format!("demo/assets/sprites/{name}.16x16.png")),
        48,
        16,
        hero_pixel,
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
    assert_eq!((sky_dmg.gbc.as_ref(), sky_dmg.tile_map.len()), (None, 4));
    assert_eq!(dmg.gbc, None);
    let dmg_tiles = read("sky.2bpp");

    let made = images(build(&rgbds(), &gbc(&options)).unwrap());
    assert_eq!(read("sky.2bpp"), dmg_tiles);
    let mut written: Vec<String> = fs::read_dir(temp.path("demo/build/images/bg"))
        .unwrap()
        .map(|e| e.unwrap().file_name().to_string_lossy().into_owned())
        .collect();
    written.sort();
    // No GBC tile data of its own: its attribute map and GBC preview only.
    assert_eq!(
        written,
        [
            "sky.2bpp",
            "sky.attrmap",
            "sky.gbc.preview.png",
            "sky.preview.png",
            "sky.tilemap"
        ],
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
    // Drawn in shades 2 and 3, which OBP gives indices 1 and 2.
    assert_eq!(made.gbc.unwrap().obj_palettes, [by_obp(ember(), 2, 3, 0)]);
}

/// The named palette reordered as a sheet's OBP maps its indices: entry
/// `i` is the color of the shade OBP gives index `i` (index 0, never
/// drawn, takes shade 0's).
fn by_obp(GbcPalette(p): GbcPalette, one: usize, two: usize, three: usize) -> GbcPalette {
    GbcPalette([p[0], p[one], p[two], p[three]])
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
    assert_eq!(
        made.gbc.unwrap().obj_palettes,
        [by_obp(forest(), 2, 3, 0), by_obp(ember(), 2, 3, 0)]
    );
}

/// The shade of a gray the `dmg` line of `PALETTES` declares.
fn shade(c: Rgba) -> usize {
    [WHITE, LIGHT, DARK, BLACK]
        .iter()
        .position(|&g| g == c)
        .unwrap_or_else(|| panic!("{c:?} is no declared shade"))
}

/// A sheet as drawn: its file under `sprites/`, its label, its size and
/// frame size in pixels, its pixels and the palette assigned it.
struct Art {
    file: &'static str,
    label: &'static str,
    size: (u32, u32),
    frame: (u32, u32),
    pixel: fn(u32, u32) -> Rgba,
    named: GbcPalette,
}

/// Every opaque pixel of the sheet `art`, as the GBC draws it: each
/// frame's objects from `images.asm` (tile and flip), the slot from the
/// unit's tile map entry, the color index from the tile data, and the
/// color from that slot of the shared OBJ palettes. Each must be its
/// shade's color in the palette assigned it.
fn assert_drawn_in_shades(temp: &Temp, made: &ImagesMade, art: &Art) {
    let Art {
        file,
        label,
        size: (w, h),
        frame: (fw, fh),
        pixel,
        named,
    } = *art;
    let stem = file.split('.').next().unwrap();
    let data = fs::read(temp.path(&format!("demo/build/images/sprites/{stem}.2bpp"))).unwrap();
    let asm = fs::read_to_string(temp.path("demo/build/images/images.asm")).unwrap();
    let asset = image(made, file);
    let obj = &made.gbc.as_ref().expect("GBC images").obj_palettes;
    let columns = (w / 8) as usize;
    let mut covered = vec![false; (w * h) as usize];
    for frame in 0..(w / fw) * (h / fh) {
        let (fx, fy) = ((frame % (w / fw)) * fw, (frame / (w / fw)) * fh);
        let mut lines = asm
            .lines()
            .skip_while(|l| *l != format!("{label}Frame{frame}::"))
            .skip(1);
        let count: usize = lines
            .next()
            .unwrap()
            .trim()
            .strip_prefix("db ")
            .unwrap()
            .parse()
            .unwrap();
        for line in lines.take(count) {
            let bytes: Vec<u32> = line
                .trim()
                .strip_prefix("db ")
                .unwrap()
                .split(", ")
                .map(|b| match b.strip_prefix('$') {
                    Some(hex) => u32::from_str_radix(hex, 16).unwrap(),
                    None => b.parse().unwrap(),
                })
                .collect();
            let [oy, ox, tile, attr] = bytes[..] else {
                panic!("{line}")
            };
            let (ux, uy) = (fx + ox, fy + oy);
            let entry = asset.tile_map[(uy / 16) as usize * columns + (ux / 8) as usize]
                .expect("an object's unit has an entry");
            // The attributes: the flip, and the GBC palette in bits 0-2.
            let slot = entry.palette.expect("a slot").get();
            assert_eq!(
                (entry.index, entry.flip.oam_bits() | slot),
                (tile, attr as u8)
            );
            let GbcPalette(colors) = obj[usize::from(slot)];
            for py in 0..16 {
                for px in 0..8 {
                    let (x, y) = (ux + px, uy + py);
                    let sx = if entry.flip.x() { 7 - px } else { px };
                    let sy = if entry.flip.y() { 15 - py } else { py };
                    let t = (tile + sy / 8) as usize;
                    let index = index_at(&data[t * 16..], (sy % 8) as usize, sx);
                    let p = pixel(x, y);
                    if p[3] == 0 {
                        assert_eq!(index, 0, "{file} ({x},{y}) is transparent");
                        continue;
                    }
                    covered[(y * w + x) as usize] = true;
                    assert_eq!(
                        colors[usize::from(index)],
                        named.0[shade(p)],
                        "{file} ({x},{y}): shade {} draws index {index} of slot {:?}",
                        shade(p),
                        entry.palette
                    );
                }
            }
        }
    }
    for y in 0..h {
        for x in 0..w {
            assert!(
                pixel(x, y)[3] == 0 || covered[(y * w + x) as usize],
                "{file} ({x},{y}) is opaque but no object draws it"
            );
        }
    }
}

/// White and black on transparent, in two 8x16 frames, the second the
/// first mirrored.
fn ghost(x: u32, y: u32) -> Rgba {
    let x = if x < 8 { x } else { 15 - x };
    match (x, y) {
        (0, 0..=3) => WHITE,
        (1..=2, 2..=9) => BLACK,
        _ => CLEAR,
    }
}

/// White, light and black in 16x16 frames: a frame and its upside-down
/// copy.
fn gem(x: u32, y: u32) -> Rgba {
    let (x, y) = (x % 16, if x < 16 { y } else { 15 - y });
    match (x, y) {
        (0..=3, 0..=3) => WHITE,
        (4..=6, 5..=12) => LIGHT,
        (9, _) => BLACK,
        _ => CLEAR,
    }
}

/// Ben's ruling (plan question 4): on the GBC, which has no OBP, a sheet's
/// color index picks its palette entry directly, so the slot holds the
/// named palette reordered by the sheet's OBP. Every opaque pixel then
/// draws its shade's color from the named palette, through the frame
/// lists, the tile data and the slot.
#[test]
fn every_pixel_of_an_indexed_sheet_draws_its_shade_s_color_on_the_gbc() {
    let (temp, options) = starter("indexed-colors");
    hero(&temp, "hero");
    write_png(
        &temp.path("demo/assets/sprites/ghost.8x16.png"),
        16,
        16,
        ghost,
    );
    write_png(&temp.path("demo/assets/sprites/gem.16x16.png"), 32, 16, gem);
    palettes(
        &temp,
        "sprites/hero.16x16.png = ember\n\
         sprites/ghost.8x16.png = ember\n\
         sprites/gem.16x16.png = forest\n",
    );
    let made = images(build(&rgbds(), &gbc(&options)).unwrap());
    let sheets = [
        Art {
            file: "hero.16x16.png",
            label: "Hero",
            size: (48, 16),
            frame: (16, 16),
            pixel: hero_pixel,
            named: ember(),
        },
        Art {
            file: "ghost.8x16.png",
            label: "Ghost",
            size: (16, 16),
            frame: (8, 16),
            pixel: ghost,
            named: ember(),
        },
        Art {
            file: "gem.16x16.png",
            label: "Gem",
            size: (32, 16),
            frame: (16, 16),
            pixel: gem,
            named: forest(),
        },
    ];
    for art in &sheets {
        assert_drawn_in_shades(&temp, &made, art);
    }
}

/// OBJ slots are keyed by palette and shade mapping: two sheets assigned
/// one palette but drawn in different shades have different OBPs, so they
/// take two slots, and sheets alike in both share one. Slots follow the
/// palettes' file order, then each palette's mappings by OBP value.
#[test]
fn sheets_with_one_palette_and_different_shades_take_two_slots() {
    let (temp, options) = starter("indexed-mappings");
    hero(&temp, "hero");
    hero(&temp, "coin");
    write_png(
        &temp.path("demo/assets/sprites/ghost.8x16.png"),
        16,
        16,
        ghost,
    );
    write_png(&temp.path("demo/assets/sprites/gem.16x16.png"), 32, 16, gem);
    palettes(
        &temp,
        "sprites/hero.16x16.png = ember\n\
         sprites/coin.16x16.png = ember\n\
         sprites/ghost.8x16.png = ember\n\
         sprites/gem.16x16.png = forest\n",
    );
    let made = images(build(&rgbds(), &gbc(&options)).unwrap());
    let inc = fs::read_to_string(temp.path("demo/build/images/images.inc")).unwrap();
    // hero and coin: shades 2, 3; ghost: 0, 3; gem: 0, 1, 3.
    assert_eq!(binary_constant(&inc, "HERO_OBP"), 0b00_11_10_00);
    assert_eq!(binary_constant(&inc, "GHOST_OBP"), 0b00_11_00_00);
    assert_eq!(binary_constant(&inc, "GEM_OBP"), 0b11_01_00_00);
    let first_slot = |name: &str| {
        image(&made, name)
            .tile_map
            .iter()
            .flatten()
            .next()
            .expect("an opaque unit")
            .palette
    };
    assert_eq!(first_slot("gem.16x16.png"), slot(0));
    assert_eq!(first_slot("ghost.8x16.png"), slot(1), "ember's lower OBP");
    assert_eq!(first_slot("hero.16x16.png"), slot(2));
    assert_eq!(
        first_slot("coin.16x16.png"),
        slot(2),
        "hero's shades: its slot"
    );
    assert_eq!(
        made.gbc.unwrap().obj_palettes,
        [
            by_obp(forest(), 0, 1, 3),
            by_obp(ember(), 0, 3, 0),
            by_obp(ember(), 2, 3, 0)
        ]
    );
}

/// One palette in nine shade sets is nine OBJ slots: `gbc_palettes`.
#[test]
fn one_palette_in_nine_shade_sets_overflows_the_eight() {
    let (temp, options) = starter("indexed-nine-sets");
    let sets: [&[usize]; 9] = [
        &[0],
        &[1],
        &[2],
        &[3],
        &[0, 1],
        &[0, 2],
        &[0, 3],
        &[1, 2],
        &[1, 3],
    ];
    let mut text = String::from(PALETTES);
    for (i, set) in sets.iter().enumerate() {
        let set = set.to_vec();
        write_png(
            &temp.path(&format!("demo/assets/sprites/s{i}.png")),
            8,
            16,
            move |x, y| match set.get(x as usize) {
                Some(&s) if y == 0 => [WHITE, LIGHT, DARK, BLACK][s],
                _ => CLEAR,
            },
        );
        text += &format!("sprites/s{i}.png = forest\n");
    }
    fs::write(temp.path("demo/assets/palettes.txt"), text).unwrap();
    let made = match build(&rgbds(), &gbc(&options)) {
        Err(gbcli_build::BuildError::Failed { made, .. }) => made,
        other => panic!("{other:?}"),
    };
    let p = problem(&made, ImageLimit::GbcPalettes);
    assert_eq!((p.severity, p.count, p.max), (Severity::Error, 9, 8));
}

/// Every result the build makes, DMG and GBC, indexed and color art,
/// backgrounds and sheets with transparent units, reads back as it was
/// sent: `ImageAsset`'s read check never refuses the build's own output.
#[test]
fn every_built_result_round_trips() {
    let (temp, options) = starter("indexed-round-trip");
    sky(&temp);
    write_png(&temp.path("demo/assets/bg/field.png"), 16, 8, |x, _| {
        if x < 8 {
            [0xFF, 0, 0, 0xFF]
        } else {
            [0, 0, 0xFF, 0xFF]
        }
    });
    hero(&temp, "hero");
    write_png(
        &temp.path("demo/assets/sprites/ghost.8x16.png"),
        16,
        16,
        ghost,
    );
    write_png(
        &temp.path("demo/assets/sprites/bolt.16x16.png"),
        32,
        16,
        |x, y| {
            if x < 8 && y < 4 {
                [0xFF, 0, 0, 0xFF]
            } else {
                CLEAR
            }
        },
    );
    palettes(
        &temp,
        "bg/sky.png = forest\nsprites/hero.16x16.png = ember\nsprites/ghost.8x16.png = ember\n",
    );
    for opts in [options.clone(), gbc(&options)] {
        let made = build(&rgbds(), &opts).unwrap();
        let json = serde_json::to_string(&made).unwrap();
        let back: BuildMade = serde_json::from_str(&json).unwrap_or_else(|e| panic!("{e}: {json}"));
        assert_eq!(back, made, "{json}");
    }
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

/// Without `--gbc`, nothing GBC is reported: no GBC version, and a tile
/// map without slots (filled on every build, Ben's ruling of 2026-11-07).
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
        assert_eq!(i.gbc, None, "{}", i.source);
        assert!(!i.tile_map.is_empty(), "{}", i.source);
        let slots = i.tile_map.iter().flatten().filter(|e| e.palette.is_some());
        assert_eq!(slots.count(), 0, "{}", i.source);
    }
}
