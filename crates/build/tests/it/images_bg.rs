//! `build` over backgrounds in `assets/bg/`: converted by rgbgfx into unique
//! tiles and a tilemap the program includes, previewed, and refused with a
//! structured problem at each DMG background limit.

use crate::common;
use crate::pngs;

use std::fs;

use gbcli_build::{BuildError, build};
use gbcli_protocol::{ErrorCode, ImageKind, ImageLimit, Severity, TileAt};

use common::{rgbds, starter};
use pngs::*;

/// A 3x2-tile sign: three tiles, one used three times, in all four shades.
fn sign(x: u32, y: u32) -> Rgba {
    match (x / 8, y / 8) {
        (0, 0) => BLACK,
        (1, 0) => [WHITE, LIGHT, DARK, BLACK][(x % 4) as usize],
        (2, 0) => LIGHT,
        _ => WHITE,
    }
}

#[test]
fn a_background_becomes_tiles_a_tilemap_and_an_include_with_a_preview() {
    let (temp, options) = starter("bg-converts");
    write_png(&temp.path("demo/assets/bg/sign.png"), 24, 16, sign);
    let made = build(&rgbds(), &options).unwrap();
    let images = made.images.expect("the build reports its images");
    assert!(images.problems.is_empty(), "{:?}", images.problems);
    assert_eq!(
        images.include.as_deref(),
        Some("demo/build/images/images.inc")
    );
    let [image] = &images.images[..] else {
        panic!("one image: {images:?}")
    };
    assert_eq!(image.kind, ImageKind::Bg);
    assert_eq!(image.source, "demo/assets/bg/sign.png");
    assert_eq!(image.label, "Sign");
    assert_eq!((image.width, image.height, image.tiles), (3, 2, 4));
    assert_eq!(image.preview, "demo/build/images/bg/sign.preview.png");

    // The data, as the generated assembly includes it: four unique tiles,
    // and the map names the white tile three times.
    let tiles = fs::read(temp.path("demo/build/images/bg/sign.2bpp")).unwrap();
    assert_eq!(tiles.len(), 4 * 16);
    let map = fs::read(temp.path("demo/build/images/bg/sign.tilemap")).unwrap();
    assert_eq!(map.len(), 6);
    assert_eq!(map[3], map[4]);
    assert_eq!(map[4], map[5]);
    // Shade = color index (BGP $E4): the black tile is index 3 throughout.
    let black = &tiles[usize::from(map[0]) * 16..][..16];
    assert!(black.iter().all(|&b| b == 0xFF), "{black:02x?}");

    let include = fs::read_to_string(temp.path("demo/build/images/images.inc")).unwrap();
    for line in [
        "DEF SIGN_WIDTH EQU 3",
        "DEF SIGN_HEIGHT EQU 2",
        "DEF SIGN_TILES EQU 4",
        "DEF SIGN_BGP EQU %11_10_01_00",
    ] {
        assert!(include.contains(line), "{line} in\n{include}");
    }
    let asm = fs::read_to_string(temp.path("demo/build/images/images.asm")).unwrap();
    for line in [
        "SignTiles::",
        "INCBIN \"build/images/bg/sign.2bpp\"",
        "SignTilesEnd::",
        "SignTilemap::",
        "SignTilemapEnd::",
    ] {
        assert!(asm.contains(line), "{line} in\n{asm}");
    }
    // Assembled with the sources (the starter's songs may follow it).
    assert!(
        made.sources
            .iter()
            .any(|s| s == "demo/build/images/images.asm"),
        "{:?}",
        made.sources
    );
    let sym = fs::read_to_string(temp.path("demo/build/demo.sym")).unwrap();
    assert!(sym.lines().any(|l| l.ends_with(" SignTilemap")), "{sym}");

    // The preview draws the converted data: its top-left is the sign as the
    // hardware shows it, at its scale.
    let (width, height, pixels) = read_png(&temp.path(&image.preview));
    let scale = height / 16;
    assert!(scale >= 1 && width > 24 * scale, "{width}x{height}");
    for y in 0..16 {
        for x in 0..24 {
            let [r, g, b, _] = sign(x, y);
            let want = (u32::from(r) << 16) | (u32::from(g) << 8) | u32::from(b);
            let got = pixels[((y * scale) * width + x * scale) as usize];
            assert_eq!(got, want, "preview pixel ({x}, {y})");
        }
    }
}

