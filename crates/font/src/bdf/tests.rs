use super::*;
use crate::key::Key;
use crate::model::{Font, Glyph, HEIGHT, Property};
use crate::yaff;

fn glyph(key: Key, width: u8, rows: [u8; HEIGHT]) -> Glyph {
    Glyph {
        comment: vec![],
        key,
        width,
        rows,
    }
}

fn prop(key: &str, value: &str) -> Property {
    Property {
        comment: vec![],
        key: key.into(),
        value: value.into(),
    }
}

fn font() -> Font {
    Font {
        comment: vec!["A test font.".into()],
        properties: vec![
            prop("family", "Talk"),
            prop("ascent", "6"),
            prop("descent", "2"),
            prop("notice", "Free: use it \"as is\""),
        ],
        glyphs: vec![
            glyph(Key::Char(' '), 3, [0; 8]),
            glyph(Key::Char('A'), 5, [0, 0x70, 0x88, 0xF8, 0x88, 0x88, 0, 0]),
            glyph(Key::Char('g'), 4, [0, 0, 0, 0x60, 0x90, 0x70, 0x10, 0x60]),
            glyph(
                Key::Name("heart".into()),
                8,
                [0x6C, 0xFE, 0xFE, 0x7C, 0x38, 0x10, 0, 0x81],
            ),
            glyph(Key::Char('\u{301}'), 3, [0x20, 0x40, 0, 0, 0, 0, 0, 0]),
        ],
        tail: vec![],
    }
}

/// `text` written by gbcli's YAFF writer reads back as the same font.
fn reads_back(font: &Font) {
    let text = yaff::write(font);
    let back = yaff::read(&text).unwrap_or_else(|e| panic!("{e:?}\n{text}"));
    assert_eq!(&back, font, "{text}");
}

#[test]
fn a_font_round_trips_through_bdf_glyphs_and_properties() {
    let bdf = write_bdf(&font(), "talk");
    assert!(bdf.starts_with("STARTFONT 2.1\n"), "{bdf}");
    assert!(bdf.contains("FONT_ASCENT 6\n"), "{bdf}");
    assert!(
        bdf.contains("ENCODING -1\n"),
        "a named glyph is unencoded: {bdf}"
    );
    let imported = read_bdf(&bdf, None).unwrap();
    assert_eq!(imported.font.glyphs, font().glyphs);
    assert_eq!(imported.font.comment, font().comment);
    for p in &font().properties {
        assert_eq!(
            imported.font.property(&p.key),
            Some(p.value.as_str()),
            "{}",
            p.key
        );
    }
    assert!(imported.skipped.is_empty() && imported.missing.is_empty());
    reads_back(&imported.font);
}

/// Misaki's shape: trimmed glyphs placed by their bounding box and the
/// font's ascent, ASCII half width.
const TRIMMED: &str = "\
STARTFONT 2.1
COMMENT trimmed like Misaki
FONT -Test-Trim-Medium-R-Normal--8-80-75-75-C-80-ISO10646-1
SIZE 8 75 75
FONTBOUNDINGBOX 8 8 0 -2
STARTPROPERTIES 6
FONT_ASCENT 6
FONT_DESCENT 2
CHARSET_REGISTRY \"ISO10646\"
CHARSET_ENCODING \"1\"
COPYRIGHT \"Public domain\"
WEIRD_VALUE \"ends with:\"
ENDPROPERTIES
CHARS 4
STARTCHAR A
ENCODING 65
SWIDTH 500 0
DWIDTH 4 0
BBX 3 5 0 0
BITMAP
40
A0
E0
A0
A0
ENDCHAR
STARTCHAR g
ENCODING 103
SWIDTH 500 0
DWIDTH 4 0
BBX 3 4 0 -2
BITMAP
60
A0
60
C0
ENDCHAR
STARTCHAR period
ENCODING 46
SWIDTH 250 0
DWIDTH 2 0
BBX 1 1 0 0
BITMAP
80
ENDCHAR
STARTCHAR a.alt
ENCODING -1
SWIDTH 500 0
DWIDTH 4 0
BBX 1 1 0 0
BITMAP
80
ENDCHAR
ENDFONT
";

