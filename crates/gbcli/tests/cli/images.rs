//! Images through the binary: a PNG under `assets/` in a `new` project is
//! converted by `build`, the program includes what it became, and the
//! screenshot shows it pixel for pixel; a background over a hardware limit
//! comes back as the limit, its count and the remedy, in rows and JSON.

use std::fs;
use std::path::Path;

use crate::common::{Project, golden, read_png};

const GRAYS: [[u8; 3]; 4] = [[0xFF; 3], [0xAA; 3], [0x55; 3], [0x00; 3]];

/// A 4x2-tile sign in all four shades: stripes, a checker and a frame.
fn sign(x: u32, y: u32) -> [u8; 3] {
    let shade = match (x / 8, y / 8) {
        (0, _) => x % 4,
        (1, _) => (x / 2 + y / 2) % 2 * 3,
        (2, 0) => 2,
        (3, _) if x.is_multiple_of(8) || y % 8 == 7 => 3,
        _ => 0,
    };
    GRAYS[shade as usize]
}

pub(crate) fn write_png(path: &Path, width: u32, height: u32, pixel: impl Fn(u32, u32) -> [u8; 3]) {
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    let data: Vec<u8> = (0..height)
        .flat_map(|y| (0..width).map(move |x| (x, y)))
        .flat_map(|(x, y)| pixel(x, y))
        .collect();
    let mut encoder = png::Encoder::new(fs::File::create(path).unwrap(), width, height);
    encoder.set_color(png::ColorType::Rgb);
    encoder.set_depth(png::BitDepth::Eight);
    let mut writer = encoder.write_header().unwrap();
    writer.write_image_data(&data).unwrap();
}

/// The starter's program, also drawing the sign at map row 2, column 2,
/// its tiles loaded from tile 16 up.
const LOAD_SIGN: &str = "\t; The sign from assets/bg/sign.png: its tiles from tile 16, its map
\t; at row 2, column 2, each entry offset by 16.
\tld de, SignTiles
\tld hl, TILEBLOCK0 + 16 * 16
\tld bc, SIGN_TILES * 16
\tcall CopyMem
\tld de, SignTilemap
\tld hl, TILEMAP0 + 2 * 32 + 2
\tld c, SIGN_HEIGHT
.signRow
\tld b, SIGN_WIDTH
.signTile
\tld a, [de]
\tadd 16
\tld [hli], a
\tinc de
\tdec b
\tjr nz, .signTile
\tld a, l
\tadd 32 - SIGN_WIDTH
\tld l, a
\tld a, h
\tadc 0
\tld h, a
\tdec c
\tjr nz, .signRow

";

#[test]
fn a_png_in_a_new_project_is_built_and_shows_on_screen_pixel_for_pixel() {
    let p = Project::new("images-on-screen");
    let made = p.rgbds(&["new", "game"]);
    assert_eq!(made.code, 0, "{}", made.stderr);
    write_png(&p.path("game/assets/bg/sign.png"), 32, 16, sign);
    let main = p.path("game/src/main.asm");
    let text = fs::read_to_string(&main).unwrap();
    let text = text
        .replacen(
            "include \"hardware.inc\"\n",
            "include \"hardware.inc\"\ninclude \"images.inc\"\n",
            1,
        )
        .replacen("\t; One sprite:", &format!("{LOAD_SIGN}\t; One sprite:"), 1);
    fs::write(&main, text).unwrap();

    let built = p.rgbds(&["build", "game"]);
    assert_eq!(built.code, 0, "{}{}", built.stdout, built.stderr);
    for row in [
        "image kind=bg source=game/assets/bg/sign.png label=Sign size=4x2 tiles=",
        " preview=game/build/images/bg/sign.preview.png\n",
        "images include=game/build/images/images.inc converted=1 fonts=1 errors=0 \
         warnings=0 shades=known palette=dmg_grays\n",
    ] {
        assert!(built.stdout.contains(row), "{row:?} in\n{}", built.stdout);
    }

    let shot = p.rgbds(&[
        "run",
        "game/build/game.gb",
        "--session",
        "game",
        "--frames",
        "120",
        "--shot",
        "sign.png",
    ]);
    assert_eq!(shot.code, 0, "{}", shot.stderr);
    let (width, pixels) = read_png(&p.path("sign.png"));
    let scale = width / 160;
    assert!(scale >= 1, "a full-frame shot: {width} wide");
    for y in 0..16 {
        for x in 0..32 {
            let (sx, sy) = ((16 + x) * scale, (16 + y) * scale);
            assert_eq!(
                pixels[(sy * width + sx) as usize],
                sign(x, y),
                "the sign's pixel ({x}, {y}) on screen"
            );
        }
    }
}

/// A 16x16 hero: a dark-and-black left half on transparent, and the right
/// half its mirror image, so the sheet keeps one 8x16 unit and draws it
/// twice, once flipped.
fn hero(x: u32, y: u32) -> Option<[u8; 3]> {
    let x = if x < 8 { x } else { 15 - x };
    match (x, y) {
        (0..=1, _) => Some(GRAYS[3]),
        (2..=5, 4..=9) => Some(GRAYS[2]),
        (6, 0..=2) => Some(GRAYS[3]),
        _ => None,
    }
}