#[test]
fn a_project_without_images_reports_none_and_builds_as_before() {
    let (temp, options) = starter("bg-none");
    crate::common::without_text(&temp.path("demo"));
    let made = build(&rgbds(), &options).unwrap();
    assert_eq!(made.images, None);
    assert!(
        !made.sources.iter().any(|s| s.contains("build/images/")),
        "{:?}",
        made.sources
    );
}

/// The classic greens are a known palette (Ben's second ruling,
/// 2026-11-07), so they take the four shades exactly. By plain lightness
/// (0.66, 0.60, 0.33, 0.17) they would crowd into shades 1 and 2.
#[test]
fn the_classic_greens_take_the_four_shades_as_a_known_palette() {
    let (temp, options) = starter("bg-green");
    // The classic green palette, lightest to darkest across one tile.
    let greens: [Rgba; 4] = [
        [0x9B, 0xBC, 0x0F, 0xFF],
        [0x8B, 0xAC, 0x0F, 0xFF],
        [0x30, 0x62, 0x30, 0xFF],
        [0x0F, 0x38, 0x0F, 0xFF],
    ];
    write_png(&temp.path("demo/assets/bg/field.png"), 8, 8, |x, _| {
        greens[(x / 2) as usize]
    });
    let made = build(&rgbds(), &options).unwrap();
    let tiles = fs::read(temp.path("demo/build/images/bg/field.2bpp")).unwrap();
    // Every row: pixels 0-1 index 0, 2-3 index 1, 4-5 index 2, 6-7 index 3.
    assert_eq!(&tiles[..2], &[0b0011_0011, 0b0000_1111]);
    let shared = problems(&made, ImageLimit::SharedShade);
    assert!(shared.is_empty(), "{shared:#?}");
}

#[test]
fn an_over_budget_background_is_bg_tiles_map_naming_the_tiles_used_once() {
    let (temp, options) = starter("bg-over");
    // 20x15 tiles, every one different except the bottom row, all blank.
    write_png(&temp.path("demo/assets/bg/town.png"), 160, 120, |x, y| {
        let (tx, ty) = (x / 8, y / 8);
        let n = if ty == 14 { 0 } else { 1 + ty * 20 + tx };
        numbered_tile(n, x % 8, y % 8)
    });
    let made = refused(&options);
    assert_eq!(errors(&made), [ImageLimit::BgTilesMap]);
    let p = problem(&made, ImageLimit::BgTilesMap);
    assert_eq!(p.image.as_deref(), Some("demo/assets/bg/town.png"));
    assert_eq!((p.count, p.max), (281, 256));
    assert_eq!(p.tiles.len(), 280, "every tile used once");
    assert_eq!(p.tiles[0], TileAt { x: 0, y: 0 });
    assert!(
        !p.tiles.contains(&TileAt { x: 0, y: 14 }),
        "blank is reused"
    );
    assert!(p.message.contains("281 unique tiles"), "{}", p.message);
    assert!(p.message.contains("280"), "{}", p.message);
    assert!(made.outputs.is_empty());
    assert!(!temp.path("demo/build/demo.gb").exists());

    // The error the caller gets names the image, the limit and the remedy.
    let body = BuildError::Failed {
        made: Box::new(made),
        again: "gbcli build demo".into(),
    }
    .body();
    assert_eq!(body.code, ErrorCode::BuildFailed);
    assert!(
        body.message.contains("demo/assets/bg/town.png"),
        "{}",
        body.message
    );
    assert!(body.message.contains("bg_tiles_map"), "{}", body.message);
    assert_eq!(body.remedy, p.remedy);
}

#[test]
fn an_indexed_background_of_more_than_four_colors_is_dmg_palette_naming_the_rarest() {
    let (temp, options) = starter("bg-colors");
    let red: Rgba = [0xFF, 0x00, 0x00, 0xFF];
    write_png(&temp.path("demo/assets/bg/sky.png"), 16, 8, |x, y| {
        match (x, y) {
            (9, 1) => red,
            _ => [WHITE, LIGHT, DARK, BLACK][(x % 4) as usize],
        }
    });
    // Color art would derive its DMG version instead.
    indexed(&temp.path("demo"), &["bg/sky.png"]);
    let made = refused(&options);
    let p = problem(&made, ImageLimit::DmgPalette);
    assert_eq!((p.count, p.max), (5, 4));
    assert_eq!(p.tiles, [TileAt { x: 1, y: 0 }]);
    assert!(p.message.contains("#ff0000"), "{}", p.message);
}

