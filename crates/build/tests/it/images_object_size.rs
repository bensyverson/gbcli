//! The project-wide object size (leaf mjBkTn; Ben's ruling, 2026-11-08):
//! LCDC bit 2 sets one object size for the whole screen, so an `objects`
//! line in `assets/palettes.txt` (a manifest's `objects` field) makes every
//! sheet's objects 8x8, or 8x16, whatever its frame height. Without it a
//! sheet's frame height decides, as before.

use crate::common;
use crate::images_review_8x8::{frame_objects, gbc_draws_the_color_image, pinned, quad};
use crate::pngs;

use std::fs;
use std::path::Path;

use gbcli_build::{AssetsError, AssetsInput, AssetsOptions, BuildOptions, assets, build};
use gbcli_pixels::{Layout, decode_png, frame_size, from_raster, to_text};
use gbcli_protocol::{
    AssetManifest, BuildTarget, GbcPalette, ImageLimit, ImagesMade, ManifestImage, NamedPalettes,
    ObjectSize, Rgb555,
};

use common::{Temp, rgbds, starter};
use pngs::*;

/// What the include says when the project sets 8x8 objects.
const SAYS_8X8: &str = "clear LCDC_OBJ_16";

/// Pins the shades and sets the object size with an `objects` line.
fn objects(temp: &Temp, size: &str) {
    pinned(temp);
    let path = temp.path("demo/assets/palettes.txt");
    let mut text = fs::read_to_string(&path).unwrap();
    text += &format!("objects: {size}\n");
    fs::write(path, text).unwrap();
}

fn text(temp: &Temp, file: &str) -> String {
    fs::read_to_string(temp.path(&format!("demo/build/images/{file}"))).unwrap()
}

fn built(options: &BuildOptions) -> ImagesMade {
    let made = build(&rgbds(), options).unwrap().images.unwrap();
    assert!(made.first_error().is_none(), "{:#?}", made.problems);
    made
}

/// A 16x16 frame under `objects: 8x8`, built with --gbc: four 8x8 objects
/// whose tiles, frame list and OBJ palettes draw the color image.
#[test]
fn the_objects_line_makes_a_16x16_frame_8x8_objects() {
    let (temp, mut options) = starter("objects-8x8-gbc");
    options.target = BuildTarget::Gbc;
    objects(&temp, "8x8");
    write_png(
        &temp.path("demo/assets/sprites/quad.16x16.png"),
        16,
        16,
        quad,
    );
    let made = built(&options);
    let image = made.images.iter().find(|i| i.label == "Quad").unwrap();
    assert_eq!(image.object_size, Some(ObjectSize::Square));
    assert_eq!((image.frames, image.objects), (Some(1), Some(4)));
    assert_eq!(image.tiles, 4, "four 8x8 objects, a tile each");
    assert_eq!(image.tile_map.len(), 4, "one entry a tile");
    let gbc = fs::read(temp.path("demo/build/images/sprites/quad.gbc.2bpp")).unwrap();
    gbc_draws_the_color_image(&made, image, &gbc, &quad);
    let dmg = fs::read(temp.path("demo/build/images/sprites/quad.2bpp")).unwrap();
    assert_eq!(dmg.len(), 4 * 16);
    // One frame of four objects, y then x, each the tile map's tile and
    // slot.
    let asm = text(&temp, "images.asm");
    let [frame] = &frame_objects(&asm, "Quad")[..] else {
        panic!("{asm}")
    };
    let places: Vec<(u32, u32)> = frame.iter().map(|&(y, x, _, _)| (y, x)).collect();
    assert_eq!(places, [(0, 0), (0, 8), (8, 0), (8, 8)], "{asm}");
    for (o, &(_, _, tile, attr)) in frame.iter().enumerate() {
        let e = image.tile_map[o].unwrap();
        assert_eq!(
            (tile, attr & 7),
            (e.index, e.palette.unwrap().get()),
            "object {o}"
        );
    }
    let inc = text(&temp, "images.inc");
    for line in [
        "; Objects: 8x8 for every sheet",
        "objects line of demo/assets/palettes.txt",
        SAYS_8X8,
        "DEF QUAD_TILES EQU 4 ; tiles, one per object",
    ] {
        assert!(inc.contains(line), "{line:?} in\n{inc}");
    }
}

