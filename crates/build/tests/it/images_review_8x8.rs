//! From the review of 372d772 (review-img14): plain and --gbc builds agree on
//! the DMG data of the same color art, and 8x8 sheets' frame lists carry each
//! object's own GBC palette.

use crate::common;
use crate::pngs;

use std::fs;
use std::path::Path;

use gbcli_build::{BuildOptions, build};
use gbcli_protocol::{BuildTarget, ImageAsset, ImageLimit, ImagesMade, ObjectSize, Rgb555};

use common::{Temp, rgbds, starter};
use pngs::*;

// Light colors near shade 1, mid near shade 2, dark near shade 3, under
// the dmg line `pinned` writes, so every sheet draws 3 shades.
const PINK: Rgba = [0xF0, 0x90, 0xA0, 0xFF];
const MINT: Rgba = [0x90, 0xC0, 0x90, 0xFF];
const SKY: Rgba = [0x90, 0xA8, 0xF0, 0xFF];
const RED: Rgba = [0xE0, 0x20, 0x20, 0xFF];
const GREEN: Rgba = [0x30, 0x80, 0x30, 0xFF];
const BLUE: Rgba = [0x40, 0x50, 0xA0, 0xFF];
const MAROON: Rgba = [0x40, 0x08, 0x10, 0xFF];
const FOREST: Rgba = [0x08, 0x20, 0x08, 0xFF];
const NAVY: Rgba = [0x08, 0x10, 0x50, 0xFF];

/// Pins the shades to the DMG grays with a dmg line.
pub(crate) fn pinned(temp: &Temp) {
    let path = temp.path("demo/assets/palettes.txt");
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    fs::write(path, "dmg: #ffffff #aaaaaa #555555 #000000\n").unwrap();
}

/// A 16x16 sheet of four 8x8 frames, each object in its own three colors
/// (so the packer needs several OBJ palettes), shaped so no two objects are
/// alike or flips of one another.
pub(crate) fn quad(x: u32, y: u32) -> Rgba {
    let sets = [
        [PINK, RED, MAROON],
        [MINT, GREEN, FOREST],
        [SKY, BLUE, NAVY],
        [PINK, GREEN, NAVY],
    ];
    let f = ((y / 8) * 2 + x / 8) as usize;
    let (lx, ly) = (x % 8, y % 8);
    let set = sets[f];
    if lx <= ly && lx + ly <= 7 + f as u32 {
        set[((lx + 2 * ly + f as u32) % 3) as usize]
    } else {
        CLEAR
    }
}

/// A 2bpp tile's color index at (x, y).
fn index(tile: &[u8], x: u32, y: u32) -> u8 {
    let (lo, hi) = (tile[2 * y as usize], tile[2 * y as usize + 1]);
    let bit = 7 - x;
    ((lo >> bit) & 1) | (((hi >> bit) & 1) << 1)
}

/// Every pixel the GBC data draws, through the tile map, its flips, its
/// slots and the OBJ palettes, is the color image's (RGB555), and every
/// transparent one is transparent.
pub(crate) fn gbc_draws_the_color_image(
    made: &ImagesMade,
    image: &ImageAsset,
    gbc: &[u8],
    color: &dyn Fn(u32, u32) -> Rgba,
) {
    let size = image.object_size.expect("a sheet");
    let (per, oh) = (size.tiles(), size.height());
    let palettes = &made.gbc.as_ref().expect("a GBC build").obj_palettes;
    let rows = image.height / per;
    for cy in 0..rows {
        for cx in 0..image.width {
            let cell = (cy * image.width + cx) as usize;
            let entry = image.tile_map[cell];
            for y in 0..oh {
                for x in 0..8 {
                    let want = color(cx * 8 + x, cy * oh + y);
                    let Some(e) = entry else {
                        assert_eq!(
                            want[3], 0,
                            "cell {cell} has no entry but ({x},{y}) is opaque"
                        );
                        continue;
                    };
                    let sx = if e.flip.x() { 7 - x } else { x };
                    let sy = if e.flip.y() { oh - 1 - y } else { y };
                    let tile = e.index + sy / 8;
                    let data = &gbc[tile as usize * 16..][..16];
                    let i = index(data, sx, sy % 8);
                    if i == 0 {
                        assert_eq!(
                            want[3], 0,
                            "cell {cell} ({x},{y}): drawn clear, image opaque"
                        );
                        continue;
                    }
                    let slot = e.palette.expect("a color sheet's slot").get();
                    let drawn = palettes[usize::from(slot)].0[usize::from(i)];
                    assert_eq!(
                        drawn,
                        Rgb555::from_rgb888(want[0], want[1], want[2]),
                        "{}: cell {cell} pixel ({x},{y}) slot {slot} index {i}",
                        image.source
                    );
                }
            }
        }
    }
}