#[test]
fn a_size_off_the_tile_grid_and_a_transparent_background_are_refused() {
    let (temp, options) = starter("bg-size");
    write_png(&temp.path("demo/assets/bg/narrow.png"), 12, 8, |_, _| WHITE);
    write_png(&temp.path("demo/assets/bg/holes.png"), 8, 8, |x, _| {
        if x == 0 { CLEAR } else { WHITE }
    });
    let made = refused(&options);
    let p = problem(&made, ImageLimit::SizeMultiple8);
    assert_eq!(p.image.as_deref(), Some("demo/assets/bg/narrow.png"));
    assert!(p.message.contains("12x8"), "{}", p.message);
    let p = problem(&made, ImageLimit::Alpha);
    assert_eq!(p.image.as_deref(), Some("demo/assets/bg/holes.png"));
    assert_eq!(p.severity, Severity::Error);
    assert_eq!(p.tiles, [TileAt { x: 0, y: 0 }]);
}

#[test]
fn an_image_name_that_cant_be_a_label_is_refused_with_the_name_to_use() {
    let (temp, options) = starter("bg-name");
    write_png(&temp.path("demo/assets/bg/2 town.png"), 8, 8, |_, _| WHITE);
    let err = build(&rgbds(), &options).unwrap_err();
    let body = err.body();
    assert_eq!(body.code, ErrorCode::BadRequest);
    assert!(body.message.contains("2 town.png"), "{}", body.message);
    assert!(body.remedy.contains("town.png"), "{}", body.remedy);
}

/// Fake rgbasm, rgblink and rgbfix that run the real ones, and an `rgbgfx`
/// running `body` (or none).
fn tools_with(temp: &common::Temp, rgbgfx: Option<&str>) -> gbcli_build::Toolchain {
    use gbcli_protocol::Tool;
    let mut scripts = vec![
        ("rgbasm", common::real(Tool::Rgbasm, "")),
        ("rgblink", common::real(Tool::Rgblink, "")),
        ("rgbfix", common::real(Tool::Rgbfix, "")),
    ];
    if let Some(body) = rgbgfx {
        scripts.push(("rgbgfx", body.to_string()));
    }
    let scripts: Vec<(&str, &str)> = scripts.iter().map(|(n, b)| (*n, b.as_str())).collect();
    let path = common::fake_tools(temp, &scripts);
    gbcli_build::Toolchain::discover(Some(&path), &temp.0).unwrap()
}

#[test]
fn without_rgbgfx_a_project_with_images_is_toolchain_missing_and_one_without_builds() {
    let (temp, options) = starter("bg-no-rgbgfx");
    let tools = tools_with(&temp, None);
    build(&tools, &options).expect("no images, no rgbgfx needed");
    write_png(&temp.path("demo/assets/bg/sign.png"), 8, 8, |_, _| WHITE);
    let body = build(&tools, &options).unwrap_err().body();
    assert_eq!(body.code, ErrorCode::ToolchainMissing);
    assert!(
        body.message.contains("rgbgfx is not on PATH"),
        "{}",
        body.message
    );
    assert!(
        body.remedy.contains("releases/tag/v1.0.4"),
        "{}",
        body.remedy
    );
}

#[test]
fn an_rgbgfx_failure_is_its_diagnostic_on_the_image_and_stops_the_build() {
    let (temp, options) = starter("bg-rgbgfx-fails");
    let tools = tools_with(&temp, Some("echo 'FATAL: out of cheese' >&2; exit 1"));
    write_png(&temp.path("demo/assets/bg/sign.png"), 8, 8, |_, _| WHITE);
    let made = match build(&tools, &options) {
        Err(BuildError::Failed { made, .. }) => *made,
        other => panic!("expected rgbgfx to stop the build, got {other:?}"),
    };
    let d = &made.diagnostics[0];
    assert_eq!(d.tool, gbcli_protocol::Tool::Rgbgfx);
    assert_eq!(d.severity, Severity::Error);
    assert_eq!(d.file.as_deref(), Some("demo/assets/bg/sign.png"));
    assert_eq!(d.message, "out of cheese");
    assert!(made.images.unwrap().images.is_empty());
    assert!(!temp.path("demo/build/demo.gb").exists());
}