/// The program, also drawing frame 0 of the hero at (100, 100) as 8x16
/// objects from tile 32, through OBP1 set to HERO_OBP.
const LOAD_HERO: &str = "\tld de, HeroTiles
\tld hl, TILEBLOCK0 + 32 * 16
\tld bc, HERO_TILES * 16
\tcall CopyMem
\tld hl, HeroFrames
\tld a, [hli]
\tld h, [hl]
\tld l, a
\tld a, [hli]
\tld b, a
\tld de, wShadowOAM + 4
.heroObject
\tld a, [hli]
\tadd 100 + OAM_Y_OFS
\tld [de], a
\tinc de
\tld a, [hli]
\tadd 100 + OAM_X_OFS
\tld [de], a
\tinc de
\tld a, [hli]
\tadd 32
\tld [de], a
\tinc de
\tld a, [hli]
\tor OAM_PAL1
\tld [de], a
\tinc de
\tdec b
\tjr nz, .heroObject
\tld a, HERO_OBP
\tldh [rOBP1], a

";

#[test]
fn a_sprite_sheet_is_built_into_objects_that_show_on_screen_flipped_and_shaded() {
    let p = Project::new("images-hero");
    assert_eq!(p.rgbds(&["new", "game"]).code, 0);
    let path = p.path("game/assets/sprites/hero.png");
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    let data: Vec<u8> = (0..16u32)
        .flat_map(|y| (0..16u32).map(move |x| (x, y)))
        .flat_map(|(x, y)| match hero(x, y) {
            Some([r, g, b]) => [r, g, b, 0xFF],
            None => [0, 0, 0, 0],
        })
        .collect();
    let mut encoder = png::Encoder::new(fs::File::create(&path).unwrap(), 16, 16);
    encoder.set_color(png::ColorType::Rgba);
    encoder.set_depth(png::BitDepth::Eight);
    encoder
        .write_header()
        .unwrap()
        .write_image_data(&data)
        .unwrap();
    let main = p.path("game/src/main.asm");
    let text = fs::read_to_string(&main)
        .unwrap()
        .replacen(
            "include \"hardware.inc\"\n",
            "include \"hardware.inc\"\ninclude \"images.inc\"\n",
            1,
        )
        .replacen("\t; One sprite:", &format!("{LOAD_HERO}\t; One sprite:"), 1)
        .replacen("LCDC_OBJ_ON |", "LCDC_OBJ_ON | LCDC_OBJ_16 |", 1);
    fs::write(&main, text).unwrap();

    let built = p.rgbds(&["build", "game"]);
    assert_eq!(built.code, 0, "{}{}", built.stdout, built.stderr);
    let row = "image kind=sprites source=game/assets/sprites/hero.png label=Hero size=2x2 \
               tiles=2 frames=1 objects=2 ";
    assert!(built.stdout.contains(row), "{}", built.stdout);

    let shot = p.rgbds(&[
        "run",
        "game/build/game.gb",
        "--session",
        "game",
        "--frames",
        "120",
        "--shot",
        "hero.png",
    ]);
    assert_eq!(shot.code, 0, "{}", shot.stderr);
    let (width, pixels) = read_png(&p.path("hero.png"));
    let scale = width / 160;
    for y in 0..16 {
        for x in 0..16 {
            let (sx, sy) = ((100 + x) * scale, (100 + y) * scale);
            // Transparent shows the blank background, shade 0.
            let want = hero(x, y).unwrap_or(GRAYS[0]);
            assert_eq!(
                pixels[(sy * width + sx) as usize],
                want,
                "the hero's pixel ({x}, {y}) on screen"
            );
        }
    }
}

/// The images result of a DMG build of a background and a sheet, pinned
/// whole: the GBC types (leaf ZA5Z7c) and the target (leaf vo5gRl) must
/// leave a DMG build's `images` JSON as it was.
#[test]
fn a_dmg_builds_images_json_is_pinned() {
    let p = Project::new("images-json");
    assert_eq!(p.rgbds(&["new", "game"]).code, 0);
    write_png(&p.path("game/assets/bg/sign.png"), 32, 16, sign);
    let path = p.path("game/assets/sprites/hero.png");
    let data: Vec<u8> = (0..16u32)
        .flat_map(|y| (0..16u32).map(move |x| (x, y)))
        .flat_map(|(x, y)| match hero(x, y) {
            Some([r, g, b]) => [r, g, b, 0xFF],
            None => [0, 0, 0, 0],
        })
        .collect();
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    let mut encoder = png::Encoder::new(fs::File::create(&path).unwrap(), 16, 16);
    encoder.set_color(png::ColorType::Rgba);
    encoder.set_depth(png::BitDepth::Eight);
    encoder
        .write_header()
        .unwrap()
        .write_image_data(&data)
        .unwrap();
    let json = p.rgbds(&["build", "game", "--json"]);
    assert_eq!(json.code, 0, "{}{}", json.stdout, json.stderr);
    // The line as printed, so field order is pinned too.
    golden("images.json.txt", &json.stdout);
}

