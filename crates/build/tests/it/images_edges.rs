//! `build` over images at the edges the first review found (leaf JAzaVy):
//! colors rgbgfx sees as one, sheets and frames at the size of what holds
//! them (rgbgfx's tile limit, OAM's byte positions, a ROMX bank), names
//! that collide in images.inc, and file names gbcli must not skip.

use crate::common;
use crate::pngs;

use std::fs;

use gbcli_build::{BuildError, build};
use gbcli_protocol::ImageLimit;

use common::{rgbds, starter};
use pngs::*;

/// One 8x16 object (a black stroke) repeated in every 8x16 cell.
fn stroke(x: u32, _y: u32) -> Rgba {
    if x % 8 == 1 { BLACK } else { CLEAR }
}

/// Black and white as two near colors that are each one RGB555 color with
/// them: rgbgfx keeps 5 bits a channel, so #040404 is #000000.
const NEAR_BLACK: Rgba = [4, 4, 4, 0xFF];

/// Makes the starter's main.asm include images.inc, as a program using its
/// constants does.
fn include_images(temp: &common::Temp) {
    let main = temp.path("demo/src/main.asm");
    let text = fs::read_to_string(&main).unwrap().replacen(
        "include \"hardware.inc\"\n",
        "include \"hardware.inc\"\ninclude \"images.inc\"\n",
        1,
    );
    fs::write(&main, text).unwrap();
}

fn describe(r: &Result<gbcli_protocol::BuildMade, BuildError>) -> String {
    match r {
        Ok(_) => "ok".into(),
        Err(BuildError::Failed { made, .. }) => format!(
            "failed: diagnostics {:#?}, images {:#?}",
            made.diagnostics,
            made.images.as_ref().map(|i| &i.problems)
        ),
        Err(e) => format!("{:?}", e.body()),
    }
}

/// The layout error a build stops with, as (message, remedy).
fn layout_error(r: Result<gbcli_protocol::BuildMade, BuildError>) -> (String, String) {
    match r {
        Err(e @ BuildError::Layout { .. }) => {
            let body = e.body();
            (body.message, body.remedy)
        }
        other => panic!("expected a layout error, got {}", describe(&other)),
    }
}

/// A sheet of many frames that all reuse one 8x16 unit needs 2 tiles,
/// though the sheet itself is 17x32 = 544 tiles: rgbgfx must not count the
/// raw sheet against a tile limit gbcli's own dedup already checked.
#[test]
fn a_big_sheet_of_one_repeated_object_builds() {
    let (temp, options) = starter("edges-big-sheet");
    write_png(
        &temp.path("demo/assets/sprites/hero.8x16.png"),
        136,
        256,
        stroke,
    );
    let r = build(&rgbds(), &options);
    let Ok(made) = &r else {
        panic!("{}", describe(&r));
    };
    let images = made.images.as_ref().unwrap();
    assert_eq!(
        (images.images[0].tiles, images.images[0].frames),
        (2, Some(17 * 16))
    );
}

/// `assets/bg/hero-frame.png` and `assets/sprites/hero.png` have labels
/// `HeroFrame` and `Hero`, but both define HERO_FRAME_WIDTH in images.inc.
#[test]
fn constants_that_collide_are_a_layout_error_naming_both_files() {
    let (temp, options) = starter("edges-const-collide");
    include_images(&temp);
    write_png(&temp.path("demo/assets/bg/hero-frame.png"), 8, 8, |_, _| {
        WHITE
    });
    write_png(&temp.path("demo/assets/sprites/hero.png"), 8, 16, stroke);
    let (message, remedy) = layout_error(build(&rgbds(), &options));
    for needle in [
        "demo/assets/bg/hero-frame.png",
        "demo/assets/sprites/hero.png",
        "HERO_FRAME_WIDTH",
    ] {
        assert!(message.contains(needle), "{needle} missing: {message}");
    }
    assert!(remedy.contains("rename"), "{remedy}");
}

/// `assets/bg/hero.png` and `assets/sprites/HERO.png`: labels `Hero` and
/// `HERO` differ, but both define HERO_TILES.
#[test]
fn names_differing_only_in_case_are_a_layout_error() {
    let (temp, options) = starter("edges-case-collide");
    include_images(&temp);
    write_png(&temp.path("demo/assets/bg/hero.png"), 8, 8, |_, _| WHITE);
    write_png(&temp.path("demo/assets/sprites/HERO.png"), 8, 16, stroke);
    let (message, _) = layout_error(build(&rgbds(), &options));
    assert!(message.contains("HERO_TILES"), "{message}");
}

/// A 24x16 sheet named for 16x16 frames: the frames don't tile its width.
/// That is `frame_grid`, the sheet's width over the most frames fit, with a
/// remedy that offers a frame size that does tile it.
#[test]
fn a_frame_width_that_doesnt_tile_the_sheet_is_frame_grid_offering_one_that_does() {
    let (temp, options) = starter("edges-frame-grid");
    write_png(
        &temp.path("demo/assets/sprites/hero.16x16.png"),
        24,
        16,
        stroke,
    );
    let made = refused(&options);
    assert_eq!(errors(&made), [ImageLimit::FrameGrid]);
    let p = problem(&made, ImageLimit::FrameGrid);
    assert_eq!((p.count, p.max), (24, 16));
    assert!(p.remedy.contains("hero.8x16.png"), "{}", p.remedy);
    assert!(p.remedy.contains("32x16"), "{}", p.remedy);
}

