//! The GBC include API and previews end to end (leaf zcxvjc): a program
//! that uses every GBC label assembles under `build --gbc`, the constants
//! are in the `.sym`, and in a `--model gbc` session the screen draws each
//! color exactly as the GBC preview does (plan question 8).

use std::fs;
use std::path::Path;

use crate::common::{Project, png_golden, read_png};

/// Four tiles in a row of four colors each, neighbor columns unlike, so the
/// preview's scale is the run of its first pixel.
fn town(x: u32, _: u32) -> [u8; 4] {
    const COLORS: [[u8; 4]; 6] = [
        [0xFF, 0x00, 0x00, 0xFF],
        [0x00, 0x80, 0xFF, 0xFF],
        [0x20, 0xC0, 0x40, 0xFF],
        [0xF8, 0xF8, 0x08, 0xFF],
        [0x40, 0x10, 0x60, 0xFF],
        [0xFF, 0xFF, 0xFF, 0xFF],
    ];
    COLORS[((x / 8 + x % 4) % 6) as usize]
}

/// One 8x16 object, red over green on transparent.
fn hero(x: u32, y: u32) -> [u8; 4] {
    match (x, y < 8) {
        (0..=4, true) => [0xFF, 0x00, 0x00, 0xFF],
        (0..=4, false) => [0x00, 0xFF, 0x00, 0xFF],
        _ => [0; 4],
    }
}

fn write_png(path: &Path, width: u32, height: u32, pixel: impl Fn(u32, u32) -> [u8; 4]) {
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    let data: Vec<u8> = (0..height)
        .flat_map(|y| (0..width).map(move |x| (x, y)))
        .flat_map(|(x, y)| pixel(x, y))
        .collect();
    let mut encoder = png::Encoder::new(fs::File::create(path).unwrap(), width, height);
    encoder.set_color(png::ColorType::Rgba);
    encoder.set_depth(png::BitDepth::Eight);
    let mut writer = encoder.write_header().unwrap();
    writer.write_image_data(&data).unwrap();
}

/// Loads the town's GBC tiles, map, attribute map and palettes, the OBJ
/// palettes and the hero's GBC tiles, and names every other GBC symbol.
const PROGRAM: &str = r#"include "hardware.inc"
include "images.inc"

SECTION "Header", ROM0[$0100]
	nop
	jp Start
	ds $0150 - @, 0

SECTION "Start", ROM0
Start:
	di
	ld sp, $E000
.wait
	ldh a, [rLY]
	cp LY_VBLANK
	jr c, .wait
	xor a
	ldh [rLCDC], a
	; BG palettes, then OBJ palettes, with autoincrement.
	ld a, BGPI_AUTOINC
	ldh [rBCPS], a
	ld hl, TownPalettes
	ld b, TownPalettesEnd - TownPalettes
.bg
	ld a, [hli]
	ldh [rBCPD], a
	dec b
	jr nz, .bg
	ld a, OBPI_AUTOINC
	ldh [rOCPS], a
	ld hl, ObjPalettesGbc
	ld b, ObjPalettesGbcEnd - ObjPalettesGbc
.obj
	ld a, [hli]
	ldh [rOCPD], a
	dec b
	jr nz, .obj
	; Tiles at $8000, the hero's after the town's.
	ld de, TownTilesGbc
	ld hl, $8000
	ld bc, TownTilesGbcEnd - TownTilesGbc
	call Copy
	ld de, HeroTilesGbc
	ld bc, HeroTilesGbcEnd - HeroTilesGbc
	call Copy
	; The map's first row, then its attributes in VRAM bank 1.
	ld de, TownTilemap
	ld hl, $9800
	ld bc, TOWN_WIDTH
	call Copy
	ld a, 1
	ldh [rVBK], a
	ld de, TownAttrmap
	ld hl, $9800
	ld bc, TownAttrmapEnd - TownAttrmap
	call Copy
	xor a
	ldh [rVBK], a
	ld a, LCDC_ON | LCDC_BG_ON | LCDC_BLOCK01
	ldh [rLCDC], a
.forever
	jr .forever

; de to hl, bc bytes.
Copy:
	ld a, [de]
	ld [hli], a
	inc de
	dec bc
	ld a, b
	or c
	jr nz, Copy
	ret

SECTION "Every GBC symbol", ROM0
	dw PaletteForest, PaletteForestEnd, HeroFrames
	db TOWN_PALETTES, OBJ_PALETTES, PALETTE_FOREST, GBCLI_GBC
"#;

#[test]
fn a_program_using_every_gbc_label_draws_what_the_gbc_preview_shows() {
    let p = Project::new("images-gbc");
    assert_eq!(p.rgbds(&["new", "game"]).code, 0);
    for file in ["src/music.asm", "src/hUGEDriver.asm", "src/oam_dma.asm"] {
        fs::remove_file(p.path(&format!("game/{file}"))).unwrap();
    }
    fs::remove_dir_all(p.path("game/assets/music")).unwrap();
    fs::write(p.path("game/src/main.asm"), PROGRAM).unwrap();
    write_png(&p.path("game/assets/bg/town.png"), 32, 8, town);
    write_png(&p.path("game/assets/sprites/hero.png"), 8, 16, hero);
    fs::write(
        p.path("game/assets/palettes.txt"),
        "forest: #fff #8c4 #363 #021\n",
    )
    .unwrap();

    let built = p.rgbds(&["build", "game", "--gbc"]);
    assert_eq!(built.code, 0, "{}{}", built.stdout, built.stderr);
    let sym = fs::read_to_string(p.path("game/build/gbc/game.sym")).unwrap();
    for name in [
        "TOWN_PALETTES",
        "OBJ_PALETTES",
        "PALETTE_FOREST",
        "TownTilesGbc",
        "TownAttrmap",
        "TownPalettes",
        "HeroTilesGbc",
        "ObjPalettesGbc",
        "PaletteForest",
    ] {
        let defined = sym.lines().any(|l| l.ends_with(&format!(" {name}")));
        assert!(defined, "{name} in\n{sym}");
    }
    for (preview, golden) in [
        ("bg/town.gbc.preview.png", "town_gbc_preview.png"),
        ("sprites/hero.gbc.preview.png", "hero_gbc_preview.png"),
    ] {
        let path = p.path(&format!("game/build/images/{preview}"));
        png_golden(&fs::read(&path).unwrap(), golden);
    }

    let run = p.gbcli(&[
        "run",
        "game/build/gbc/game.gbc",
        "--session",
        "color",
        "--model",
        "gbc",
        "--frames",
        "30",
        "--shot",
        "shot.png",
        "--scale",
        "1",
    ]);
    assert_eq!(run.code, 0, "{}{}", run.stdout, run.stderr);
    let (width, shot) = read_png(&p.path("shot.png"));
    assert_eq!(width, 160);
    let (pw, preview) = read_png(&p.path("game/build/images/bg/town.gbc.preview.png"));
    let scale = preview.iter().take_while(|&&c| c == preview[0]).count() as u32;
    assert!(scale >= 1, "{scale}");
    for y in 0..8 {
        for x in 0..32 {
            let drawn = shot[(y * 160 + x) as usize];
            let previewed = preview[(y * scale * pw + x * scale) as usize];
            assert_eq!(drawn, previewed, "pixel ({x}, {y})");
        }
    }
}
