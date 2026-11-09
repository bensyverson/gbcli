//! The starter's renderer, `src/text.asm` (leaf RDlaLo), run in the
//! emulator against `gbcli_font::set`, the reference `gbcli font --sample`
//! prints: `TextDraw` sets every glyph of the starter font at every pen
//! offset 0-7, in shades the planes disagree on, across page carries of its
//! tile pointer, and stops at a line break; `TextLoadTiles` expands glyphs
//! to tiles. A probe program replaces the starter's `main.asm` and writes
//! into WRAM. The cost per glyph that `gbcli help fonts` states is pinned
//! here, measured over those same draws.

use crate::common;
use crate::player::{booted, machine};

use std::collections::BTreeSet;
use std::fs;

use gbcli_build::build;
use gbcli_core::{Budget, Region, RunRequest};
use gbcli_engine::profile::MeasureSpec;
use gbcli_font::Font;
use gbcli_protocol::Location;

use common::{rgbds, starter};

/// Ink shade 1 and paper shade 2: plane 0 is ink's, plane 1 paper's, so
/// a renderer that mixes the planes up shows it.
const INK: u8 = 1;
const PAPER: u8 = 2;

/// The cost of one glyph in M-cycles, over every glyph at every pen offset
/// (`gbcli help fonts` states the maximum, rounded up, as its bound).
const MIN_GLYPH_M: u64 = 517;
const MAX_GLYPH_M: u64 = 954;

/// Tiles a segment of the line may take: each of the two segments has a
/// buffer this big (`wLineA` in WRAM0, `wLineB` in WRAMX).
const SEGMENT_TILES: usize = 224;

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
	ld hl, wLineA
	ld bc, wEndA - wLineA
	call Fill
	ld hl, wLineB
	ld bc, wEndB - wLineB
	call Fill
	ld hl, SegmentA
	ld de, wLineA
	ld bc, INK << 8 | PAPER
	call TextDraw
	ld [wTilesA], a
	; hl is past the segment's end: the next one.
	ld de, wLineB
	ld bc, INK << 8 | PAPER
	call TextDraw
	ld [wTilesB], a
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
	ld [wLoadReturn], a
	ld a, 1
	ld [wDone], a
.idle
	jr .idle

Fill:
.fill
	ld a, $5A
	ld [hli], a
	dec bc
	ld a, b
	or c
	jr nz, .fill
	ret

SegmentA:
	db SEGMENT_A, MAIN_FONT_NEWLINE
SegmentB:
	db SEGMENT_B, MAIN_FONT_NEWLINE
	db LINE2, MAIN_FONT_END
TextEnd:

SECTION "Probe out A", WRAM0
wLineA: ds 16 * SEGMENT_TILES
wEndA:
wLine2: ds 16 * 4
wLoaded: ds 16 * LOADED
wTilesA: db
wTilesB: db
wTiles2: db
wAfter: dw
wLoadReturn: db
wDone: db

SECTION "Probe out B", WRAMX
wLineB: ds 16 * SEGMENT_TILES
wEndB:
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

/// Segments of glyph numbers, each at most `SEGMENT_TILES` tiles wide, in
/// which every glyph of `font` starts at every pen offset 0-7 (the pen is
/// the sum of the widths so far, mod 8). Fillers, glyphs that visit no new
/// pair, steer the pen to an offset with glyphs left. A glyph at pen 7 or
/// any other offset crosses into the next tile, and the tile pointer's low
/// byte wraps into its high byte every 16 tiles.
fn every_glyph_at_every_offset(font: &Font) -> Vec<Vec<usize>> {
    let width = |g: usize| usize::from(font.glyphs[g].width);
    let mut todo: BTreeSet<(usize, usize)> = (0..font.glyphs.len())
        .flat_map(|g| (0..8).map(move |k| (g, k)))
        .collect();
    let mut segments = vec![Vec::new()];
    let mut pen = 0;
    while !todo.is_empty() {
        let k = pen % 8;
        let fresh = todo.iter().find(|&&(_, o)| o == k).map(|&(g, _)| g);
        let next = fresh.unwrap_or_else(|| {
            // A filler that lands on an offset with glyphs left.
            (0..font.glyphs.len())
                .find(|&g| todo.iter().any(|&(_, o)| o == (pen + width(g)) % 8))
                .expect("a filler")
        });
        if (pen + width(next)).div_ceil(8) > SEGMENT_TILES {
            segments.push(Vec::new());
            pen = 0;
            continue;
        }
        todo.remove(&(next, k));
        segments.last_mut().unwrap().push(next);
        pen += width(next);
    }
    segments
}

/// A glyph number's character: the starter font is ASCII from the space.
fn text_of(glyphs: &[usize]) -> String {
    glyphs.iter().map(|&g| char::from(32 + g as u8)).collect()
}

