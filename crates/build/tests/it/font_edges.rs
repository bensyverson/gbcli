//! Edge cases from review-fonts (cff86f7): charmap edge keys against FontAsset and
//! `gbcli_font::set`, the 254 boundary, a charlist under the cap, and a
//! project with no fonts.

use crate::common;

use std::fs;

use gbcli_build::build;

use common::{Temp, failed, rgbds, starter};

fn yaff(glyphs: &[(&str, &[&str])]) -> String {
    let mut out = String::from("name: Probe\n");
    for (label, rows) in glyphs {
        out += &format!("\n{label}:\n");
        for r in *rows {
            out += &format!("    {r}\n");
        }
    }
    out
}

fn rom_at(temp: &Temp, symbol: &str, len: usize) -> Vec<u8> {
    let sym = fs::read_to_string(temp.path("demo/build/demo.sym")).unwrap();
    let line = sym
        .lines()
        .find(|l| l.split_whitespace().nth(1) == Some(symbol))
        .unwrap_or_else(|| panic!("{symbol} not in the .sym:\n{sym}"));
    let place = line.split_whitespace().next().unwrap();
    let (bank, addr) = place.split_once(':').unwrap();
    let bank = usize::from_str_radix(bank, 16).unwrap();
    let addr = usize::from_str_radix(addr, 16).unwrap();
    let offset = if bank == 0 {
        addr
    } else {
        bank * 0x4000 + addr - 0x4000
    };
    let rom = fs::read(temp.path("demo/build/demo.gb")).unwrap();
    rom[offset..offset + len].to_vec()
}

const ROWS: [&str; 8] = ["@."; 8];

/// Labels in glyph order; each a 2-px glyph.
const LABELS: &[&str] = &[
    "u+0020",    // 0
    "'<'",       // 1
    "\"heart\"", // 2
    "'\"'",      // 3
    "'\\'",      // 4
    "'{'",       // 5
    "'}'",       // 6
    "'é'",       // 7 C3 A9
    "u+1F600",   // 8 four bytes
    "u+00A0",    // 9 no-break space
    "'>'",       // 10
    "'h'",       // 11
    "'e'",       // 12
    "'a'",       // 13
    "'r'",       // 14
    "'t'",       // 15
    "\"he\"",    // 16
    "';'",       // 17
    "','",       // 18
    "u+0027",    // 19 '
    "'ê'",       // 20 C3 AA, shares é's lead byte
    "u+200B",    // 21 zero-width space
    "'#'",       // 22
    "'@'",       // 23
    "\"ab-c\"",  // 24
    "u+FEFF",    // 25
    "u+2028",    // 26
];

#[test]
fn edge_keys_assemble_to_the_numbers_fontasset_and_set_give() {
    let (temp, options) = starter("review-fonts-keys");
    let glyphs: Vec<(&str, &[&str])> = LABELS.iter().map(|l| (*l, &ROWS[..])).collect();
    let text = yaff(&glyphs);
    fs::create_dir_all(temp.path("demo/assets/fonts")).unwrap();
    fs::write(temp.path("demo/assets/fonts/main.yaff"), &text).unwrap();
    // The same text twice: as rgbasm source, and as what it means.
    let asm_body =
        "<heart><he><hear<<>\\\"\\\\\\{\\}é😀\u{A0}>;,'ê\u{200B}#@ <ab-c><a\u{FEFF}\u{2028}a";
    let meant = "<heart><he><hear<<>\"\\{}é😀\u{A0}>;,'ê\u{200B}#@ <ab-c><a\u{FEFF}\u{2028}a";
    let src = format!(
        "INCLUDE \"images.inc\"\n\tMAIN_FONT_CHARMAP probe, 0\nSECTION \"probe\", ROMX\n\
         \tPUSHC probe\nProbe:: db \"{asm_body}\", MAIN_FONT_END\n\tPOPC\n"
    );
    fs::write(temp.path("demo/src/probe.asm"), src).unwrap();
    let made = build(&rgbds(), &options).expect("builds");
    let images = made.images.expect("images");
    let keys: Vec<String> = images.fonts[0]
        .glyphs
        .iter()
        .map(|g| g.key.clone())
        .collect();
    let font = gbcli_font::read(&text).unwrap();
    let line = gbcli_font::set(&font, meant);
    let want: Vec<u8> = line.glyphs.iter().map(|&i| i as u8).collect();
    let got = rom_at(&temp, "Probe", want.len() + 1);
    // FontAsset's order is the file's.
    let file_keys: Vec<String> = font.glyphs.iter().map(|g| g.key.text()).collect();
    assert_eq!(keys, file_keys);
    assert!(line.missing.is_empty(), "{:?}", line.missing);
    assert_eq!(&got[..want.len()], &want[..], "rgbasm vs gbcli_font::set");
    assert_eq!(got[want.len()], 0xFF);
    // And by hand, the first few: <heart> is 2, <he> 16, then < h e a r.
    assert_eq!(&got[..7], &[2, 16, 1, 11, 12, 13, 14]);
}

