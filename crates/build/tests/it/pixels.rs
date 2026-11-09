//! `pixels`: an image checked and converted, its palette references
//! resolved through the project's `assets/palettes.txt` or `--palettes`.

use std::fs;

use gbcli_build::pixels;
use gbcli_pixels::{EXAMPLE, Layout, Palettes, decode_png, encode_png, parse_text, to_text};
use gbcli_protocol::{
    ErrorBody, ErrorCode, PixelsArgs, PixelsForm, PixelsMade, PixelsOutputKind, PixelsProblem,
};

use crate::common::Temp;

fn args(file: &str) -> PixelsArgs {
    PixelsArgs {
        file: file.into(),
        ..PixelsArgs::default()
    }
}

fn problem(body: &ErrorBody) -> &PixelsProblem {
    body.result
        .as_ref()
        .and_then(|r| r.pixels_problem.as_ref())
        .unwrap_or_else(|| panic!("no problem row: {body:?}"))
}

const MEADOW: &str = "\
format gbcli-pixels 1
size 8 8
color 0 dmg 0
color 3 dmg 3
rows
00000000
00000000
00000000
00000000
33333333
33333333
33333333
33333333
";

#[test]
fn a_sheet_is_checked_and_counted_with_nothing_written() {
    let temp = Temp::new("pixels-check");
    fs::write(temp.path("hero.16x16.pixels"), EXAMPLE).unwrap();
    let made = pixels(&temp.0, &args("hero.16x16.pixels")).unwrap();
    assert_eq!(
        made,
        PixelsMade {
            path: "hero.16x16.pixels".into(),
            form: PixelsForm::Text,
            width: 32,
            height: 16,
            colors: 3,
            clear: true,
            frames: Some(2),
            tiles: Some(6),
            wrote: vec![],
        }
    );
}

#[test]
fn text_png_and_preview_are_written_and_the_png_reads_back_the_same() {
    let temp = Temp::new("pixels-convert");
    fs::write(temp.path("hero.16x16.pixels"), EXAMPLE).unwrap();
    let made = pixels(
        &temp.0,
        &PixelsArgs {
            text: Some("canon.pixels".into()),
            png: Some("hero.png".into()),
            preview: Some("big.png".into()),
            ..args("hero.16x16.pixels")
        },
    )
    .unwrap();
    let kinds: Vec<(PixelsOutputKind, &str)> = made
        .wrote
        .iter()
        .map(|o| (o.kind, o.path.as_str()))
        .collect();
    assert_eq!(
        kinds,
        [
            (PixelsOutputKind::Text, "canon.pixels"),
            (PixelsOutputKind::Png, "hero.png"),
            (PixelsOutputKind::Preview, "big.png"),
        ]
    );
    let picture = parse_text(EXAMPLE).unwrap();
    let text = fs::read_to_string(temp.path("canon.pixels")).unwrap();
    let rows = Layout::Rows {
        frame: Some((16, 16)),
    };
    assert_eq!(text, to_text(&picture, rows).unwrap());
    let raster = picture.raster(&Palettes::default()).unwrap();
    let png = fs::read(temp.path("hero.png")).unwrap();
    assert_eq!(png, encode_png(&raster));
    assert_eq!(made.wrote[1].bytes, png.len() as u64);
    let preview = decode_png(&fs::read(temp.path("big.png")).unwrap()).unwrap();
    assert_eq!((preview.width, preview.height), (128, 64));
    // And back: the PNG reads as a PNG with the same counts.
    let again = pixels(&temp.0, &args("hero.png")).unwrap();
    assert_eq!(again.form, PixelsForm::Png);
    assert_eq!((again.colors, again.clear, again.tiles), (3, true, Some(6)));
    assert_eq!(again.frames, None, "hero.png names no frame size");
}

#[test]
fn tiles_write_tiles_and_a_map_and_need_text_and_whole_tiles() {
    let temp = Temp::new("pixels-tiles");
    fs::write(temp.path("hero.pixels"), EXAMPLE).unwrap();
    let made = pixels(
        &temp.0,
        &PixelsArgs {
            text: Some("t.pixels".into()),
            tiles: true,
            ..args("hero.pixels")
        },
    )
    .unwrap();
    assert_eq!(made.wrote.len(), 1);
    let text = fs::read_to_string(temp.path("t.pixels")).unwrap();
    assert!(
        text.contains("\nmap 0 0\n00 01 00 01\n02 03 04 05\n"),
        "{text}"
    );
    let body = pixels(
        &temp.0,
        &PixelsArgs {
            tiles: true,
            ..args("hero.pixels")
        },
    )
    .unwrap_err();
    assert_eq!(body.code, ErrorCode::BadRequest);
    assert_eq!(body.remedy, "add --text PATH, or drop --tiles");
    fs::write(
        temp.path("odd.pixels"),
        "format gbcli-pixels 1\nsize 4 4\ncolor x #000\nfill x\n",
    )
    .unwrap();
    let body = pixels(
        &temp.0,
        &PixelsArgs {
            text: Some("o.pixels".into()),
            tiles: true,
            ..args("odd.pixels")
        },
    )
    .unwrap_err();
    assert_eq!(body.code, ErrorCode::BadRequest);
    assert_eq!(body.remedy, "write it as rows: drop --tiles");
    assert!(!temp.path("o.pixels").exists());
}

