//! Edge cases from review-fonts (cff86f7): the strict reader's edges, both ways,
//! and round trips.

use gbcli_font::{Key, read, write};

fn glyph(label: &str, rows: &[&str]) -> String {
    let mut out = format!("{label}:\n");
    for r in rows {
        out += &format!("    {r}\n");
    }
    out
}

const EIGHT: [&str; 8] = ["@."; 8];

fn one(label: &str) -> String {
    format!("name: P\n\n{}", glyph(label, &EIGHT))
}

fn refused(text: &str) -> String {
    match read(text) {
        Ok(f) => panic!("accepted:\n{text}\n{f:?}"),
        Err(e) => e.message,
    }
}

#[test]
fn out_of_subset_files_are_refused() {
    // Height.
    refused(&glyph("'A'", &["@"; 7]));
    refused(&glyph("'A'", &["@"; 9]));
    // Width.
    refused(&glyph("'A'", &["@@@@@@@@@"; 8]));
    // Bearings, tracking, globally.
    refused(&format!("left-bearing: 1\n\n{}", glyph("'A'", &EIGHT)));
    refused(&format!("right_bearing: -1\n\n{}", glyph("'A'", &EIGHT)));
    refused(&format!("tracking: 2\n\n{}", glyph("'A'", &EIGHT)));
    // Kerning and per-glyph bearings.
    refused(&format!(
        "{}\n    right-kerning:\n        'B' -1\n",
        glyph("'A'", &EIGHT)
    ));
    refused(&format!("{}\n    left-bearing: 1\n", glyph("'A'", &EIGHT)));
    // Two labels.
    refused(&format!("'A':\n{}", glyph("'B'", &EIGHT)));
    // Decomposed keys the table knows, in every spelling.
    let m = refused(&one("'e', u+0301"));
    assert!(m.contains("decomposed"), "{m}");
    let m = refused(&one("u+0065, u+0301"));
    assert!(m.contains("decomposed"), "{m}");
    let m = refused(&one("'e\u{301}'"));
    assert!(m.contains("decomposed"), "{m}");
    // A singleton (Angstrom sign is canonically Å).
    refused(&one("u+212B"));
    // Codepoint label, unquoted label, empty glyph, duplicate key.
    refused(&one("0x41"));
    refused(&one("A"));
    refused("'A':\n    -\n");
    refused(&format!(
        "{}\n{}",
        glyph("'A'", &EIGHT),
        glyph("u+0041", &EIGHT)
    ));
    // A control character key.
    refused(&one("u+0009"));
    // YAFF 2.
    refused(&format!("yaff: 2.0\n\n{}", glyph("'A'", &EIGHT)));
}

#[test]
fn valid_subset_files_are_accepted() {
    for label in [
        "'A'",
        "u+0041",
        "U+0041",
        "u+1F600",
        "'é'",
        "u+00E9",
        "\"heart\"",
        "\"btn_a-2\"",
        "' '",
        "':'",
        "','",
        "'''",
        "'\"'",
        "'#'",
        "'<'",
        "u+0301",
    ] {
        let f = read(&one(label)).unwrap_or_else(|e| panic!("{label}: {e:?}"));
        assert_eq!(f.glyphs.len(), 1, "{label}");
    }
    // Zero bearings and a YAFF 1.0.4 version line are allowed.
    read(&format!(
        "yaff: 1.0.4\nleft-bearing: 0\ntracking: 0\n\n{}",
        glyph("'A'", &EIGHT)
    ))
    .unwrap();
    // A property value of several lines.
    read(&format!(
        "notice:\n    Line one\n    line two\n\n{}",
        glyph("'A'", &EIGHT)
    ))
    .unwrap();
}

#[test]
fn a_property_value_starting_with_dots_is_not_glyph_rows() {
    // A value line that only starts with `.` is text, not rows: rows hold
    // nothing but `.` and `@`. (`name: Foo:` is ambiguous YAFF; not pinned.)
    let dots = read(&format!(
        "notice:\n    ...and more\n\n{}",
        glyph("'A'", &EIGHT)
    ));
    assert!(dots.is_ok(), "{:?}", dots.err().map(|e| e.message));
}

#[test]
fn read_write_round_trips() {
    let text = "# Top comment\n#\n#  indented\n\nname: RT\nnotice:\n    a\n    b\n# a property comment\nfamily: X\n\n# the space\nu+0020:\n    ...\n    ...\n    ...\n    ...\n    ...\n    ...\n    ...\n    ...\n\n'@':\n    @.\n    .@\n    @.\n    .@\n    @.\n    .@\n    @.\n    .@\n\n\"heart\":\n    @\n    @\n    @\n    @\n    @\n    @\n    @\n    @\n\nu+0027:\n    @\n    @\n    @\n    @\n    @\n    @\n    @\n    @\n\nu+0301:\n    @\n    @\n    @\n    @\n    @\n    @\n    @\n    @\n\n# tail\n";
    let font = read(text).unwrap();
    let out = write(&font);
    assert_eq!(out, text, "canonical text writes back byte for byte");
    assert_eq!(read(&out).unwrap(), font);
    // Every non-control BMP key a font can hold round-trips through its
    // label.
    for c in (0x20u32..0x3000).filter_map(char::from_u32) {
        let Ok(key) = Key::from_wire(&c.to_string()) else {
            continue;
        };
        let label = key.label();
        match Key::from_label(&label) {
            Ok(back) => assert_eq!(back, key, "u+{:04X} via {label}", c as u32),
            Err(e) => panic!(
                "u+{:04X}: {label} doesn't read back: {}",
                c as u32, e.message
            ),
        }
    }
}