/// The same sheet plain: the DMG data is four 8x8 objects too.
#[test]
fn a_plain_build_makes_the_same_8x8_objects() {
    let (temp, options) = starter("objects-8x8-plain");
    objects(&temp, "8x8");
    write_png(
        &temp.path("demo/assets/sprites/quad.16x16.png"),
        16,
        16,
        quad,
    );
    let made = built(&options);
    let image = &made.images[0];
    assert_eq!(image.object_size, Some(ObjectSize::Square));
    assert_eq!((image.tiles, image.objects), (4, Some(4)));
    let asm = text(&temp, "images.asm");
    assert!(asm.contains("unique 8x8 objects, 16 bytes each"), "{asm}");
}

/// Without the line, the frame height decides, as before: a 16x16 frame
/// is 8x16 objects and an 8x8 one 8x8, and the include sets no size for
/// the whole project.
#[test]
fn without_the_objects_line_the_height_rule_stands() {
    let (temp, options) = starter("objects-unset");
    pinned(&temp);
    write_png(
        &temp.path("demo/assets/sprites/quad.16x16.png"),
        16,
        16,
        quad,
    );
    write_png(
        &temp.path("demo/assets/sprites/shot.8x8.png"),
        8,
        8,
        |x, _| {
            if x < 2 { BLACK } else { CLEAR }
        },
    );
    let made = built(&options);
    let size = |label: &str| {
        made.images
            .iter()
            .find(|i| i.label == label)
            .unwrap()
            .object_size
    };
    assert_eq!(size("Quad"), Some(ObjectSize::Tall));
    assert_eq!(size("Shot"), Some(ObjectSize::Square));
    let inc = text(&temp, "images.inc");
    assert!(!inc.contains("; Objects:"), "{inc}");
}

/// Without the setting, sheets cut in both sizes are a project warning
/// (not an error: a game may switch LCDC bit 2 between scenes), naming the
/// sheets of each size and offering the setting.
#[test]
fn without_the_setting_mixed_object_sizes_are_a_warning_naming_each_sheet() {
    let (temp, options) = starter("objects-mixed");
    pinned(&temp);
    write_png(
        &temp.path("demo/assets/sprites/quad.16x16.png"),
        16,
        16,
        quad,
    );
    for name in ["shot.8x8.png", "spark.8x8.png"] {
        write_png(
            &temp.path(&format!("demo/assets/sprites/{name}")),
            8,
            8,
            |x, _| if x < 2 { BLACK } else { CLEAR },
        );
    }
    let made = built(&options);
    let mixed: Vec<_> = made
        .problems
        .iter()
        .filter(|p| p.limit == ImageLimit::ObjPair8x16)
        .collect();
    let [p] = &mixed[..] else {
        panic!("{:#?}", made.problems)
    };
    assert_eq!(p.severity, gbcli_protocol::Severity::Warning);
    assert_eq!(p.image, None);
    assert_eq!((p.count, p.max), (2, 1));
    assert_eq!(
        p.message,
        "the sheets are cut in two object sizes, and the screen shows one at a time (LCDC bit \
         2): 8x16 objects in demo/assets/sprites/quad.16x16.png; 8x8 objects in \
         demo/assets/sprites/shot.8x8.png and demo/assets/sprites/spark.8x8.png"
    );
    assert_eq!(
        p.remedy,
        "add `objects: 8x8` (or `objects: 8x16`) to demo/assets/palettes.txt to cut every \
         sheet in one size; or keep both, and switch LCDC bit 2 between scenes that show only \
         one"
    );
    // The setting silences it, and one size alone never raises it.
    objects(&temp, "8x8");
    let made = built(&options);
    assert!(
        made.problems
            .iter()
            .all(|p| p.limit != ImageLimit::ObjPair8x16),
        "{:#?}",
        made.problems
    );
}

/// Under `objects: 8x8` a 48x64 frame needs 48 objects, past OAM's 40,
/// where 8x16 objects would take 24: the remedy offers the setting.
#[test]
fn objects_8x8_past_oams_total_offers_8x16() {
    let (temp, options) = starter("objects-8x8-total");
    objects(&temp, "8x8");
    write_png(
        &temp.path("demo/assets/sprites/big.48x64.png"),
        48,
        64,
        |x, y| if (x + y) % 2 == 0 { BLACK } else { CLEAR },
    );
    let made = refused(&options);
    let p = problem(&made, ImageLimit::ObjTotal);
    assert_eq!(p.count, 48);
    assert!(
        p.remedy.ends_with(
            "; or set the objects line of demo/assets/palettes.txt to 8x16, two tiles an object"
        ),
        "{}",
        p.remedy
    );
}