/// A frame's height that isn't whole 8x16 objects stays `obj_pair_8x16`,
/// counted against the height that would be.
#[test]
fn a_frame_height_that_isnt_whole_objects_is_obj_pair_8x16() {
    let (temp, options) = starter("edges-frame-pair");
    write_png(
        &temp.path("demo/assets/sprites/hero.8x24.png"),
        8,
        48,
        stroke,
    );
    let made = refused(&options);
    let p = problem(&made, ImageLimit::ObjPair8x16);
    assert_eq!((p.count, p.max), (24, 16));
    assert!(!p.remedy.contains("hero.8x24.png"), "{}", p.remedy);
}

/// Three tiles in white, #040404 and black: two colors to rgbgfx and to
/// the DMG, so the background builds and both dark tiles are black.
#[test]
fn colors_equal_in_rgb555_are_one_color() {
    let (temp, options) = starter("edges-rgb555");
    write_png(
        &temp.path("demo/assets/bg/town.png"),
        24,
        8,
        |x, _| match x / 8 {
            0 => WHITE,
            1 => NEAR_BLACK,
            _ => BLACK,
        },
    );
    let r = build(&rgbds(), &options);
    let Ok(made) = &r else {
        panic!("{}", describe(&r));
    };
    assert_eq!(made.images.as_ref().unwrap().images[0].tiles, 2);
    let map = fs::read(temp.path("demo/build/images/bg/town.tilemap")).unwrap();
    assert_eq!(map[1], map[2], "the near-black tile is the black one");
}

/// A sprite drawn in #040404 and #000000 is one color: both pixels take one
/// index, and OBP draws it black.
#[test]
fn sprite_colors_equal_in_rgb555_share_one_index_drawn_black() {
    let (temp, options) = starter("edges-rgb555-sprite");
    include_images(&temp);
    write_png(
        &temp.path("demo/assets/sprites/hero.png"),
        8,
        16,
        |x, _| match x {
            0 => NEAR_BLACK,
            1 => BLACK,
            _ => CLEAR,
        },
    );
    let r = build(&rgbds(), &options);
    let Ok(_) = r else {
        panic!("{}", describe(&r));
    };
    let data = fs::read(temp.path("demo/build/images/sprites/hero.2bpp")).unwrap();
    let index = index_at(&data, 0, 0);
    assert_eq!(index, index_at(&data, 0, 1), "{data:02x?}");
    let inc = fs::read_to_string(temp.path("demo/build/images/images.inc")).unwrap();
    let obp = inc
        .lines()
        .find_map(|l| l.strip_prefix("DEF HERO_OBP EQU %"))
        .unwrap();
    let bits: String = obp.split(' ').next().unwrap().replace('_', "");
    let obp = u8::from_str_radix(&bits, 2).unwrap();
    assert_eq!(
        (obp >> (2 * index)) & 3,
        3,
        "index {index} is black in {obp:08b}"
    );
}

/// The same in one background tile, where no tile count can catch it: both
/// halves are black (index 3 under BGP $E4).
#[test]
fn background_colors_equal_in_rgb555_in_one_tile_share_one_shade() {
    let (temp, options) = starter("edges-rgb555-bg-tile");
    write_png(&temp.path("demo/assets/bg/town.png"), 8, 8, |x, y| {
        match (x, y) {
            (_, 4..) => WHITE,
            (0..4, _) => NEAR_BLACK,
            _ => BLACK,
        }
    });
    let r = build(&rgbds(), &options);
    let Ok(_) = r else {
        panic!("{}", describe(&r));
    };
    let data = fs::read(temp.path("demo/build/images/bg/town.2bpp")).unwrap();
    assert_eq!(
        (index_at(&data, 0, 0), index_at(&data, 0, 4)),
        (3, 3),
        "{data:02x?}"
    );
}

/// `assets/bg/town.PNG` is converted like `town.png`.
#[test]
fn an_upper_case_png_extension_is_converted() {
    let (temp, options) = starter("edges-upper-ext");
    write_png(&temp.path("demo/assets/bg/town.PNG"), 8, 8, |_, _| WHITE);
    let r = build(&rgbds(), &options);
    let Ok(made) = &r else {
        panic!("{}", describe(&r));
    };
    let images = &made.images.as_ref().expect("town.PNG was skipped").images;
    assert_eq!(images[0].label, "Town");
    assert!(temp.path("demo/build/images/bg/town.2bpp").is_file());
}