/// Each frame list's 4-byte objects, attributes as written.
pub(crate) fn frame_objects(asm: &str, label: &str) -> Vec<Vec<(u32, u32, u32, u8)>> {
    let mut frames = Vec::new();
    let mut current: Option<Vec<(u32, u32, u32, u8)>> = None;
    for line in asm.lines() {
        if let Some(name) = line.strip_suffix("::") {
            if let Some(f) = current.take() {
                frames.push(f);
            }
            if name.starts_with(&format!("{label}Frame")) && name != format!("{label}Frames") {
                current = Some(Vec::new());
            }
            continue;
        }
        let Some(f) = current.as_mut() else { continue };
        let Some(rest) = line.strip_prefix("\tdb ") else {
            continue;
        };
        let parts: Vec<&str> = rest.split(", ").collect();
        if parts.len() != 4 {
            continue;
        }
        let n = |s: &str| -> u32 {
            s.strip_prefix('$').map_or_else(
                || s.parse().unwrap(),
                |h| u32::from_str_radix(h, 16).unwrap(),
            )
        };
        f.push((n(parts[0]), n(parts[1]), n(parts[2]), n(parts[3]) as u8));
    }
    if let Some(f) = current.take() {
        frames.push(f);
    }
    frames
}

/// An 8x8 color sheet of two rows under --gbc: every pixel the GBC data
/// draws is the color image's, through the tile map's slots, and every
/// frame list's palette bits are the tile map's slot.
#[test]
fn an_8x8_color_sheet_of_two_rows_draws_its_colors_on_the_gbc() {
    let (temp, mut options) = starter("review-img14-8x8-gbc");
    options.target = BuildTarget::Gbc;
    pinned(&temp);
    write_png(&temp.path("demo/assets/sprites/quad.8x8.png"), 16, 16, quad);
    let made = build(&rgbds(), &options).unwrap().images.unwrap();
    assert!(made.first_error().is_none(), "{:#?}", made.problems);
    let image = made.images.iter().find(|i| i.label == "Quad").unwrap();
    assert_eq!(image.object_size, Some(ObjectSize::Square));
    assert!(
        made.gbc.as_ref().unwrap().obj_palettes.len() >= 2,
        "several palettes"
    );
    let gbc = fs::read(temp.path("demo/build/images/sprites/quad.gbc.2bpp")).unwrap();
    gbc_draws_the_color_image(&made, image, &gbc, &quad);
    // Frame f's one object: its tile and slot are the tile map's.
    let asm = fs::read_to_string(temp.path("demo/build/images/images.asm")).unwrap();
    let frames = frame_objects(&asm, "Quad");
    assert_eq!(frames.len(), 4, "{asm}");
    for (f, objects) in frames.iter().enumerate() {
        let [(_, _, tile, attr)] = objects[..] else {
            panic!("frame {f}: {objects:?}")
        };
        let e = image.tile_map[f].unwrap();
        assert_eq!(
            tile,
            e.index,
            "frame {f}: {:?}\n{}",
            image.tile_map,
            &asm[asm.find("SECTION \"image Quad\"").unwrap()..]
        );
        assert_eq!(
            attr & 7,
            e.palette.unwrap().get(),
            "frame {f}: {:?}\n{}",
            image.tile_map,
            &asm[asm.find("SECTION \"image Quad\"").unwrap()..]
        );
    }
    // And the result reads back off the wire.
    let json = serde_json::to_string(&made).unwrap();
    assert_eq!(serde_json::from_str::<ImagesMade>(&json).unwrap(), made);
}

