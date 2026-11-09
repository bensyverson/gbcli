//! Pixel text images (`.pixels`) in a project, beside PNGs: found by the
//! PNG's conventions, converted through a transient PNG, a bad one a
//! `pixels_text` problem that still lets every other image be checked, and
//! two forms of one image a layout error.

use crate::common;
use crate::pngs;

use std::fs;
use std::path::Path;

use gbcli_build::{AssetsInput, AssetsOptions, BuildError, BuildOptions, assets, build};
use gbcli_pixels::{Layout, decode_png, frame_size, from_raster, to_text};
use gbcli_protocol::{BuildTarget, ImageLimit, ImagesMade};

use common::{Temp, rgbds};
use pngs::*;

fn copy_dir(from: &Path, to: &Path) {
    fs::create_dir_all(to).unwrap();
    for entry in fs::read_dir(from).unwrap() {
        let path = entry.unwrap().path();
        let target = to.join(path.file_name().unwrap());
        if path.is_dir() {
            copy_dir(&path, &target);
        } else {
            fs::copy(&path, &target).unwrap();
        }
    }
}

/// Every PNG under `dir` replaced by its canonical pixel text.
fn to_pixels(dir: &Path) {
    for entry in fs::read_dir(dir).unwrap() {
        let path = entry.unwrap().path();
        if path.is_dir() {
            to_pixels(&path);
        } else if path.extension().is_some_and(|e| e == "png") {
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

fn rom(project: &Path, target: BuildTarget) -> Vec<u8> {
    let options = BuildOptions {
        target,
        ..BuildOptions::new(project.to_path_buf(), project.to_path_buf())
    };
    match build(&rgbds(), &options) {
        Ok(_) => {}
        Err(e) => panic!("{e:?}"),
    }
    let name = project.file_name().unwrap().to_string_lossy();
    fs::read(project.join(format!("build/{name}.gb"))).unwrap()
}

#[test]
fn the_racer_built_from_pixel_text_is_byte_identical_plain_and_gbc() {
    let temp = Temp::new("pixels-racer");
    let examples = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../examples");
    let (png, text) = (temp.path("png/racer"), temp.path("text/racer"));
    copy_dir(&examples.join("racer"), &png);
    copy_dir(&examples.join("racer"), &text);
    to_pixels(&text.join("assets"));
    let palettes = text.join("assets/palettes.txt");
    let assigned = fs::read_to_string(&palettes)
        .unwrap()
        .replace(".png =", ".pixels =");
    fs::write(&palettes, assigned).unwrap();
    for target in [BuildTarget::Dmg, BuildTarget::Gbc] {
        let (a, b) = (rom(&png, target), rom(&text, target));
        assert!(a == b, "{target:?}: the ROMs differ");
    }
    assert!(text.join("assets/bg/road.pixels").is_file());
    assert!(
        !text.join("assets/bg/road.png").exists(),
        "nothing is written into assets/"
    );
}

const TYPO: &str = "\
format gbcli-pixels 1
size 8 8
color 0 #ffffff
color 1 #000000
rows
00000000
00000000
000x0000
";

fn demo_assets(temp: &Temp, target: BuildTarget) -> Result<ImagesMade, gbcli_build::AssetsError> {
    let options = AssetsOptions {
        target,
        ..AssetsOptions::new(AssetsInput::Project(temp.path("demo")), temp.0.clone())
    };
    assets(rgbds().images(), &options).map(|m| m.images)
}

#[test]
fn a_bad_pixel_text_image_is_a_problem_at_its_line_and_the_others_are_still_checked() {
    let temp = Temp::new("pixels-typo");
    fs::create_dir_all(temp.path("demo/assets/bg")).unwrap();
    fs::write(temp.path("demo/assets/bg/a.pixels"), TYPO).unwrap();
    write_png(&temp.path("demo/assets/bg/b.png"), 12, 8, |_, _| WHITE);
    let made = match demo_assets(&temp, BuildTarget::Dmg) {
        Err(gbcli_build::AssetsError::Failed(made)) => made.images,
        other => panic!("expected problems, got {other:?}"),
    };
    let limits: Vec<ImageLimit> = made.problems.iter().map(|p| p.limit).collect();
    assert_eq!(limits, [ImageLimit::PixelsText, ImageLimit::SizeMultiple8]);
    let p = &made.problems[0];
    assert_eq!(p.image.as_deref(), Some("demo/assets/bg/a.pixels"));
    assert_eq!((p.line, p.column.map(u32::from)), (Some(8), Some(4)));
    assert_eq!(p.message, "x isn't in the legend: it has 0 and 1");
    assert_eq!(p.remedy, "add `color x VALUE`, or use a legend character");
}

#[test]
fn a_png_and_pixel_text_of_one_name_is_a_layout_error() {
    let temp = Temp::new("pixels-both");
    fs::create_dir_all(temp.path("demo/assets/bg")).unwrap();
    write_png(&temp.path("demo/assets/bg/town.png"), 8, 8, |_, _| WHITE);
    fs::write(
        temp.path("demo/assets/bg/town.pixels"),
        "format gbcli-pixels 1\nsize 8 8\ncolor 0 #fff\nfill 0\n",
    )
    .unwrap();
    let e = demo_assets(&temp, BuildTarget::Dmg).unwrap_err();
    let gbcli_build::AssetsError::Build(BuildError::Layout { message, remedy }) = e else {
        panic!("{e:?}")
    };
    assert_eq!(
        message,
        "demo/assets/bg/town.png and demo/assets/bg/town.pixels are two forms of one image"
    );
    assert_eq!(remedy, "keep one: a PNG from any tool, or pixel text");
}

#[test]
fn a_hardware_problem_in_pixel_text_names_the_line_that_drew_the_tile() {
    let temp = Temp::new("pixels-line");
    fs::create_dir_all(temp.path("demo/assets/bg")).unwrap();
    // Five colors in the second tile, which the rows block on line 12 drew.
    let text = "\
format gbcli-pixels 1
size 16 8
color 0 #ffffff
color 1 #ff0000
color 2 #00ff00
color 3 #0000ff
color 4 #000000
fill 0

rows 8 0
01234000
";
    fs::write(temp.path("demo/assets/bg/five.pixels"), text).unwrap();
    let made = match demo_assets(&temp, BuildTarget::Gbc) {
        Err(gbcli_build::AssetsError::Failed(made)) => made.images,
        other => panic!("expected problems, got {other:?}"),
    };
    let p = made
        .problems
        .iter()
        .find(|p| p.limit == ImageLimit::GbcTileColors)
        .unwrap_or_else(|| panic!("{:#?}", made.problems));
    assert_eq!(p.tiles[0], gbcli_protocol::TileAt { x: 1, y: 0 });
    assert_eq!(p.line, Some(10));
}

#[test]
fn a_dmg_version_and_an_assignment_may_name_pixel_text() {
    let temp = Temp::new("pixels-dmg");
    fs::create_dir_all(temp.path("demo/assets/bg")).unwrap();
    write_png(&temp.path("demo/assets/bg/town.png"), 8, 8, |x, _| {
        if x < 4 {
            [0xFF, 0, 0, 0xFF]
        } else {
            [0, 0, 0xFF, 0xFF]
        }
    });
    let dmg = "format gbcli-pixels 1\nsize 8 8\ncolor 0 dmg 0\ncolor 3 dmg 3\nrows\n00003333\n\
               00003333\n00003333\n00003333\n00003333\n00003333\n00003333\n00003333\n";
    fs::write(temp.path("demo/assets/bg/town.dmg.pixels"), dmg).unwrap();
    fs::write(
        temp.path("demo/assets/palettes.txt"),
        "dmg: #fff #aaa #555 #000\nsea: #fff #aaa #555 #000\nbg/sky.pixels = sea\n",
    )
    .unwrap();
    fs::write(
        temp.path("demo/assets/bg/sky.pixels"),
        "format gbcli-pixels 1\nsize 8 8\ncolor 0 sea 1\nfill 0\n",
    )
    .unwrap();
    let made = demo_assets(&temp, BuildTarget::Gbc).unwrap_or_else(|e| panic!("{e:?}"));
    let names: Vec<&str> = made.images.iter().map(|i| i.source.as_str()).collect();
    assert_eq!(
        names,
        ["demo/assets/bg/sky.pixels", "demo/assets/bg/town.png"]
    );
    // An assignment naming the other form says which file there is.
    fs::write(
        temp.path("demo/assets/palettes.txt"),
        "dmg: #fff #aaa #555 #000\nsea: #fff #aaa #555 #000\nbg/sky.png = sea\n",
    )
    .unwrap();
    let made = match demo_assets(&temp, BuildTarget::Dmg) {
        Err(gbcli_build::AssetsError::Failed(made)) => made.images,
        other => panic!("expected a palette_file problem, got {other:?}"),
    };
    let p = &made.problems[0];
    assert_eq!(p.limit, ImageLimit::PaletteFile);
    assert!(p.remedy.contains("bg/sky.pixels"), "{}", p.remedy);
}

const WHITE_TEXT: &str = "format gbcli-pixels 1\nsize 8 8\ncolor 0 #fff\nfill 0\n";

fn layout_error(temp: &Temp) -> (String, String) {
    match demo_assets(temp, BuildTarget::Dmg) {
        Err(gbcli_build::AssetsError::Build(BuildError::Layout { message, remedy })) => {
            (message, remedy)
        }
        other => panic!("expected a layout error, got {other:?}"),
    }
}

/// A hardware problem names the line that drew the tile's colors: here
/// the rows block on line 10 drew all five colors of tile 1,0, but not its
/// top-left pixel, which `fill` (line 8) drew.
#[test]
fn a_hardware_problem_names_the_block_that_drew_the_colors_not_the_fill_under_its_corner() {
    let temp = Temp::new("pixels-corner");
    fs::create_dir_all(temp.path("demo/assets/bg")).unwrap();
    let text = "\
format gbcli-pixels 1
size 16 8
color 0 #ffffff
color 1 #ff0000
color 2 #00ff00
color 3 #0000ff
color 4 #000000
fill 0

rows 9 0
1234000
";
    fs::write(temp.path("demo/assets/bg/five.pixels"), text).unwrap();
    let made = match demo_assets(&temp, BuildTarget::Gbc) {
        Err(gbcli_build::AssetsError::Failed(made)) => made.images,
        other => panic!("expected problems, got {other:?}"),
    };
    let p = made
        .problems
        .iter()
        .find(|p| p.limit == ImageLimit::GbcTileColors)
        .unwrap_or_else(|| panic!("{:#?}", made.problems));
    assert_eq!(p.tiles[0], gbcli_protocol::TileAt { x: 1, y: 0 });
    assert_eq!(p.line, Some(10));
}

/// `.PIXELS` is found as `.PNG` is, in any case, and is still one of two
/// forms beside a PNG of its name.
#[test]
fn an_upper_case_pixels_extension_is_found_like_an_upper_case_png() {
    let temp = Temp::new("pixels-case");
    fs::create_dir_all(temp.path("demo/assets/bg")).unwrap();
    write_png(&temp.path("demo/assets/bg/a.PNG"), 8, 8, |_, _| WHITE);
    fs::write(temp.path("demo/assets/bg/b.PIXELS"), WHITE_TEXT).unwrap();
    let made = demo_assets(&temp, BuildTarget::Dmg).unwrap_or_else(|e| panic!("{e:?}"));
    let names: Vec<&str> = made.images.iter().map(|i| i.source.as_str()).collect();
    assert_eq!(names, ["demo/assets/bg/a.PNG", "demo/assets/bg/b.PIXELS"]);
    fs::write(temp.path("demo/assets/bg/a.Pixels"), WHITE_TEXT).unwrap();
    let (message, _) = layout_error(&temp);
    assert_eq!(
        message,
        "demo/assets/bg/a.PNG and demo/assets/bg/a.Pixels are two forms of one image"
    );
}

/// A `.DMG.PIXELS` beside its color image is its DMG version, as
/// `.DMG.PNG` is, and an assignment may name `.PIXELS`.
#[test]
fn an_upper_case_dmg_version_and_assignment_of_pixel_text_are_read() {
    let temp = Temp::new("pixels-case-dmg");
    fs::create_dir_all(temp.path("demo/assets/bg")).unwrap();
    fs::write(temp.path("demo/assets/bg/town.PIXELS"), WHITE_TEXT).unwrap();
    fs::write(temp.path("demo/assets/bg/town.DMG.PIXELS"), WHITE_TEXT).unwrap();
    fs::write(temp.path("demo/assets/bg/sky.PIXELS"), WHITE_TEXT).unwrap();
    fs::write(
        temp.path("demo/assets/palettes.txt"),
        "sea: #fff #aaa #555 #000\nbg/sky.PIXELS = sea\n",
    )
    .unwrap();
    let made = demo_assets(&temp, BuildTarget::Dmg).unwrap_or_else(|e| panic!("{e:?}"));
    let names: Vec<&str> = made.images.iter().map(|i| i.source.as_str()).collect();
    assert_eq!(
        names,
        ["demo/assets/bg/sky.PIXELS", "demo/assets/bg/town.PIXELS"]
    );
}

/// An orphan DMG version in pixel text names the color image in both
/// forms, since either serves.
#[test]
fn an_orphan_dmg_version_names_either_form_of_its_color_image() {
    let temp = Temp::new("pixels-orphan");
    fs::create_dir_all(temp.path("demo/assets/bg")).unwrap();
    fs::write(temp.path("demo/assets/bg/town.dmg.pixels"), WHITE_TEXT).unwrap();
    let (message, _) = layout_error(&temp);
    assert_eq!(
        message,
        "demo/assets/bg/town.dmg.pixels is a DMG version, but there is no color image \
         demo/assets/bg/town.png or demo/assets/bg/town.pixels beside it"
    );
}

/// A rename a remedy offers keeps the form the file is in.
#[test]
fn a_name_remedy_for_pixel_text_offers_a_pixels_name() {
    let temp = Temp::new("pixels-rename");
    fs::create_dir_all(temp.path("demo/assets/sprites")).unwrap();
    fs::write(temp.path("demo/assets/sprites/hero.big.pixels"), WHITE_TEXT).unwrap();
    let (_, remedy) = layout_error(&temp);
    assert_eq!(
        remedy,
        "rename it to hero.WxH.pixels, e.g. hero.16x16.pixels, or drop the size to make the \
         whole sheet one frame"
    );
    fs::remove_file(temp.path("demo/assets/sprites/hero.big.pixels")).unwrap();
    fs::write(temp.path("demo/assets/sprites/2hero.pixels"), WHITE_TEXT).unwrap();
    let (_, remedy) = layout_error(&temp);
    assert_eq!(remedy, "rename it, e.g. to hero.pixels");
}

/// A sheet's hardware remedies offer a rename in the sheet's own form.
#[test]
fn a_pixel_text_sheets_frame_remedy_offers_a_pixels_name() {
    let temp = Temp::new("pixels-frame-rename");
    fs::create_dir_all(temp.path("demo/assets/sprites")).unwrap();
    // 24 wide is not whole 16-wide frames.
    let text = "format gbcli-pixels 1\nsize 24 16\ncolor . clear\ncolor 0 #000\nfill .\n\n\
                rows\n0\n";
    fs::write(temp.path("demo/assets/sprites/hero.16x16.pixels"), text).unwrap();
    let made = match demo_assets(&temp, BuildTarget::Dmg) {
        Err(gbcli_build::AssetsError::Failed(made)) => made.images,
        other => panic!("expected problems, got {other:?}"),
    };
    let p = made
        .problems
        .iter()
        .find(|p| p.limit == ImageLimit::FrameGrid)
        .unwrap_or_else(|| panic!("{:#?}", made.problems));
    assert!(p.remedy.contains("hero.8x16.pixels"), "{}", p.remedy);
    assert!(!p.remedy.contains(".png"), "{}", p.remedy);
}

/// A malformed palette file stops the conversion, but pixel text that
/// doesn't parse is still reported beside it.
#[test]
fn a_bad_palette_file_keeps_the_pixel_text_problems() {
    let temp = Temp::new("pixels-bad-palettes");
    fs::create_dir_all(temp.path("demo/assets/bg")).unwrap();
    fs::write(temp.path("demo/assets/bg/a.pixels"), TYPO).unwrap();
    fs::write(
        temp.path("demo/assets/palettes.txt"),
        "this is not a palette\n",
    )
    .unwrap();
    let made = match demo_assets(&temp, BuildTarget::Dmg) {
        Err(gbcli_build::AssetsError::Failed(made)) => made.images,
        other => panic!("expected problems, got {other:?}"),
    };
    let limits: Vec<ImageLimit> = made.problems.iter().map(|p| p.limit).collect();
    assert_eq!(limits, [ImageLimit::PaletteFile, ImageLimit::PixelsText]);
}