/// A 1024x1024 background of one tile fits every tile check, but its
/// 16 KiB map plus its tile don't fit the one ROMX bank its section is.
#[test]
fn a_background_too_big_for_a_bank_is_romx_bank() {
    let (temp, options) = starter("edges-big-map");
    write_png(
        &temp.path("demo/assets/bg/world.png"),
        1024,
        1024,
        |_, _| WHITE,
    );
    let made = refused(&options);
    assert_eq!(errors(&made), [ImageLimit::RomxBank]);
    let p = problem(&made, ImageLimit::RomxBank);
    assert_eq!((p.count, p.max), (16 + 128 * 128, 16384));
    assert!(made.diagnostics.is_empty(), "{:#?}", made.diagnostics);
}

/// 2560 one-object 8x16 frames: each frame's pointer and list (2 + 1 + 4
/// bytes) plus the one unit's 32 pass a bank too.
#[test]
fn a_sheet_too_big_for_a_bank_is_romx_bank() {
    let (temp, options) = starter("edges-big-frames");
    write_png(
        &temp.path("demo/assets/sprites/walk.8x16.png"),
        1024,
        320,
        stroke,
    );
    let made = refused(&options);
    assert_eq!(errors(&made), [ImageLimit::RomxBank]);
    let p = problem(&made, ImageLimit::RomxBank);
    assert_eq!((p.count, p.max), (32 + 2560 * 7, 16384));
}

/// An unnamed 8x272 strip is one frame of 17 objects in a column, within
/// obj_total and obj_per_line, but OAM positions are bytes: an object 256
/// pixels below another can't be placed from one frame position.
#[test]
fn a_frame_taller_than_256_pixels_is_frame_span() {
    let (temp, options) = starter("edges-tall-frame");
    write_png(&temp.path("demo/assets/sprites/strip.png"), 8, 272, stroke);
    let made = refused(&options);
    assert_eq!(errors(&made), [ImageLimit::FrameSpan]);
    let p = problem(&made, ImageLimit::FrameSpan);
    assert_eq!((p.count, p.max), (272, 256));
}

/// A 256-pixel frame is the most that fits: its last object sits at 240,
/// emitted as written, and the build has nothing to say.
#[test]
fn a_frame_256_pixels_tall_builds_with_its_offsets() {
    let (temp, options) = starter("edges-256-frame");
    write_png(&temp.path("demo/assets/sprites/strip.png"), 8, 256, stroke);
    let r = build(&rgbds(), &options);
    let Ok(made) = &r else {
        panic!("{}", describe(&r));
    };
    assert!(made.diagnostics.is_empty(), "{:#?}", made.diagnostics);
    let asm = fs::read_to_string(temp.path("demo/build/images/images.asm")).unwrap();
    assert!(asm.contains("\tdb 240, 0, 0, 0\n"), "{asm}");
}

/// A 16-bit gray PNG (second review, leaf FPQsnp). rgbgfx scales 16-bit
/// samples to 8 bits with libpng's rounding: $07FF and $0800 both become
/// $08, one color. gbcli once kept the high byte ($07 and $08, two RGB555
/// colors), planned three shades where rgbgfx saw two and built the
/// $07FF pixels a shade off. Read as rgbgfx reads it, the image is white
/// and one dark color, drawn as shade 3 everywhere below the top row.
#[test]
fn a_16_bit_png_is_read_as_rgbgfx_rounds_it() {
    let (temp, options) = starter("edges-16bit");
    let path = temp.path("demo/assets/bg/deep.png");
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    let mut data = Vec::new();
    for y in 0..8u32 {
        for x in 0..8u32 {
            let v: u16 = match (y, x) {
                (0, _) => 0xFFFF,
                (_, 0..4) => 0x07FF,
                _ => 0x0800,
            };
            data.extend_from_slice(&v.to_be_bytes());
        }
    }
    let file = fs::File::create(&path).unwrap();
    let mut encoder = png::Encoder::new(file, 8, 8);
    encoder.set_color(png::ColorType::Grayscale);
    encoder.set_depth(png::BitDepth::Sixteen);
    let mut writer = encoder.write_header().unwrap();
    writer.write_image_data(&data).unwrap();
    drop(writer);

    let r = build(&rgbds(), &options);
    assert!(r.is_ok(), "{}", describe(&r));
    let tiles = fs::read(temp.path("demo/build/images/bg/deep.2bpp")).unwrap();
    let (a, b) = (index_at(&tiles, 1, 0), index_at(&tiles, 1, 7));
    assert_eq!(
        (index_at(&tiles, 0, 0), a, b),
        (0, 3, 3),
        "white, then the $07FF and $0800 pixels"
    );
}

/// A sheet whose name is no label and whose frame size is no size: the
/// size is checked first, and its remedy must offer a name that is one
/// (second review, leaf FPQsnp).
#[test]
fn a_bad_frame_size_remedy_offers_a_valid_name() {
    let (temp, options) = starter("edges-badname");
    write_png(
        &temp.path("demo/assets/sprites/2hero.big.png"),
        8,
        16,
        |_, _| BLACK,
    );
    let (_, remedy) = layout_error(build(&rgbds(), &options));
    assert!(remedy.contains(" hero.16x16.png"), "{remedy}");
    assert!(!remedy.contains("2hero"), "{remedy}");
}
