//! Probes from rev-fonts (review of 6feaf3e): malformed BDF must be a
//! typed problem, never a panic; an imported BDF must always write as a
//! YAFF the reader takes back; `.chars` must round-trip every row.

use gbcli_font::bdf::{read_bdf, write_bdf};
use gbcli_font::grid::{from_grid, to_grid};
use gbcli_font::{Font, Glyph, Key, read, read_chars, write, write_chars};

fn bdf(props: &str, chars: &str) -> String {
    format!("STARTFONT 2.1\nFONTBOUNDINGBOX 8 8 0 -1\n{props}CHARS 1\n{chars}ENDFONT\n")
}

fn one_char(bbx: &str) -> String {
    format!("STARTCHAR A\nENCODING 65\nDWIDTH 4 0\nBBX {bbx}\nBITMAP\n80\nENDCHAR\n")
}

/// `text` is refused as a problem with the format's remedy, not a panic
/// (in debug) or a misleading placement error (in release).
fn no_panic(text: &str) {
    let r = std::panic::catch_unwind(|| read_bdf(text, None));
    let problems = match r {
        Ok(Err(p)) => p,
        Ok(Ok(_)) => panic!("read_bdf took:\n{text}"),
        Err(_) => panic!("read_bdf panicked on:\n{text}"),
    };
    assert!(
        problems[0].remedy.contains("BDF 2.1"),
        "{problems:?}\n{text}"
    );
}

#[test]
fn a_huge_bbx_offset_is_a_problem_not_a_panic() {
    no_panic(&bdf("", &one_char("1 1 0 9223372036854775807")));
    no_panic(&bdf("", &one_char("1 1 9223372036854775807 0")));
    no_panic(&bdf("", &one_char("1 1 0 -9223372036854775808")));
}

#[test]
fn a_huge_ascent_is_a_problem_not_a_panic() {
    let props = "STARTPROPERTIES 1\nFONT_ASCENT -9223372036854775808\nENDPROPERTIES\n";
    no_panic(&bdf(props, &one_char("1 1 0 0")));
    let boxed = "STARTFONT 2.1\nFONTBOUNDINGBOX 8 9223372036854775807 0 1\nCHARS 1\n";
    no_panic(&format!("{boxed}{}ENDFONT\n", one_char("1 1 0 0")));
}

/// Every imported font is one gbcli's own YAFF writer and reader agree
/// on (font_import holds it to that, as an internal error).
fn imports_cleanly(props: &str) {
    let text = bdf(
        &format!("STARTPROPERTIES 1\n{props}\nENDPROPERTIES\n",),
        &one_char("1 1 0 0"),
    );
    let font = read_bdf(&text, None).expect("reads").font;
    let yaff = write(&font);
    let back = read(&yaff).unwrap_or_else(|e| panic!("{props}: {e:?}\n{yaff}"));
    assert_eq!(back, font, "{props}\n{yaff}");
}

#[test]
fn bdf_property_names_and_values_always_write_as_readable_yaff() {
    for p in [
        "FAMILY_NAME \"Talk\"",
        "0X41 \"a number for a name\"",
        "65 \"decimal\"",
        "U_0041 \"u-0041\"",
        "NOTES \"...\"",
        "NOTES \"'A':\"",
        "NOTES \"-\"",
        "NOTES \"@@@\"",
        "X \"  padded  \"",
    ] {
        imports_cleanly(p);
    }
}

fn c(ch: char) -> Option<Key> {
    Some(Key::Char(ch))
}

#[test]
fn chars_rows_round_trip_for_every_short_row_of_tricky_cells() {
    let alphabet: Vec<Option<Key>> = vec![
        c('<'),
        c('>'),
        c('u'),
        c('+'),
        c('4'),
        c('a'),
        c(' '),
        c('-'),
        Some(Key::Name("ab".into())),
        None,
    ];
    let n = alphabet.len();
    for len in 1..=4u32 {
        for mut i in 0..n.pow(len) {
            let mut row = Vec::new();
            for _ in 0..len {
                row.push(alphabet[i % n].clone());
                i /= n;
            }
            let last = row.iter().rposition(Option::is_some).map_or(0, |i| i + 1);
            let want = vec![row[..last].to_vec()];
            let text = write_chars(&[row.clone()]);
            let back = read_chars(&text).unwrap_or_else(|e| panic!("{row:?} -> {text:?}: {e:?}"));
            assert_eq!(back, want, "{row:?} -> {text:?}");
        }
    }
}

#[test]
fn a_byte_order_mark_glyph_first_in_the_grid_does_not_shift_every_key() {
    let g = |ch: char| Glyph {
        comment: vec![],
        key: Key::Char(ch),
        width: 3,
        rows: [0xA0; 8],
    };
    let font = Font {
        glyphs: vec![g('\u{FEFF}'), g('A'), g('B')],
        ..Font::default()
    };
    // Only probe if the key is one gbcli accepts at all.
    if Key::from_wire("\u{FEFF}").is_err() {
        return;
    }
    let grid = to_grid(&font, false);
    let back = from_grid(&grid.image, &grid.chars);
    assert_eq!(back.ok(), Some(font.glyphs), "chars: {:?}", grid.chars);
}

#[test]
fn bdf_round_trip_keeps_a_fonts_properties_as_they_were() {
    let text = "name: P\nfoo.bar: x\n\n'A':\n    @.\n    @.\n    @.\n    @.\n    @.\n    @.\n    @.\n    @.\n";
    let font = read(text).expect("reads");
    let back = read_bdf(&write_bdf(&font, "p"), None).expect("reads").font;
    let keys: Vec<&str> = back.properties.iter().map(|p| p.key.as_str()).collect();
    assert_eq!(back.glyphs, font.glyphs);
    // An exported BDF read back names its properties as the font did,
    // and adds none.
    assert_eq!(keys, ["name", "foo.bar"], "{:?}", back.properties);
}
