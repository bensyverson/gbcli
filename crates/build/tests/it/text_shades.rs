//! `TextDraw` in all 16 ink and paper shade pairs, with glyph numbers past
//! the font's in the middle of the line (no room, no ink) and an empty
//! string, and `TextLoadTiles` over the whole font. The warm test in
//! `text.rs` has the one pair the planes disagree on; this is the rest of
//! the matrix, one more build.

use crate::common;
use crate::player::booted;

use std::fs;

use gbcli_build::build;
use gbcli_core::{Budget, Region, RunRequest};

use common::{rgbds, starter};

const PROBE: &str = r#"include "hardware.inc"
include "images.inc"

SECTION "Header", ROM0[$0100]
	nop
	jp Start
	ds $0150 - @, 0

SECTION "Probe", ROM0
Start:
	di
	ld sp, wStack
	ld hl, wPairs
	ld bc, wEnd - wPairs

.fill
	ld a, $5A
	ld [hli], a
	dec bc
	ld a, b
	or c
	jr nz, .fill
	ld a, 0
	ld [wPair], a
.pair
	; ink = pair / 4, paper = pair % 4, into wPairs + 256 * pair.
	ld a, [wPair]
	ld l, a
	ld h, 0
	REPT 8
	add hl, hl
	ENDR
	ld de, wPairs
	add hl, de
	ld d, h
	ld e, l
	ld a, [wPair]
	ld c, a
	srl a
	srl a
	ld b, a
	ld a, c
	and 3
	ld c, a
	ld hl, Text
	call TextDraw
	ld b, a
	ld hl, wTiles
	ld a, [wPair]
	ld e, a
	ld d, 0
	add hl, de
	ld [hl], b
	ld a, [wPair]
	inc a
	ld [wPair], a
	cp 16
	jr c, .pair
	ld hl, wEmpty
	ld b, 16
.fillEmpty
	ld a, $5A
	ld [hli], a
	dec b
	jr nz, .fillEmpty
	ld hl, Empty
	ld de, wEmpty
	ld bc, 1 << 8 | 2
	call TextDraw
	ld [wEmptyTiles], a
	ld de, MainFontGlyphs
	ld hl, wLoaded
	ld a, MAIN_FONT_GLYPHS
	ld bc, 1 << 8 | 2
	call TextLoadTiles
	ld a, 1
	ld [wDone], a
.idle
	jr .idle

Text:
	db LINE, MAIN_FONT_END
Empty:
	db MAIN_FONT_END

SECTION "Probe out", WRAMX
wPairs: ds 256 * 16
wEnd:

SECTION "Probe vars", WRAM0
wStackBottom: ds 64
wStack:
wPair: db
wEmpty: ds 16
wLoaded: ds 16 * MAIN_FONT_GLYPHS
wTiles: ds 16
wEmptyTiles: db
wDone: db
"#;

fn address(sym: &str, symbol: &str) -> usize {
    let line = sym
        .lines()
        .find(|l| l.split_whitespace().nth(1) == Some(symbol))
        .unwrap_or_else(|| panic!("{symbol} not in the .sym"));
    let addr = line.split_whitespace().next().unwrap().split(':').nth(1);
    usize::from_str_radix(addr.unwrap(), 16).unwrap()
}

/// 1bpp tiles as 2bpp in `ink` and `paper`: each row's two bit planes.
fn two_shades(data: &[u8], ink: u8, paper: u8) -> Vec<u8> {
    let plane = |row: u8, bit: u8| {
        let ink = if ink >> bit & 1 == 1 { row } else { 0 };
        let paper = if paper >> bit & 1 == 1 { !row } else { 0 };
        ink | paper
    };
    data.iter()
        .flat_map(|&row| [plane(row, 0), plane(row, 1)])
        .collect()
}

#[test]
#[ignore = "pre-release: all 16 shade pairs and the whole font; one build and run beyond the warm test's"]
fn text_draw_and_text_load_tiles_keep_every_shade_pair() {
    let (temp, options) = starter("text-shades");
    let project = temp.path("demo");
    for file in ["src/music.asm", "src/hUGEDriver.asm", "src/oam_dma.asm"] {
        fs::remove_file(project.join(file)).unwrap();
    }
    fs::remove_dir_all(project.join("assets/music")).unwrap();
    let font_text = fs::read_to_string(project.join("assets/fonts/main.yaff")).unwrap();
    let font = gbcli_font::read(&font_text).unwrap();
    let all: String = (b'A'..=b'N').map(char::from).collect();
    let set = gbcli_font::set(&font, &all);
    assert!(set.missing.is_empty());
    assert!(set.tiles < 16, "{} tiles fit a pair's 256 bytes", set.tiles);
    let mut numbers: Vec<String> = set.glyphs.iter().map(usize::to_string).collect();
    // Glyph numbers past the font's: no room, no ink.
    for (at, number) in [(3, "200"), (8, "95"), (12, "253")] {
        numbers.insert(at, number.into());
    }
    let defines = format!("DEF LINE EQUS \"{}\"\n", numbers.join(", "));
    fs::write(project.join("src/main.asm"), format!("{defines}{PROBE}")).unwrap();
    build(&rgbds(), &options).unwrap();

    let rom = fs::read(project.join("build/demo.gb")).unwrap();
    let sym = fs::read_to_string(project.join("build/demo.sym")).unwrap();
    let mut m = booted(&rom);
    let core = m.core_mut();
    core.run(&mut RunRequest::new(Budget::frames(240))).unwrap();
    let wram = core.region(Region::Wram).unwrap();
    let wram = wram.bytes;
    let at = |symbol: &str, len: usize| {
        let a = address(&sym, symbol) - 0xC000;
        wram[a..a + len].to_vec()
    };
    assert_eq!(at("wDone", 1), [1], "the probe ran to its end");
    let tiles = set.tiles as usize;
    for pair in 0u8..16 {
        let (ink, paper) = (pair >> 2, pair & 3);
        let drawn = at("wPairs", 256 * 16);
        let line = &drawn[256 * usize::from(pair)..][..256];
        assert_eq!(
            line[..16 * tiles],
            two_shades(&set.data, ink, paper)[..],
            "ink {ink} paper {paper}"
        );
        assert!(
            line[16 * tiles..].iter().all(|&b| b == 0x5A),
            "past the line, pair {pair}"
        );
        assert_eq!(at("wTiles", 16)[usize::from(pair)], set.tiles as u8);
    }
    assert_eq!(at("wEmptyTiles", 1), [0]);
    assert!(
        at("wEmpty", 16).iter().all(|&b| b == 0x5A),
        "an empty string writes nothing"
    );
    let glyphs: Vec<u8> = font.glyphs.iter().flat_map(|g| g.rows).collect();
    assert_eq!(at("wLoaded", 16 * 95), two_shades(&glyphs, 1, 2));
}