#[test]
fn text_draw_sets_every_glyph_at_every_pen_offset_as_the_sample_does_within_the_stated_cost() {
    let (temp, options) = starter("text-probe");
    let project = temp.path("demo");
    for file in ["src/music.asm", "src/hUGEDriver.asm", "src/oam_dma.asm"] {
        fs::remove_file(project.join(file)).unwrap();
    }
    fs::remove_dir_all(project.join("assets/music")).unwrap();
    let font_text = fs::read_to_string(project.join("assets/fonts/main.yaff")).unwrap();
    let font = gbcli_font::read(&font_text).unwrap();
    assert_eq!(font.glyphs.len(), 95);
    let segments = every_glyph_at_every_offset(&font);
    assert_eq!(segments.len(), 2, "two buffers, two segments");
    let numbers = |text: &str| {
        let line = gbcli_font::set(&font, text);
        assert!(line.missing.is_empty(), "{:?}", line.missing);
        let list: Vec<String> = line.glyphs.iter().map(usize::to_string).collect();
        (list.join(", "), line)
    };
    let (db_a, set_a) = numbers(&text_of(&segments[0]));
    let (db_b, set_b) = numbers(&text_of(&segments[1]));
    let (db2, set2) = numbers("Wj");
    let (first_loaded, loaded) = (33, 3); // 'A', 'B', 'C'
    let defines = format!(
        "DEF INK EQU {INK}\nDEF PAPER EQU {PAPER}\nDEF FIRST_LOADED EQU {first_loaded}\n\
         DEF LOADED EQU {loaded}\nDEF SEGMENT_TILES EQU {SEGMENT_TILES}\n\
         DEF SEGMENT_A EQUS \"{db_a}\"\nDEF SEGMENT_B EQUS \"{db_b}\"\nDEF LINE2 EQUS \"{db2}\"\n"
    );
    fs::write(project.join("src/main.asm"), format!("{defines}{PROBE}")).unwrap();
    build(&rgbds(), &options).unwrap();

    let rom = fs::read(project.join("build/demo.gb")).unwrap();
    let sym = fs::read_to_string(project.join("build/demo.sym")).unwrap();
    let mut m = booted(&rom);
    let core = m.core_mut();
    core.run(&mut RunRequest::new(Budget::frames(120))).unwrap();
    let wram = core.region(Region::Wram).unwrap().bytes;
    let at = |symbol: &str, len: usize| {
        let a = address(&sym, symbol) - 0xC000;
        wram[a..a + len].to_vec()
    };
    assert_eq!(at("wDone", 1), [1], "the probe ran to its end");

    for (name, set, tiles_symbol, buffer) in [
        ("segment A", &set_a, "wTilesA", "wLineA"),
        ("segment B", &set_b, "wTilesB", "wLineB"),
    ] {
        let tiles = set.tiles as usize;
        assert_eq!(
            at(tiles_symbol, 1),
            [set.tiles as u8],
            "{name}: TextDraw returns its tiles in a"
        );
        let drawn = at(buffer, 16 * (tiles + usize::from(tiles < SEGMENT_TILES)));
        assert_eq!(
            drawn[..16 * tiles],
            two_shades(&set.data, INK, PAPER)[..],
            "{name}, by the layout rules"
        );
        assert!(
            drawn[16 * tiles..].iter().all(|&b| b == 0x5A),
            "{name}: nothing past the line's last tile is written"
        );
    }

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
    assert_eq!(at("wLoadReturn", 1), [0], "TextLoadTiles returns a = 0");

    // Cost, from power-on: one span from each glyph's arrival at
    // `TextDraw.glyph` to the next, the string's end arriving there too.
    // Between segments the span also holds the return and the next call.
    let (glyph_a, glyph_b) = (segments[0].len(), segments[1].len());
    let glyph = address(&sym, "TextDraw.glyph") as u16;
    let at_glyph = || Location {
        addr: glyph,
        bank: None,
        symbol: None,
    };
    let mut m = machine(&rom, Some(&sym));
    let measured = m
        .measure(&MeasureSpec {
            from: at_glyph(),
            to: at_glyph(),
            iterations: (glyph_a + 1 + glyph_b) as u32,
            budget: Budget::frames(600),
            interrupt: None,
            keys: &[],
        })
        .unwrap();
    let costs: Vec<u64> = measured
        .samples
        .iter()
        .enumerate()
        .filter(|&(i, _)| i != glyph_a)
        .map(|(_, s)| s.m(measured.speed))
        .collect();
    assert_eq!(costs.len(), glyph_a + glyph_b);
    assert_eq!(
        (costs.iter().min(), costs.iter().max()),
        (Some(&MIN_GLYPH_M), Some(&MAX_GLYPH_M)),
        "M-cycles a glyph: change `gbcli help fonts`' bound with these"
    );
}