#[test]
fn dmg_shades_resolve_through_the_project_the_image_sits_in_or_the_flag() {
    let temp = Temp::new("pixels-palettes");
    fs::create_dir_all(temp.path("game/assets/bg")).unwrap();
    fs::write(temp.path("game/assets/bg/m.pixels"), MEADOW).unwrap();
    fs::write(
        temp.path("game/assets/palettes.txt"),
        "dmg: #e0f8d0 #88c070 #346856 #081820\n",
    )
    .unwrap();
    let to_png = |palettes: Option<&str>| PixelsArgs {
        png: Some("m.png".into()),
        palettes: palettes.map(String::from),
        ..args("game/assets/bg/m.pixels")
    };
    pixels(&temp.0, &to_png(None)).unwrap();
    let png = decode_png(&fs::read(temp.path("m.png")).unwrap()).unwrap();
    // #e0f8d0 as RGB555 and back, as the build reads the palette file.
    assert_eq!(png.at(0, 0), [0xE7, 0xFF, 0xD6, 0xFF]);
    assert_eq!(png.at(0, 7), [0x08, 0x18, 0x21, 0xFF]);
    fs::write(temp.path("grays.txt"), "dmg: #fff #aaa #555 #000\n").unwrap();
    pixels(&temp.0, &to_png(Some("grays.txt"))).unwrap();
    let png = decode_png(&fs::read(temp.path("m.png")).unwrap()).unwrap();
    assert_eq!(png.at(0, 0), [0xFF, 0xFF, 0xFF, 0xFF]);
    // With no dmg line, the check itself refuses, at the value.
    fs::write(
        temp.path("game/assets/palettes.txt"),
        "sea: #fff #aaa #555 #000\n",
    )
    .unwrap();
    let body = pixels(&temp.0, &args("game/assets/bg/m.pixels")).unwrap_err();
    assert_eq!(body.code, ErrorCode::PixelsUnreadable);
    let p = problem(&body);
    assert_eq!(
        (p.path.as_str(), p.line, p.column),
        ("game/assets/bg/m.pixels", Some(3), Some(9))
    );
    assert!(p.remedy.contains("--palettes"), "{}", p.remedy);
    assert_eq!(
        body.message,
        format!("game/assets/bg/m.pixels:3:9: {}", p.message)
    );
}

#[test]
fn a_broken_palette_file_is_the_problem_named_at_its_line() {
    let temp = Temp::new("pixels-bad-palettes");
    fs::write(temp.path("m.pixels"), MEADOW).unwrap();
    fs::write(temp.path("p.txt"), "dmg: #fff #aaa #555\n").unwrap();
    let body = pixels(
        &temp.0,
        &PixelsArgs {
            palettes: Some("p.txt".into()),
            ..args("m.pixels")
        },
    )
    .unwrap_err();
    assert_eq!(body.code, ErrorCode::PixelsUnreadable);
    let p = problem(&body);
    assert_eq!(
        (p.path.as_str(), p.line, p.column),
        ("p.txt", Some(1), None)
    );
    assert_eq!(
        p.message,
        "dmg lists 3 colors; a palette has 4, index 0 first"
    );
    let missing = pixels(
        &temp.0,
        &PixelsArgs {
            palettes: Some("nope.txt".into()),
            ..args("m.pixels")
        },
    )
    .unwrap_err();
    assert_eq!(missing.code, ErrorCode::PixelsUnreadable);
    assert!(
        missing.message.starts_with("nope.txt: "),
        "{}",
        missing.message
    );
    assert_eq!(
        missing.remedy,
        "check the path; --palettes names a palette file like assets/palettes.txt"
    );
}

#[test]
fn a_file_that_doesnt_read_writes_nothing_and_says_where() {
    let temp = Temp::new("pixels-unreadable");
    let typo = EXAMPLE.replacen("....kssssssk....", "....kssssxsk....", 1);
    fs::write(temp.path("hero.pixels"), typo).unwrap();
    let body = pixels(
        &temp.0,
        &PixelsArgs {
            png: Some("out.png".into()),
            ..args("hero.pixels")
        },
    )
    .unwrap_err();
    assert_eq!(body.code, ErrorCode::PixelsUnreadable);
    assert_eq!(body.exit, 4);
    let p = problem(&body);
    assert_eq!((p.line, p.column), (Some(13), Some(10)));
    assert_eq!(p.message, "x isn't in the legend: it has ., k, s and r");
    assert!(!temp.path("out.png").exists());
    let missing = pixels(&temp.0, &args("none.pixels")).unwrap_err();
    assert_eq!(missing.code, ErrorCode::PixelsUnreadable);
    assert_eq!(
        missing.remedy,
        "check the path; an image file is a PNG or pixel text (gbcli help pixels)"
    );
    // A PNG's problem is located to its pixel.
    let half = gbcli_pixels::Raster {
        width: 2,
        height: 1,
        rgba: vec![[0, 0, 0, 255], [0, 0, 0, 7]],
    };
    fs::write(temp.path("half.png"), encode_png(&half)).unwrap();
    let body = pixels(&temp.0, &args("half.png")).unwrap_err();
    let p = problem(&body);
    assert_eq!((p.x, p.y, p.line), (Some(1), Some(0), None));
    assert_eq!(body.message, format!("half.png: pixel 1,0: {}", p.message));
}

#[test]
fn an_output_that_cant_be_written_is_io_failed() {
    let temp = Temp::new("pixels-io");
    fs::write(temp.path("hero.pixels"), EXAMPLE).unwrap();
    let body = pixels(
        &temp.0,
        &PixelsArgs {
            png: Some("no/such/dir/x.png".into()),
            ..args("hero.pixels")
        },
    )
    .unwrap_err();
    assert_eq!(body.code, ErrorCode::IoFailed);
    assert!(
        body.message.starts_with("writing no/such/dir/x.png: "),
        "{}",
        body.message
    );
}