/// The same check for an 8x16 color sheet, so the 8x8 work left it alone.
#[test]
fn an_8x16_color_sheet_draws_its_colors_on_the_gbc() {
    let (temp, mut options) = starter("review-img14-8x16-gbc");
    options.target = BuildTarget::Gbc;
    pinned(&temp);
    write_png(&temp.path("demo/assets/sprites/duo.png"), 16, 16, duo);
    let made = build(&rgbds(), &options).unwrap().images.unwrap();
    assert!(made.problems.is_empty(), "{:#?}", made.problems);
    let image = made.images.iter().find(|i| i.label == "Duo").unwrap();
    assert_eq!(image.object_size, Some(ObjectSize::Tall));
    let gbc = fs::read(temp.path("demo/build/images/sprites/duo.gbc.2bpp")).unwrap();
    gbc_draws_the_color_image(&made, image, &gbc, &duo);
    let asm = fs::read_to_string(temp.path("demo/build/images/images.asm")).unwrap();
    let [frame] = &frame_objects(&asm, "Duo")[..] else {
        panic!("{asm}")
    };
    for (o, &(_, _, tile, attr)) in frame.iter().enumerate() {
        let e = image.tile_map[o].unwrap();
        assert_eq!(
            (tile, attr & 7),
            (e.index, e.palette.unwrap().get()),
            "object {o}"
        );
    }
}

/// A 16x16 sheet of two 8x16 objects, each in one of quad's color sets.
fn duo(x: u32, y: u32) -> Rgba {
    let sets = [[PINK, RED, MAROON], [SKY, BLUE, NAVY]];
    let set = sets[(x / 8) as usize];
    let (lx, ly) = (x % 8, y);
    if lx <= ly / 2 + x / 8 {
        set[((lx + ly) % 3) as usize]
    } else {
        CLEAR
    }
}

/// Two indexed 8x8 sheets of different palettes, so the second takes OBJ
/// slot 1: every object of its frame lists must say slot 1, the slot its
/// tile map gives every entry.
#[test]
fn an_indexed_8x8_sheet_in_slot_1_says_slot_1_in_every_object() {
    let (temp, mut options) = starter("review-img14-8x8-indexed");
    options.target = BuildTarget::Gbc;
    write_png(&temp.path("demo/assets/sprites/a.8x8.png"), 8, 8, |x, _| {
        if x < 4 { DARK } else { CLEAR }
    });
    // Four distinct objects, one a frame.
    write_png(
        &temp.path("demo/assets/sprites/b.8x8.png"),
        32,
        8,
        |x, y| {
            if x % 8 <= y / 2 + x / 8 { BLACK } else { CLEAR }
        },
    );
    fs::write(
        temp.path("demo/assets/palettes.txt"),
        "dmg: #ffffff #aaaaaa #555555 #000000\nice: #fff #8cf #46a #013\n\
         ember: #ffe #f80 #a20 #200\nsprites/a.8x8.png = ice\nsprites/b.8x8.png = ember\n",
    )
    .unwrap();
    let made = build(&rgbds(), &options).unwrap().images.unwrap();
    assert!(made.problems.is_empty(), "{:#?}", made.problems);
    let b = made.images.iter().find(|i| i.label == "B").unwrap();
    let slots: Vec<u8> = b
        .tile_map
        .iter()
        .flatten()
        .map(|e| e.palette.unwrap().get())
        .collect();
    assert_eq!(slots, [1, 1, 1, 1], "the tile map");
    let asm = fs::read_to_string(temp.path("demo/build/images/images.asm")).unwrap();
    let attrs: Vec<u8> = frame_objects(&asm, "B")
        .iter()
        .flatten()
        .map(|o| o.3 & 7)
        .collect();
    assert_eq!(attrs, [1, 1, 1, 1], "the frame lists");
}

/// One project, built plain and with --gbc: the DMG data (tiles, maps,
/// OBP/BGP and every constant both define, frame lists without the GBC
/// palette bits) and the non-GBC problems, shared_shade aside, are the
/// same.
/// What a build says about the DMG side: tile and map files, include
/// constants, frame lists (palette bits masked) and problems.
type DmgSide = (
    Vec<(String, Vec<u8>)>,
    Vec<(String, String)>,
    Vec<Vec<Vec<(u32, u32, u32, u8)>>>,
    Vec<String>,
);

