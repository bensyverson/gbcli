//! Short blocks: a `pattern` line may name fewer than four columns, and `=`
//! marks a column whose pattern another block shows, so a pattern several
//! orders play is written once. Input only: `to_text` stays canonical.

use crate::common;

use gbcli_music::{EXAMPLE, FORMAT_LINE, ParseError, parse_text, rgbds_asm, to_text};

const BLANK: &str = "... .. ... ...";
const BLANK4: &str = "... .. ... ... | ... .. ... ... | ... .. ... ... | ... .. ... ...";

/// The canonical text with each pattern written once: a column whose
/// pattern an earlier block showed becomes `=`, trailing `=` columns are
/// dropped (`trim`) or kept with blank cells, and so are their cells.
fn shorten(canonical: &str, trim: bool) -> String {
    let mut shown = std::collections::BTreeSet::new();
    // Per column of the open block: keep its cells, blank them, or drop them.
    let mut keep: Vec<Option<bool>> = Vec::new();
    let mut out = String::new();
    for line in canonical.lines() {
        if let Some(rest) = line.strip_prefix("pattern ") {
            let mut names: Vec<String> = Vec::new();
            keep.clear();
            for name in rest.split(' ') {
                if name != "-" && !shown.insert(name.to_string()) {
                    names.push("=".into());
                    keep.push(Some(false));
                } else {
                    names.push(name.into());
                    keep.push(Some(true));
                }
            }
            if trim {
                while names.len() > 1 && names.last().is_some_and(|n| n == "=") {
                    names.pop();
                    *keep.iter_mut().rev().find(|k| k.is_some()).unwrap() = None;
                }
            }
            out += &format!("pattern {}\n", names.join(" "));
            continue;
        }
        let is_row = !keep.is_empty() && line.as_bytes().first().is_some_and(u8::is_ascii_digit);
        if !is_row {
            if line.is_empty() {
                keep.clear();
            }
            out += line;
            out.push('\n');
            continue;
        }
        let (number, cells) = line.split_once(' ').unwrap();
        let cells: Vec<&str> = cells
            .split(" | ")
            .zip(&keep)
            .filter_map(|(cell, k)| match k {
                Some(true) => Some(cell),
                Some(false) => Some(BLANK),
                None => None,
            })
            .collect();
        out += &format!("{number} {}\n", cells.join(" | "));
    }
    out
}

fn error(text: &str) -> ParseError {
    match parse_text(text) {
        Ok(_) => panic!("parsed:\n{text}"),
        Err(e) => e,
    }
}

fn check(e: &ParseError, line: u32, column: u32, message: &str, remedy: &str) {
    assert_eq!((e.line, e.column), (line, column), "{e:?}");
    assert!(e.message.contains(message), "message: {}", e.message);
    assert!(e.remedy.contains(remedy), "remedy: {}", e.remedy);
}

/// Each short form of `canonical` reads as the same song, writes the same
/// canonical text and emits the same driver data.
fn assert_short_forms_match(name: &str, canonical: &str) {
    let song = parse_text(canonical).unwrap();
    let asm = rgbds_asm(&song, "song_t");
    for trim in [true, false] {
        let short = shorten(canonical, trim);
        let parsed = parse_text(&short).unwrap_or_else(|e| panic!("{name}: {e}\n{short}"));
        assert!(parsed == song, "{name} (trim {trim}): a different song");
        assert_eq!(to_text(&parsed), canonical, "{name} (trim {trim})");
        assert!(
            rgbds_asm(&parsed, "song_t") == asm,
            "{name} (trim {trim}): different driver data"
        );
    }
}

#[test]
fn the_examples_short_form_is_the_same_song() {
    let canonical = to_text(&parse_text(EXAMPLE).unwrap());
    let short = shorten(&canonical, true);
    // The second block shows only its lead; P1-P3 are written once.
    assert!(short.contains("\npattern P4\n"), "{short}");
    assert_short_forms_match("example", &canonical);
}