/// From a manifest the remedy names the manifest's field.
#[test]
fn mixed_object_sizes_from_a_manifest_offer_its_objects_field() {
    let temp = Temp::new("objects-mixed-manifest");
    write_png(&temp.path("art/quad.png"), 16, 16, quad);
    write_png(&temp.path("art/shot.png"), 8, 8, |x, _| {
        if x < 2 { BLACK } else { CLEAR }
    });
    let mut m = manifest(None, "sprites/quad.16x16.png");
    m.images.push(ManifestImage {
        image: "sprites/shot.8x8.png".into(),
        file: "art/shot.png".into(),
        dmg: None,
    });
    let options = AssetsOptions::new(AssetsInput::Manifest(m), temp.0.clone());
    let made = assets(rgbds().images(), &options).unwrap().images;
    let p = made
        .problems
        .iter()
        .find(|p| p.limit == ImageLimit::ObjPair8x16)
        .unwrap_or_else(|| panic!("{:#?}", made.problems));
    assert!(
        p.remedy
            .starts_with("set the manifest's objects to 8x8 (or 8x16)"),
        "{}",
        p.remedy
    );
    assert!(!p.remedy.contains("palettes.txt"), "{}", p.remedy);
}

/// `objects: 8x16` can't cut an 8-high frame: an error naming the line,
/// and no remedy offering the 8x8 frame size the line overrides.
#[test]
fn objects_8x16_refuses_an_8_high_frame_naming_the_setting() {
    let (temp, options) = starter("objects-8x16-short");
    objects(&temp, "8x16");
    write_png(
        &temp.path("demo/assets/sprites/shot.8x8.png"),
        16,
        8,
        |x, _| {
            if x % 8 < 2 { BLACK } else { CLEAR }
        },
    );
    let made = refused(&options);
    let p = problem(&made, ImageLimit::ObjPair8x16);
    let said = format!("{} {}", p.message, p.remedy);
    assert!(
        said.contains("objects line of demo/assets/palettes.txt"),
        "{said}"
    );
    assert!(said.contains("8x16"), "{said}");
}

/// `objects: 8x16` and a sheet 8 high with no frame size: the remedy sets
/// the line to 8x8 rather than offering an 8x8 frame size it overrides.
#[test]
fn objects_8x16_refuses_a_sheet_8_high_naming_the_setting() {
    let (temp, options) = starter("objects-8x16-sheet");
    objects(&temp, "8x16");
    write_png(&temp.path("demo/assets/sprites/dot.png"), 16, 8, |x, _| {
        if x % 8 < 2 { BLACK } else { CLEAR }
    });
    let made = refused(&options);
    let p = problem(&made, ImageLimit::ObjPair8x16);
    assert!(
        p.remedy
            .contains("set the objects line of demo/assets/palettes.txt to 8x8"),
        "{}",
        p.remedy
    );
    assert!(!p.remedy.contains("dot.8x8.png"), "{}", p.remedy);
}

/// `objects: 8x8` takes any frame of whole tiles, but not one 12 high: an
/// error naming the line.
#[test]
fn objects_8x8_refuses_a_frame_of_part_tiles_naming_the_setting() {
    let (temp, options) = starter("objects-8x8-part");
    objects(&temp, "8x8");
    write_png(
        &temp.path("demo/assets/sprites/bat.16x12.png"),
        16,
        24,
        |x, _| {
            if x % 8 < 2 { BLACK } else { CLEAR }
        },
    );
    let made = refused(&options);
    let p = &made.images.as_ref().unwrap().problems[0];
    let said = format!("{} {}", p.message, p.remedy);
    assert!(
        said.contains("objects line of demo/assets/palettes.txt"),
        "{p:#?}"
    );
    assert!(said.contains("8x8"), "{said}");
}

/// A sheet with no frame size, 8 high, builds under `objects: 8x8`.
#[test]
fn objects_8x8_takes_a_sheet_8_high_with_no_frame_size() {
    let (temp, options) = starter("objects-8x8-whole");
    objects(&temp, "8x8");
    write_png(&temp.path("demo/assets/sprites/dot.png"), 16, 8, |x, _| {
        if x % 8 < 2 { BLACK } else { CLEAR }
    });
    let made = built(&options);
    assert_eq!(made.images[0].object_size, Some(ObjectSize::Square));
    assert_eq!(made.images[0].objects, Some(2));
}