#[test]
fn plain_and_gbc_builds_agree_on_the_dmg_data() {
    let mut seen: Vec<DmgSide> = Vec::new();
    for target in [BuildTarget::Dmg, BuildTarget::Gbc] {
        let (temp, mut options): (Temp, BuildOptions) =
            starter(&format!("review-img14-agree-{}", target.name()));
        options.target = target;
        pinned(&temp);
        // Color art: a background whose colors fall to 4 shades with no two
        // tiles alike on the DMG only (see the next probe), an 8x8 sheet, an
        // 8x16 sheet with an override; indexed art: a gray tileset.
        write_png(&temp.path("demo/assets/bg/town.png"), 48, 16, |x, y| {
            [WHITE, MINT, RED, NAVY][((x / 8 + y / 8) % 4) as usize]
        });
        write_png(&temp.path("demo/assets/sprites/quad.8x8.png"), 16, 16, quad);
        write_png(&temp.path("demo/assets/sprites/duo.png"), 16, 16, duo);
        write_png(
            &temp.path("demo/assets/sprites/duo.dmg.png"),
            16,
            16,
            |x, y| {
                if duo(x, y)[3] == 0 {
                    CLEAR
                } else {
                    [WHITE, DARK, BLACK][((x + y) % 3) as usize]
                }
            },
        );
        write_png(&temp.path("demo/assets/tiles/font.png"), 16, 8, |x, _| {
            [WHITE, BLACK][(x / 8) as usize]
        });
        indexed(&temp.path("demo"), &["tiles/font.png"]);
        let made = build(&rgbds(), &options).unwrap().images.unwrap();
        let root = temp.path("demo/build/images");
        let mut data = Vec::new();
        for kind in ["bg", "sprites", "tiles"] {
            let mut names: Vec<_> = fs::read_dir(root.join(kind))
                .unwrap()
                .map(|e| e.unwrap().path())
                .collect();
            names.sort();
            for p in names {
                let n = p.file_name().unwrap().to_string_lossy().to_string();
                if (n.ends_with(".2bpp") || n.ends_with(".tilemap")) && !n.contains(".gbc.") {
                    data.push((n, fs::read(&p).unwrap()));
                }
            }
        }
        let inc = fs::read_to_string(root.join("images.inc")).unwrap();
        let consts: Vec<(String, String)> = inc
            .lines()
            .filter_map(|l| l.strip_prefix("DEF "))
            .filter_map(|l| {
                let (k, v) = l.split_once(" EQU ")?;
                Some((k.to_string(), v.split(' ').next()?.to_string()))
            })
            .filter(|(k, _)| !k.ends_with("_PALETTES") && !k.starts_with("PALETTE_"))
            .collect();
        let asm = fs::read_to_string(root.join("images.asm")).unwrap();
        let lists = ["Quad", "Duo"]
            .iter()
            .map(|l| {
                frame_objects(&asm, l)
                    .into_iter()
                    .map(|f| {
                        f.into_iter()
                            .map(|(y, x, t, a)| (y, x, t, a & !7))
                            .collect()
                    })
                    .collect()
            })
            .collect();
        // shared_shade's scope differs by ruling (Ben, 2026-11-08, leaf
        // Vho2Gc): image-wide in a plain build, per tile or object under
        // --gbc; images_dmg_version pins each.
        let gbc_only = [
            ImageLimit::ShadesUnpinned,
            ImageLimit::GbcPalettes,
            ImageLimit::GbcTileColors,
            ImageLimit::SharedShade,
        ];
        let problems = made
            .problems
            .iter()
            .filter(|p| !gbc_only.contains(&p.limit))
            .map(|p| format!("{:?} {:?} {}", p.limit, p.image, p.message))
            .collect();
        seen.push((data, consts, lists, problems));
        let _ = Path::new("");
    }
    let (dmg, gbc) = (&seen[0], &seen[1]);
    assert_eq!(dmg.0, gbc.0, "DMG tile and map data");
    assert_eq!(dmg.1, gbc.1, "include constants");
    assert_eq!(dmg.2, gbc.2, "frame lists");
    assert_eq!(dmg.3, gbc.3, "problems");
}