#[test]
fn every_samples_short_form_is_the_same_song() {
    for s in common::samples() {
        let song = gbcli_music::read_uge(&s.bytes).unwrap().song;
        assert_short_forms_match(&s.name, &to_text(&song));
    }
}

#[test]
fn a_one_column_block_reads_one_cell_a_row() {
    let text = format!(
        "{FORMAT_LINE}\norders\n00 P0 P1 P2 P3\n01 P4 P1 P2 P3\n\
         pattern P0 P1 P2 P3\n00 {BLANK4}\n\
         pattern P4\n00 C-5 01 ... ...\n"
    );
    let song = parse_text(&text).unwrap();
    assert_eq!(song.pattern(4).unwrap().rows[0].note, 24);
    assert_eq!(song.orders[0], [0, 4]);
    // The same block with = for the columns it doesn't show.
    let equals = text.replace(
        "pattern P4\n00 C-5 01 ... ...",
        &format!("pattern P4 = = =\n00 C-5 01 ... ... | {BLANK} | {BLANK} | {BLANK}"),
    );
    assert!(parse_text(&equals).unwrap() == song);
}

#[test]
fn an_equals_column_may_come_first() {
    let text = format!(
        "{FORMAT_LINE}\norders\n00 P0 P1 P2 P3\n01 P0 P5 P2 P3\n\
         pattern P0 P1 P2 P3\n00 {BLANK4}\n\
         pattern = P5\n00 {BLANK} | E-4 02 ... ...\n"
    );
    let song = parse_text(&text).unwrap();
    assert_eq!(song.pattern(5).unwrap().rows[0].instrument, 2);
}

#[test]
fn a_row_must_have_one_cell_per_column_the_block_names() {
    let text = format!(
        "{FORMAT_LINE}\norders\n00 P0 P1 P2 P3\n\
         pattern P0 P1 P2 P3\n00 {BLANK4}\n\
         pattern P9\n00 C-5 01 ... ... | {BLANK}\n"
    );
    let e = error(&text);
    check(&e, 7, 1, "2 cells", "one cell");
    // A four-column block still asks for four.
    let e = error(&format!(
        "{FORMAT_LINE}\npattern P0 P1 P2 P3\n00 C-5 01 ... ... | {BLANK}\n"
    ));
    check(&e, 3, 1, "2 cells", "four cells");
}

#[test]
fn a_cell_under_equals_must_be_blank() {
    let e = error(&format!(
        "{FORMAT_LINE}\norders\n00 P0 P1 P2 P3\n\
         pattern P0 P1 P2 P3\n00 {BLANK4}\n\
         pattern P0 =\n00 {BLANK} | C-5 01 ... ...\n"
    ));
    check(&e, 7, 21, "shown in another block (=)", "blank");
}

#[test]
fn an_equals_pattern_still_needs_a_block_that_shows_it() {
    let e = error(&format!(
        "{FORMAT_LINE}\norders\n00 P0 P1 P2 P3\npattern P0 = P2 P3\n00 {BLANK4}\n"
    ));
    check(&e, 3, 7, "P1", "pattern block");
}

#[test]
fn a_pattern_line_names_one_to_four_columns() {
    let e = error(&format!("{FORMAT_LINE}\npattern\n"));
    check(&e, 2, 1, "1-4", "pattern P0 P1 P2 P3");
    let e = error(&format!("{FORMAT_LINE}\npattern P0 P1 P2 P3 P4\n"));
    check(&e, 2, 1, "1-4", "pattern P0 P1 P2 P3");
    // A full line still reads.
    assert!(
        parse_text(&format!(
            "{FORMAT_LINE}\npattern P0 P1 P2 P3\n00 {BLANK4}\n"
        ))
        .is_ok()
    );
}

#[test]
fn equals_is_not_a_pattern_in_the_orders() {
    let e = error(&format!(
        "{FORMAT_LINE}\norders\n00 P0 = P2 P3\npattern P0 P1 P2 P3\n00 {BLANK4}\n"
    ));
    check(&e, 3, 7, "`=` is not a pattern name", "P0");
}