#[test]
fn an_over_budget_background_comes_back_as_its_limit_in_rows_and_json() {
    let p = Project::new("images-over");
    assert_eq!(p.rgbds(&["new", "game"]).code, 0);
    // 17x16 tiles, each different: 272 unique tiles for a 256-tile map.
    write_png(&p.path("game/assets/bg/town.png"), 136, 128, |x, y| {
        let n = (y / 8) * 17 + x / 8 + 1;
        let bit = (y % 8) * 8 + x % 8;
        if bit < 16 && (n >> bit) & 1 == 1 {
            GRAYS[3]
        } else {
            GRAYS[0]
        }
    });
    let built = p.rgbds(&["build", "game"]);
    assert_eq!(built.code, 4, "{}{}", built.stdout, built.stderr);
    for row in [
        "problem limit=bg_tiles_map severity=error image=game/assets/bg/town.png count=272 \
         max=256 tiles=\"0,0 1,0 2,0 3,0 4,0 5,0 6,0 7,0 (+264 more; --json lists every tile)\"",
        "images converted=0 fonts=1 errors=1 warnings=0 shades=known palette=dmg_grays\n",
    ] {
        assert!(built.stdout.contains(row), "{row:?} in\n{}", built.stdout);
    }
    assert!(
        built.stderr.contains("bg_tiles_map") && built.stderr.contains("split the image"),
        "{}",
        built.stderr
    );

    let json = p.rgbds(&["build", "game", "--json"]);
    let line: serde_json::Value = serde_json::from_str(json.stdout.trim()).unwrap();
    let problem = &line["error"]["result"]["build"]["images"]["problems"][0];
    assert_eq!(problem["limit"], "bg_tiles_map");
    assert_eq!(problem["count"], 272);
    assert_eq!(problem["max"], 256);
    assert_eq!(problem["tiles"].as_array().unwrap().len(), 272);
    assert!(
        problem["remedy"]
            .as_str()
            .unwrap()
            .contains("split the image")
    );
}

#[test]
fn a_malformed_palette_file_is_a_problem_row_naming_its_line() {
    let p = Project::new("images-palette-bad");
    assert_eq!(p.rgbds(&["new", "game"]).code, 0);
    write_png(&p.path("game/assets/bg/sky.png"), 8, 8, |_, _| GRAYS[0]);
    fs::write(
        p.path("game/assets/palettes.txt"),
        "# lightest first\n\ndmg: #ffffff #aaaa #555555 #000000\n",
    )
    .unwrap();
    let built = p.rgbds(&["build", "game"]);
    assert_eq!(built.code, 4, "{}{}", built.stdout, built.stderr);
    let row = "problem limit=palette_file severity=error image=game/assets/palettes.txt count=";
    assert!(built.stdout.contains(row), "{row:?} in\n{}", built.stdout);
    assert!(built.stdout.contains(" line=3 "), "{}", built.stdout);
}

/// LoadHigh's question 993: `#rrggbb = N` in assets/palettes.txt pins a
/// color's shade. A color pinned twice is a palette_file row at its line;
/// `--json` lists every color's shade and what decided it, and plain rows
/// list none (the CLI has no verbose mode to show them under).
#[test]
fn a_shade_pin_shades_its_color_and_json_lists_every_colors_shade() {
    let p = Project::new("images-shade-pins");
    assert_eq!(p.rgbds(&["new", "game"]).code, 0);
    write_png(&p.path("game/assets/bg/sky.png"), 16, 8, |x, _| {
        GRAYS[usize::from(x >= 8)]
    });
    let palettes = p.path("game/assets/palettes.txt");
    fs::write(&palettes, "#aaaaaa = 3\n# again\n#aaa = 1\n").unwrap();
    let built = p.rgbds(&["build", "game"]);
    assert_eq!(built.code, 4, "{}{}", built.stdout, built.stderr);
    let row = "problem limit=palette_file severity=error image=game/assets/palettes.txt count=";
    assert!(built.stdout.contains(row), "{row:?} in\n{}", built.stdout);
    assert!(built.stdout.contains(" line=3 "), "{}", built.stdout);
    assert!(built.stdout.contains("first on line 1"), "{}", built.stdout);

    fs::write(&palettes, "#aaaaaa = 3\n").unwrap();
    let built = p.rgbds(&["build", "game"]);
    assert_eq!(built.code, 0, "{}{}", built.stdout, built.stderr);
    assert!(
        !built.stdout.lines().any(|l| l.starts_with("shade ")),
        "{}",
        built.stdout
    );
    let json = p.rgbds(&["build", "game", "--json"]);
    assert_eq!(json.code, 0, "{}{}", json.stdout, json.stderr);
    let line: serde_json::Value = serde_json::from_str(json.stdout.trim()).unwrap();
    let shades = &line["result"]["build"]["images"]["color_shades"];
    assert_eq!(
        shades,
        &serde_json::json!([
            {"color": 0x56B5, "shade": 3, "art": true, "rule": "pinned"},
            {"color": 0x7FFF, "shade": 0, "art": true, "rule": "known"},
        ]),
        "{}",
        json.stdout
    );
}