#[test]
fn trimmed_glyphs_are_placed_by_bounding_box_and_ascent() {
    let imported = read_bdf(TRIMMED, None).unwrap();
    let f = &imported.font;
    assert_eq!(f.glyphs.len(), 3);
    let a = &f.glyphs[0];
    assert_eq!((a.key.clone(), a.width), (Key::Char('A'), 4));
    // Rows 1-5 (the top of a 5-row box ending on the baseline, ascent 6).
    assert_eq!(a.rows, [0, 0x40, 0xA0, 0xE0, 0xA0, 0xA0, 0, 0]);
    // g descends 2 rows below the baseline: rows 4-7.
    assert_eq!(f.glyphs[1].rows, [0, 0, 0, 0, 0x60, 0xA0, 0x60, 0xC0]);
    assert_eq!(f.glyphs[2].rows, [0, 0, 0, 0, 0, 0x80, 0, 0]);
    assert_eq!(imported.skipped, vec!["a.alt".to_string()]);
    assert_eq!(f.property("copyright"), Some("Public domain"));
    assert_eq!(f.property("weird-value"), Some("ends with:"));
    assert_eq!(f.property("ascent"), Some("6"));
    reads_back(f);
}

#[test]
fn a_character_list_limits_the_import_and_names_what_it_lacks() {
    let only = [Key::Char('g'), Key::Char('A'), Key::Char('¿')];
    let imported = read_bdf(TRIMMED, Some(&only)).unwrap();
    let keys: Vec<Key> = imported.font.glyphs.iter().map(|g| g.key.clone()).collect();
    assert_eq!(
        keys,
        vec![Key::Char('A'), Key::Char('g')],
        "the file's order"
    );
    assert_eq!(imported.missing, vec![Key::Char('¿')]);
}

#[test]
fn glyphs_a_gbcli_font_cant_hold_are_problems_naming_the_glyph() {
    let wide = TRIMMED.replace("DWIDTH 4 0\nBBX 3 5 0 0", "DWIDTH 9 0\nBBX 3 5 0 0");
    let p = read_bdf(&wide, None).unwrap_err();
    assert_eq!(p.len(), 1, "{p:#?}");
    assert_eq!(p[0].glyph, Some(Key::Char('A')));
    assert!(p[0].message.contains('9'), "{}", p[0].message);
    let tall = TRIMMED.replace("BBX 3 5 0 0", "BBX 3 5 0 2");
    let p = read_bdf(&tall, None).unwrap_err();
    assert!(p[0].message.contains("above"), "{}", p[0].message);
    let past = TRIMMED.replace("BBX 3 5 0 0", "BBX 3 5 2 0");
    let p = read_bdf(&past, None).unwrap_err();
    assert!(p[0].message.contains("right"), "{}", p[0].message);
    let jis = TRIMMED.replace("\"ISO10646\"", "\"JISX0208.1983\"");
    let p = read_bdf(&jis, None).unwrap_err();
    assert!(p[0].message.contains("JISX0208"), "{}", p[0].message);
    let p = read_bdf("STARTFONT 2.1\nCHARS 1\nSTARTCHAR x\n", None).unwrap_err();
    assert!(p[0].line.is_some(), "{p:#?}");
}

#[test]
fn imported_properties_are_always_ones_the_yaff_reader_takes() {
    let props = "STARTPROPERTIES 5\nFONT_ASCENT 6\nEMPTY \"\"\nCOLON \"a:\"\nDOTS \"...\"\n\
                 X-ODD.KEY 3\nENDPROPERTIES\n";
    let text = TRIMMED.replace(
        &TRIMMED[TRIMMED.find("STARTPROPERTIES").unwrap()..TRIMMED.find("CHARS 4").unwrap()],
        props,
    );
    let imported = read_bdf(&text, None).unwrap();
    let f = &imported.font;
    assert_eq!(f.property("empty"), None);
    assert_eq!(f.property("colon"), Some("a:"));
    assert_eq!(f.property("dots"), Some("..."));
    reads_back(f);
}
