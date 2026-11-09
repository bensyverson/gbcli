//! gbcli's YAFF reader against monobit's published samples (Ben's ruling
//! 1; the files and their licenses: crates/font/data/monobit/SOURCE).

use gbcli_font::yaff::syntax;

fn sample(name: &str) -> String {
    let path = format!("{}/data/monobit/{name}", env!("CARGO_MANIFEST_DIR"));
    std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("{path}: {e}"))
}

#[test]
fn the_grammar_reads_4x6_whole() {
    let doc = syntax::parse(&sample("4x6.yaff")).expect("4x6.yaff is YAFF");
    assert_eq!(doc.comment.len(), 3, "{:?}", doc.comment);
    assert!(doc.comment[0].starts_with("Contributed by Janne V. Kujala"));
    assert_eq!(doc.properties[0].key, "name");
    assert_eq!(doc.properties[0].value, "Fixed Medium 6");
    assert_eq!(doc.properties.len(), 23);
    assert_eq!(doc.glyphs.len(), 919);
    // Every glyph is 6 rows of 4, under a code point and a tag.
    for g in &doc.glyphs {
        assert_eq!(g.rows.len(), 6, "line {}", g.rows_line);
        assert!(g.rows.iter().all(|r| r.len() == 4));
        assert_eq!(g.labels.len(), 2, "line {}", g.rows_line);
    }
    let a = doc
        .glyphs
        .iter()
        .find(|g| g.labels[0].text == "u+0041")
        .expect("A");
    assert_eq!(a.labels[0].line, 335);
    assert_eq!(a.labels[1].text, "\"A\"");
    assert_eq!(a.rows, [".@..", "@.@.", "@@@.", "@.@.", "@.@.", "...."]);
}

#[test]
fn the_strict_reader_refuses_4x6_at_its_first_glyph() {
    // u+0000 is a control character, which no text draws.
    let e = gbcli_font::read(&sample("4x6.yaff")).expect_err("not the subset");
    assert_eq!(e.line, 29);
    assert!(e.message.contains("control character"), "{}", e.message);
}

#[test]
fn the_grammar_reads_webby_small_kerned_whole() {
    let doc = syntax::parse(&sample("webby-small-kerned.yaff")).expect("is YAFF");
    // The header comment is the MIT license, kept as written.
    assert!(doc.comment.iter().any(|l| l == "MIT License"));
    assert_eq!(doc.glyphs.len(), 96);
    let bdf = doc.properties.iter().find(|p| p.key == "bdf").expect("bdf");
    assert_eq!(bdf.value, "WEIGHT: 10\nQUAD_WIDTH: 7");
    let a = doc
        .glyphs
        .iter()
        .find(|g| g.labels[0].text == "0x41")
        .expect("A");
    assert_eq!(a.rows.len(), 9);
    assert_eq!(a.rows[2], ".@@@@..");
    let keys: Vec<&str> = a.properties.iter().map(|p| p.key.as_str()).collect();
    assert_eq!(keys, ["left-bearing", "right-bearing", "shift-up"]);
    assert_eq!(a.properties[2].value, "-2");
    // Kerning is a per-glyph property of several lines.
    let kerned = doc
        .glyphs
        .iter()
        .flat_map(|g| &g.properties)
        .find(|p| p.line == 1185)
        .expect("the kerning at line 1185");
    assert_eq!(kerned.key, "right-kerning");
    assert_eq!(kerned.value, "0x69 -2\n0x6a -1\n0x66 -1\n0x67 -1");
}

#[test]
fn the_strict_reader_refuses_webby_at_its_first_codepoint_label() {
    let text = sample("webby-small-kerned.yaff");
    let e = gbcli_font::read(&text).expect_err("not the subset");
    let line = text.lines().position(|l| l == "0x20:").unwrap() as u32 + 1;
    assert_eq!(e.line, line);
    assert!(e.message.contains("codepoint label"), "{}", e.message);
}
