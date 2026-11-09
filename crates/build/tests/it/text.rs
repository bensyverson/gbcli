//! The starter's renderer, `src/text.asm` (leaf RDlaLo), run in the
//! emulator against `gbcli_font::set`, the reference `gbcli font --sample`
//! prints: `TextDraw` sets every glyph of the starter font at every pen
//! offset, in shades the planes disagree on, and stops at a line break;
//! `TextLoadTiles` expands glyphs to tiles. A probe program replaces the
//! starter's `main.asm` and writes into WRAM.

use crate::common;
use crate::player::booted;

use std::fs;

use gbcli_build::build;
use gbcli_core::{Budget, Region, RunRequest};

use common::{rgbds, starter};

/// Ink shade 1 and paper shade 2: plane 0 is ink's, plane 1 paper's, so
/// a renderer that mixes the planes up shows it.
const INK: u8 = 1;
const PAPER: u8 = 2;

const PROBE: &str = r#"include "hardware.inc"
include "images.inc"

SECTION "Header", ROM0[$0100]
	nop
	jp Start
	ds $0150 - @, 0

SECTION "Probe", ROM0
Start:
	di
	ld sp, $E000
	; Every output byte $5A first, so a write past the line shows.
	ld hl, wLine
	ld bc, wEnd - wLine
.fill
	ld a, $5A
	ld [hli], a
	dec bc
	ld a, b
	or c
	jr nz, .fill
	ld hl, Text
	ld de, wLine
	ld bc, INK << 8 | PAPER
	call TextDraw
	ld [wTiles], a
	; hl is past the line break: the second line.
	ld de, wLine2
	ld bc, INK << 8 | PAPER
	call TextDraw
	ld [wTiles2], a
	ld a, l
	ld [wAfter], a
	ld a, h
	ld [wAfter + 1], a
	ld de, MainFontGlyphs + 8 * FIRST_LOADED
	ld hl, wLoaded
	ld a, LOADED
	ld bc, INK << 8 | PAPER
	call TextLoadTiles
	ld a, 1
	ld [wDone], a
.idle
	jr .idle

Text:
	db LINE1, MAIN_FONT_NEWLINE, LINE2, MAIN_FONT_END
TextEnd:

SECTION "Probe out", WRAM0
wLine: ds 16 * 128
wLine2: ds 16 * 4
wLoaded: ds 16 * LOADED
wEnd:
wTiles: db
wTiles2: db
wAfter: dw
wDone: db
"#;

/// The address of `symbol` in a built `.sym`.
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
fn text_draw_sets_every_glyph_as_the_sample_does_and_text_load_tiles_expands_glyphs() {
    let (temp, options) = starter("text-probe");
    let project = temp.path("demo");
    for file in ["src/music.asm", "src/hUGEDriver.asm", "src/oam_dma.asm"] {
        fs::remove_file(project.join(file)).unwrap();
    }
    fs::remove_dir_all(project.join("assets/music")).unwrap();
    let font_text = fs::read_to_string(project.join("assets/fonts/main.yaff")).unwrap();
    let font = gbcli_font::read(&font_text).unwrap();
    // Every glyph, then again shifted one pixel by a leading 'i', so each
    // glyph lands at more than one pen offset.
    let all: String = (32u8..127).map(char::from).collect();
    let line1 = format!("{all}i{all}");
    let line2 = "Wj";
    let numbers = |text: &str| {
        let line = gbcli_font::set(&font, text);
        assert!(line.missing.is_empty(), "{:?}", line.missing);
        let list: Vec<String> = line.glyphs.iter().map(usize::to_string).collect();
        (list.join(", "), line)
    };
    let (db1, set1) = numbers(&line1);
    let (db2, set2) = numbers(line2);
    assert!(set1.tiles <= 128, "{} tiles fit wLine", set1.tiles);
    let (first_loaded, loaded) = (33, 3); // 'A', 'B', 'C'
    let defines = format!(
        "DEF INK EQU {INK}\nDEF PAPER EQU {PAPER}\nDEF FIRST_LOADED EQU {first_loaded}\n\
         DEF LOADED EQU {loaded}\nDEF LINE1 EQUS \"{db1}\"\nDEF LINE2 EQUS \"{db2}\"\n"
    );
    fs::write(project.join("src/main.asm"), format!("{defines}{PROBE}")).unwrap();
    build(&rgbds(), &options).unwrap();

    let rom = fs::read(project.join("build/demo.gb")).unwrap();
    let sym = fs::read_to_string(project.join("build/demo.sym")).unwrap();
    let mut m = booted(&rom);
    let core = m.core_mut();
    core.run(&mut RunRequest::new(Budget::frames(30))).unwrap();
    let wram = core.region(Region::Wram).unwrap().bytes;
    let at = |symbol: &str, len: usize| {
        let a = address(&sym, symbol) - 0xC000;
        wram[a..a + len].to_vec()
    };
    assert_eq!(at("wDone", 1), [1], "the probe ran to its end");

    let tiles1 = set1.tiles as usize;
    assert_eq!(
        at("wTiles", 1),
        [set1.tiles as u8],
        "TextDraw returns its tiles in a"
    );
    let drawn = at("wLine", 16 * (tiles1 + 1));
    assert_eq!(
        drawn[..16 * tiles1],
        two_shades(&set1.data, INK, PAPER)[..],
        "line 1, by the layout rules"
    );
    assert!(
        drawn[16 * tiles1..].iter().all(|&b| b == 0x5A),
        "nothing past the line's last tile is written"
    );

    let tiles2 = set2.tiles as usize;
    assert_eq!(at("wTiles2", 1), [set2.tiles as u8]);
    assert_eq!(
        at("wLine2", 16 * tiles2),
        two_shades(&set2.data, INK, PAPER),
        "line 2, after the line break"
    );
    let after = at("wAfter", 2);
    assert_eq!(
        usize::from(u16::from_le_bytes([after[0], after[1]])),
        address(&sym, "TextEnd"),
        "TextDraw returns hl past the string's end"
    );

    let glyphs: Vec<u8> = font.glyphs[first_loaded..first_loaded + loaded]
        .iter()
        .flat_map(|g| g.rows)
        .collect();
    assert_eq!(
        at("wLoaded", 16 * loaded),
        two_shades(&glyphs, INK, PAPER),
        "TextLoadTiles: ink where a bit is set, paper elsewhere"
    );
}