fn big_font(temp: &Temp, n: u32) {
    let labels: Vec<String> = (0..n).map(|i| format!("u+{:04X}", 0x4E00 + i)).collect();
    let glyphs: Vec<(&str, &[&str])> = labels.iter().map(|l| (l.as_str(), &ROWS[..])).collect();
    fs::create_dir_all(temp.path("demo/assets/fonts")).unwrap();
    fs::write(temp.path("demo/assets/fonts/big.yaff"), yaff(&glyphs)).unwrap();
}

#[test]
fn a_font_of_254_glyphs_build_and_the_last_is_253() {
    let (temp, options) = starter("review-fonts-254");
    big_font(&temp, 254);
    let last = char::from_u32(0x4E00 + 253).unwrap();
    let src = format!(
        "INCLUDE \"images.inc\"\n\tBIG_FONT_CHARMAP probe, 0\nSECTION \"probe\", ROMX\n\
         \tPUSHC probe\nProbe:: db \"一{last}\\n\", BIG_FONT_END\n\tPOPC\n"
    );
    fs::write(temp.path("demo/src/probe.asm"), src).unwrap();
    let made = build(&rgbds(), &options).expect("254 builds");
    assert_eq!(made.images.unwrap().fonts[0].glyphs.len(), 254);
    assert_eq!(rom_at(&temp, "Probe", 4), [0, 253, 0xFE, 0xFF]);
}

#[test]
fn a_charlist_under_the_cap_selects_exactly_what_it_lists() {
    let (temp, options) = starter("review-fonts-list");
    big_font(&temp, 10);
    // u+4E03 and u+4E01, plus a duplicate; the build keeps the font's order.
    fs::write(temp.path("demo/assets/fonts/big.charlist"), "七丁\n丁\n").unwrap();
    let made = build(&rgbds(), &options).expect("builds");
    let keys: Vec<String> = made.images.unwrap().fonts[0]
        .glyphs
        .iter()
        .map(|g| g.key.clone())
        .collect();
    assert_eq!(keys, ["丁", "七"]);
    let inc = fs::read_to_string(temp.path("demo/build/images/images.inc")).unwrap();
    assert!(inc.contains("CHARMAP \"丁\", (\\2) + 0"), "{inc}");
    assert!(inc.contains("CHARMAP \"七\", (\\2) + 1"), "{inc}");
    assert!(!inc.contains("一"), "{inc}");
}

#[test]
fn a_charlist_over_the_cap_is_refused() {
    let (temp, options) = starter("review-fonts-list-over");
    big_font(&temp, 300);
    let all: String = (0..255)
        .map(|i| char::from_u32(0x4E00 + i).unwrap())
        .collect();
    fs::write(temp.path("demo/assets/fonts/big.charlist"), all).unwrap();
    let made = failed(build(&rgbds(), &options));
    let p = made
        .images
        .unwrap()
        .first_error()
        .cloned()
        .expect("refused");
    assert_eq!((p.count, p.max), (255, 254));
}

#[test]
fn no_images_no_fonts_and_an_old_palette_file_generate_nothing() {
    let (temp, options) = starter("review-fonts-nofont");
    crate::common::without_text(&temp.path("demo"));
    fs::write(
        temp.path("demo/assets/palette.txt"),
        "#FFFFFF #AAAAAA #555555 #000000\n",
    )
    .unwrap();
    let made = build(&rgbds(), &options).expect("builds");
    let images = made.images.expect("the warning is reported");
    assert!(images.first_error().is_none(), "{:#?}", images.problems);
    // Before cff86f7 this branch returned `generated: None`, so no
    // images.asm was written or linked and no include was reported.
    assert!(
        !temp.path("demo/build/images/images.asm").exists(),
        "a project with no images and no fonts now gets an images.asm"
    );
    assert_eq!(images.include, None);
}
