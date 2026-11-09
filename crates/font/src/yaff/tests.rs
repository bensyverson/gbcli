use super::*;
use crate::key::Key;
use crate::model::Property;

/// A small font in the subset, written as the canonical writer writes it.
const SMALL: &str = "\
# A test font.
# Two lines of comment.

yaff: 1.0.4
name: Small
notice:
    First line.
    Second line.

# The space: 3 px of paper.
u+0020:
    ...
    ...
    ...
    ...
    ...
    ...
    ...
    ...

'i':
    ..
    @.
    ..
    @.
    @.
    @.
    @.
    ..

\"heart\":
    .......@
    .@@.@@..
    @@@@@@@.
    @@@@@@@.
    .@@@@@..
    ..@@@...
    ...@....
    ........
";

fn refused(text: &str) -> FontError {
    read(text).expect_err("refused")
}

/// A font whose one glyph is `rows`, keyed 'A', with `head` before it.
fn one(head: &str, label: &str, rows: &[&str]) -> String {
    let mut s = String::from(head);
    s += label;
    s += ":\n";
    for r in rows {
        s += "    ";
        s += r;
        s += "\n";
    }
    s
}

const EIGHT: [&str; 8] = ["@.", "@.", "@.", "@.", "@.", "@.", "@.", ".."];

#[test]
fn a_font_reads_its_comments_properties_and_glyphs_in_file_order() {
    let font = read(SMALL).expect("reads");
    assert_eq!(font.comment, ["A test font.", "Two lines of comment."]);
    assert_eq!(
        font.properties,
        [
            Property {
                comment: vec![],
                key: "yaff".into(),
                value: "1.0.4".into()
            },
            Property {
                comment: vec![],
                key: "name".into(),
                value: "Small".into()
            },
            Property {
                comment: vec![],
                key: "notice".into(),
                value: "First line.\nSecond line.".into()
            },
        ]
    );
    let keys: Vec<Key> = font.glyphs.iter().map(|g| g.key.clone()).collect();
    assert_eq!(
        keys,
        [Key::Char(' '), Key::Char('i'), Key::Name("heart".into())]
    );
    assert_eq!(font.glyphs[0].comment, ["The space: 3 px of paper."]);
    let widths: Vec<u8> = font.glyphs.iter().map(|g| g.width).collect();
    assert_eq!(widths, [3, 2, 8]);
    assert_eq!(font.glyphs[1].rows, [0, 0x80, 0, 0x80, 0x80, 0x80, 0x80, 0]);
    assert_eq!(font.glyphs[2].rows[0], 0x01);
    assert_eq!(font.glyphs[2].rows[2], 0xFE);
    assert!(!font.fixed());
    assert_eq!(font.widths(), Some((2, 8)));
    assert_eq!(font.find(&Key::Char('i')), Some(1));
}

#[test]
fn the_writer_is_canonical_and_round_trips() {
    let font = read(SMALL).expect("reads");
    assert_eq!(write(&font), SMALL);
    assert_eq!(read(&write(&font)).expect("reads back"), font);
    // Any accepted spelling writes canonically: tabs, CRLF, a BOM, extra
    // blank lines, trailing spaces and a lower-case code point.
    let loose = "\u{FEFF}name: X\r\n\r\n\r\nu+0041:\r\n\t@.  \r\n\t@.\r\n\t@.\r\n\t@.\r\n\t@.\r\n\
                 \t@.\r\n\t@.\r\n\t..\r\n";
    let font = read(loose).expect("reads");
    assert_eq!(
        write(&font),
        "name: X\n\n'A':\n    @.\n    @.\n    @.\n    @.\n    @.\n    @.\n    @.\n    ..\n"
    );
    // A font with nothing but glyphs, and an empty file.
    let bare = one("", "'A'", &EIGHT);
    assert_eq!(write(&read(&bare).unwrap()), bare);
    assert_eq!(write(&read("").unwrap()), "");
}

#[test]
fn comments_before_properties_and_glyphs_and_after_the_last_are_kept() {
    let text = "# global\n\n# about the name\nname: X\n\n'A':\n    @\n    @\n    @\n    @\n    @\n    \
                @\n    @\n    @\n\n# the end\n";
    let font = read(text).expect("reads");
    assert_eq!(font.comment, ["global"]);
    assert_eq!(font.properties[0].comment, ["about the name"]);
    assert_eq!(font.tail, ["the end"]);
    assert_eq!(write(&font), text);
    // With no blank line after it, the first comment is the property's.
    let font = read("# about the name\nname: X\n").unwrap();
    assert!(font.comment.is_empty());
    assert_eq!(font.properties[0].comment, ["about the name"]);
}