/// An rgbgfx that runs the real one, then sets the first byte of the tile
/// data it wrote (`-o`) to $FF: row 0's low plane, every pixel's bit 0.
fn tools_whose_rgbgfx_misdraws(temp: &common::Temp) -> gbcli_build::Toolchain {
    let real = rgbds().rgbgfx().unwrap().display().to_string();
    let body = format!(
        "'{real}' \"$@\" || exit $?\n\
         out=; prev=\n\
         for a in \"$@\"; do [ \"$prev\" = -o ] && out=$a; prev=$a; done\n\
         printf '\\377' | dd of=\"$out\" bs=1 count=1 conv=notrunc 2>/dev/null"
    );
    tools_with(temp, Some(&body))
}

#[test]
fn a_background_rgbgfx_draws_otherwise_than_planned_stops_the_build() {
    let (temp, options) = starter("bg-rgbgfx-disagrees");
    write_png(&temp.path("demo/assets/bg/plain.png"), 16, 8, |_, _| WHITE);
    let tools = tools_whose_rgbgfx_misdraws(&temp);
    let made = match build(&tools, &options) {
        Err(BuildError::Failed { made, .. }) => *made,
        other => panic!("expected the disagreement to stop the build, got {other:?}"),
    };
    let p = problem(&made, ImageLimit::RgbgfxAgrees);
    assert_eq!(p.image.as_deref(), Some("demo/assets/bg/plain.png"));
    assert_eq!(p.severity, Severity::Error);
    // rgbgfx draws every cell (gbcli keeps the unique tiles), so the cell
    // whose tile it misdrew is the first.
    assert_eq!(p.tiles, [TileAt { x: 0, y: 0 }]);
    assert!(
        p.message
            .contains("pixel (0, 0), gbcli planned color index 0 and rgbgfx wrote index 1"),
        "{}",
        p.message
    );
    assert!(made.images.unwrap().images.is_empty());
    assert!(!temp.path("demo/build/demo.gb").exists());
}

/// Colors that share a shade reach rgbgfx as one color, and its output is
/// still checked: a misdrawn tile stops the build.
#[test]
fn a_background_with_a_shared_shade_rgbgfx_misdraws_stops_the_build() {
    let (temp, options) = starter("bg-shared-disagrees");
    let near_white: Rgba = [0xF0, 0xF0, 0xF0, 0xFF];
    // Black too, so the stretch keeps #f0f0f0 beside white, in shade 0.
    write_png(&temp.path("demo/assets/bg/haze.png"), 24, 8, |x, _| {
        [WHITE, near_white, BLACK][(x / 8) as usize]
    });
    // Indexed, so the shade shared across tiles warns and merges.
    indexed(&temp.path("demo"), &["bg/haze.png"]);
    let tools = tools_whose_rgbgfx_misdraws(&temp);
    let made = match build(&tools, &options) {
        Err(BuildError::Failed { made, .. }) => *made,
        other => panic!("expected the disagreement to stop the build, got {other:?}"),
    };
    let p = problem(&made, ImageLimit::RgbgfxAgrees);
    assert_eq!(p.tiles, [TileAt { x: 0, y: 0 }]);
    assert!(
        p.message
            .contains("planned color index 0 and rgbgfx wrote index 1"),
        "{}",
        p.message
    );
    assert_eq!(
        problem(&made, ImageLimit::SharedShade).severity,
        Severity::Warning
    );
}

#[test]
fn a_sheet_rgbgfx_draws_otherwise_than_planned_stops_the_build() {
    let (temp, options) = starter("sheet-rgbgfx-disagrees");
    write_png(&temp.path("demo/assets/sprites/dot.png"), 8, 16, |x, y| {
        if (x, y) == (0, 8) { BLACK } else { CLEAR }
    });
    let tools = tools_whose_rgbgfx_misdraws(&temp);
    let made = match build(&tools, &options) {
        Err(BuildError::Failed { made, .. }) => *made,
        other => panic!("expected the disagreement to stop the build, got {other:?}"),
    };
    let p = problem(&made, ImageLimit::RgbgfxAgrees);
    assert_eq!(p.image.as_deref(), Some("demo/assets/sprites/dot.png"));
    assert_eq!(p.tiles, [TileAt { x: 0, y: 0 }]);
    assert!(p.message.contains("planned color index 0"), "{}", p.message);
    assert!(made.images.unwrap().images.is_empty());
}