/// A manifest of `art/quad.png` as `image`, its shades pinned to the DMG
/// grays as `pinned` pins a project's.
fn manifest(objects: Option<ObjectSize>, image: &str) -> AssetManifest {
    let grays = [0xFF, 0xAA, 0x55, 0x00].map(|v| Rgb555::from_rgb888(v, v, v));
    AssetManifest {
        images: vec![ManifestImage {
            image: image.into(),
            file: "art/quad.png".into(),
            dmg: None,
        }],
        palettes: NamedPalettes {
            dmg: Some(GbcPalette(grays)),
            ..NamedPalettes::default()
        },
        objects,
    }
}

/// A manifest's `objects` field does what the line does, and an error
/// names the field.
#[test]
fn the_setting_works_from_a_manifest() {
    let temp = Temp::new("objects-manifest");
    write_png(&temp.path("art/quad.png"), 16, 16, quad);
    let m = manifest(Some(ObjectSize::Square), "sprites/quad.16x16.png");
    let options = AssetsOptions::new(AssetsInput::Manifest(m), temp.0.clone());
    let made = assets(rgbds().images(), &options).unwrap().images;
    assert!(made.first_error().is_none(), "{:#?}", made.problems);
    assert_eq!(made.images[0].object_size, Some(ObjectSize::Square));
    assert_eq!(made.images[0].objects, Some(4));
    // Unset, the height rule: 8x16.
    let m = manifest(None, "sprites/quad.16x16.png");
    let options = AssetsOptions::new(AssetsInput::Manifest(m), temp.0.clone());
    let made = assets(rgbds().images(), &options).unwrap().images;
    assert_eq!(made.images[0].object_size, Some(ObjectSize::Tall));
    // 8x16 can't cut an 8-high frame.
    let m = manifest(Some(ObjectSize::Tall), "sprites/quad.16x8.png");
    let options = AssetsOptions::new(AssetsInput::Manifest(m), temp.0.clone());
    let made = match assets(rgbds().images(), &options) {
        Err(AssetsError::Failed(m)) => m.images,
        other => panic!("expected a refusal, got {other:?}"),
    };
    let p = made
        .problems
        .iter()
        .find(|p| p.limit == ImageLimit::ObjPair8x16)
        .unwrap_or_else(|| panic!("{:#?}", made.problems));
    let said = format!("{} {}", p.message, p.remedy);
    assert!(said.contains("the manifest's objects"), "{said}");
    assert!(!said.contains("palettes.txt"), "{said}");
}

/// Every PNG under `dir` as its pixel text, as `gbcli pixels` writes it.
fn to_pixels(dir: &Path) {
    for entry in fs::read_dir(dir).unwrap() {
        let path = entry.unwrap().path();
        if path.extension().is_some_and(|e| e == "png") {
            let raster = decode_png(&fs::read(&path).unwrap()).unwrap();
            let name = path.file_name().unwrap().to_string_lossy().into_owned();
            let layout = Layout::Rows {
                frame: frame_size(&name),
            };
            let text = to_text(&from_raster(&raster).unwrap(), layout).unwrap();
            fs::write(path.with_extension("pixels"), text).unwrap();
            fs::remove_file(&path).unwrap();
        }
    }
}

/// A `.pixels` sheet under the line converts as its PNG does.
#[test]
fn a_pixels_sheet_takes_the_setting_as_its_png_does() {
    let mut seen = Vec::new();
    for pixels in [false, true] {
        let (temp, mut options) = starter(&format!("objects-pixels-{pixels}"));
        options.target = BuildTarget::Gbc;
        objects(&temp, "8x8");
        write_png(
            &temp.path("demo/assets/sprites/quad.16x16.png"),
            16,
            16,
            quad,
        );
        if pixels {
            to_pixels(&temp.path("demo/assets/sprites"));
        }
        let made = built(&options);
        let image = &made.images[0];
        let data = fs::read(temp.path("demo/build/images/sprites/quad.2bpp")).unwrap();
        let gbc = fs::read(temp.path("demo/build/images/sprites/quad.gbc.2bpp")).unwrap();
        let asm = text(&temp, "images.asm");
        let lists = frame_objects(&asm, "Quad");
        seen.push((image.object_size, image.tile_map.clone(), data, gbc, lists));
    }
    assert_eq!(seen[0].0, Some(ObjectSize::Square));
    assert_eq!(seen[0], seen[1], "the PNG, then its pixel text");
}