#[test]
fn a_glyph_is_eight_rows_of_one_to_eight_pixels() {
    let e = refused(&one("", "'A'", &EIGHT[..7]));
    assert_eq!(e.line, 2);
    assert!(e.message.contains("7 rows"), "{}", e.message);
    let e = refused(&one("", "'A'", &["@@@@@@@@@"; 8]));
    assert!(e.message.contains("9 pixels wide"), "{}", e.message);
    let e = refused(&one(
        "",
        "'A'",
        &["@.", "@.", "@", "@.", "@.", "@.", "@.", ".."],
    ));
    assert_eq!((e.line, e.column), (4, 5));
    assert!(e.message.contains("1 pixel"), "{}", e.message);
    let e = refused(&one(
        "",
        "'A'",
        &["@.", "@x", "@.", "@.", "@.", "@.", "@.", ".."],
    ));
    assert_eq!((e.line, e.column), (3, 6));
    let e = refused(&one("", "'A'", &["-"]));
    assert!(e.message.contains("empty glyph"), "{}", e.message);
}

#[test]
fn rows_share_one_indent() {
    let text = "'A':\n    @\n    @\n   @\n    @\n    @\n    @\n    @\n    @\n";
    let e = refused(text);
    assert_eq!(e.line, 4);
    assert!(e.message.contains("indent"), "{}", e.message);
}

#[test]
fn a_glyph_has_one_label_and_each_key_one_glyph() {
    let mut text = one("", "'A'", &EIGHT);
    text.insert_str(5, "\"latin_a\":\n");
    let e = refused(&text);
    assert_eq!(e.line, 2);
    assert!(e.message.contains("second label"), "{}", e.message);
    let twice = format!("{}\n{}", one("", "'A'", &EIGHT), one("", "u+0041", &EIGHT));
    let e = refused(&twice);
    assert_eq!(e.line, 11);
    assert!(e.message.contains("line 1"), "{}", e.message);
}

#[test]
fn a_label_outside_the_subset_is_refused_at_its_line() {
    let e = refused(&one("name: X\n\n", "0x41", &EIGHT));
    assert_eq!(e.line, 3);
    assert!(e.message.contains("codepoint label"));
    let e = refused(&one("", "u+0065, u+0301", &EIGHT));
    assert!(e.remedy.contains("'é'"), "{}", e.remedy);
}

#[test]
fn per_glyph_properties_and_bearings_are_refused() {
    let mut text = one("", "'A'", &EIGHT);
    text += "\n    right-bearing: 1\n";
    let e = refused(&text);
    assert_eq!(e.line, 11);
    assert!(e.message.contains("right-bearing"), "{}", e.message);
    let e = refused(&one("left-bearing: 1\n\n", "'A'", &EIGHT));
    assert_eq!(e.line, 1);
    assert!(e.remedy.contains("drawn into the glyph"), "{}", e.remedy);
    // A bearing of 0 changes nothing, so it reads.
    assert!(read(&one("left-bearing: 0\nright-bearing: 0\n\n", "'A'", &EIGHT)).is_ok());
}

#[test]
fn properties_come_first_once_each_and_the_version_is_1() {
    let mut text = one("", "'A'", &EIGHT);
    text += "\nname: late\n";
    let e = refused(&text);
    assert_eq!(e.line, 11);
    assert!(e.message.contains("before the glyphs"), "{}", e.message);
    let e = refused("name: a\nName: b\n");
    assert_eq!(e.line, 2);
    let e = refused("yaff: 2.0\n");
    assert!(e.message.contains("2.0"), "{}", e.message);
}

#[test]
fn malformed_lines_are_refused_where_they_are() {
    let e = refused("name: X\n    stray\n");
    assert_eq!(e.line, 2);
    let e = refused("na me: X\n");
    assert_eq!(e.line, 1);
    let e = refused("name: X\u{7}\n");
    assert!(e.message.contains("control character"), "{}", e.message);
    // A label with nothing under it.
    let e = refused("'A':\n\n'B':\n");
    assert_eq!(e.line, 1);
}
