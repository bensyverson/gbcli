//! The text form: what it accepts, how it spells things, and the errors it
//! gives.

use gbcli_music::{
    BANK_SIZE, Cell, EXAMPLE, FORMAT_LINE, ParseError, ROWS, Song, parse_text, to_text,
};

/// A song with one pattern block, rows given as (row, cells) lines.
fn song_with(rows: &str) -> String {
    format!("{FORMAT_LINE}\norders\n00 P0 P1 P2 P3\npattern P0 P1 P2 P3\n{rows}")
}

fn error(text: &str) -> ParseError {
    match parse_text(text) {
        Ok(_) => panic!("parsed:\n{text}"),
        Err(e) => e,
    }
}

const BLANK4: &str = "... .. ... ... | ... .. ... ... | ... .. ... ... | ... .. ... ...";

#[test]
fn the_example_parses_and_its_canonical_text_is_a_fixpoint() {
    let song = parse_text(EXAMPLE).unwrap();
    assert!(!song.patterns.is_empty());
    let text = to_text(&song);
    assert!(parse_text(&text).unwrap() == song);
    assert_eq!(to_text(&parse_text(&text).unwrap()), text);
}

#[test]
fn rows_left_out_are_blank_and_the_canonical_text_writes_all_64() {
    let song = parse_text(&song_with(
        "04 C-5 01 ... 2FF | ... .. ... ... | ... .. ... ... | ... .. ... ...\n",
    ))
    .unwrap();
    let p0 = song.pattern(0).unwrap();
    assert!(p0.rows[0].is_blank());
    assert_eq!(
        p0.rows[4],
        Cell {
            note: 24,
            instrument: 1,
            volume: 0,
            effect_code: 2,
            effect_param: 0xFF
        }
    );
    let text = to_text(&song);
    let block: Vec<&str> = text
        .lines()
        .skip_while(|l| !l.starts_with("pattern "))
        .skip(1)
        .take_while(|l| !l.is_empty())
        .collect();
    assert_eq!(block.len(), ROWS);
    assert_eq!(block[0], format!("00 {BLANK4}"));
    assert!(block[4].starts_with("04 C-5 01 ... 2FF | "), "{}", block[4]);
    assert!(block[63].starts_with("63 "));
}

#[test]
fn a_new_song_takes_hugetrackers_defaults_for_what_it_leaves_out() {
    let song = parse_text(&format!("{FORMAT_LINE}\n")).unwrap();
    assert!(song == Song::default());
    assert_eq!(song.ticks_per_row, 7);
}

#[test]
fn an_instrument_reads_its_fields_and_defaults_the_rest() {
    let song = parse_text(&format!(
        "{FORMAT_LINE}\n\
         instrument square 2 \"Lead\"\n\
         \x20 envelope 12 up 3\n\
         \x20 duty 75%\n\
         \x20 length 32\n\
         instrument noise 1 \"Hat\"\n\
         \x20 mode 7-bit\n\
         \x20 subpattern on\n\
         \x20 00 +00 .. ... ...\n\
         \x20 01 -02 .. J01 C08\n"
    ))
    .unwrap();
    let lead = &song.instruments[1];
    assert_eq!(lead.name.bytes(), b"Lead");
    assert_eq!(
        (
            lead.initial_volume,
            lead.volume_direction,
            lead.volume_pace,
            lead.duty
        ),
        (12, 0, 3, 3)
    );
    assert_eq!((lead.length_enabled, lead.length), (1, 32));
    assert_eq!(lead.sweep_direction, 1, "left out: the default");
    let hat = &song.instruments[2 * BANK_SIZE];
    assert_eq!((hat.counter_step, hat.subpattern_enabled), (1, 1));
    assert_eq!(hat.subpattern[0].note, 36);
    assert_eq!(
        hat.subpattern[1],
        Cell {
            note: 34,
            instrument: 0,
            volume: 1,
            effect_code: 0xC,
            effect_param: 0x08
        }
    );
}

#[test]
fn an_out_of_range_stored_value_round_trips_as_a_bang_number() {
    let mut song = Song::default();
    song.instruments[0].initial_volume = 63;
    song.instruments[0].output_level = 7;
    song.patterns.push(gbcli_music::Pattern {
        key: 0,
        rows: [Cell {
            volume: 5,
            note: 80,
            ..Cell::BLANK
        }; ROWS],
    });
    song.orders = [vec![0], vec![0], vec![0], vec![0]];
    let text = to_text(&song);
    assert!(text.contains("envelope !63 down 0"), "{text}");
    assert!(text.contains("unused output_level=7"), "{text}");
    assert!(text.contains("00 !80 .. !5 ..."), "{text}");
    assert!(parse_text(&text).unwrap() == song);
}

#[test]
fn a_pattern_that_differs_from_the_file_order_keeps_a_storage_line() {
    let mut text = song_with(&format!("00 {BLANK4}\n"));
    text = text.replace(
        "pattern P0 P1 P2 P3",
        "storage P3 P1 P0 P2\npattern P0 P1 P2 P3",
    );
    let song = parse_text(&text).unwrap();
    let keys: Vec<i32> = song.patterns.iter().map(|p| p.key).collect();
    assert_eq!(keys, [3, 1, 0, 2]);
    assert!(to_text(&song).contains("\nstorage P3 P1 P0 P2\n"));
    let plain = parse_text(&song_with(&format!("00 {BLANK4}\n"))).unwrap();
    assert!(!to_text(&plain).contains("storage"));
}

#[test]
fn a_routine_keeps_its_line_endings() {
    let text = format!(
        "{FORMAT_LINE}\nroutine 2 crlf\n| ld a, 1\n|   ret\n|\nend\nroutine 5\n| nop\nend\n"
    );
    let song = parse_text(&text).unwrap();
    assert_eq!(song.routines[2], b"ld a, 1\r\n  ret\r\n");
    assert_eq!(song.routines[5], b"nop");
    let again = parse_text(&to_text(&song)).unwrap();
    assert!(again == song);
}

#[test]
fn strings_escape_quotes_backslashes_and_bytes_that_are_not_utf8() {
    let song = Song {
        name: gbcli_music::ShortString::new(b"say \"hi\" \\ \xFF\n".to_vec()).unwrap(),
        ..Song::default()
    };
    let text = to_text(&song);
    assert!(text.contains(r#"name "say \"hi\" \\ \xFF\n""#), "{text}");
    assert!(parse_text(&text).unwrap() == song);
}

// The errors. Each names a line and column, says what is wrong and what to
// do next.

#[track_caller]
fn check(e: &ParseError, line: u32, column: u32, message: &str, remedy: &str) {
    assert_eq!((e.line, e.column), (line, column), "{e:?}");
    assert!(e.message.contains(message), "message: {}", e.message);
    assert!(e.remedy.contains(remedy), "remedy: {}", e.remedy);
}

#[test]
fn a_file_without_the_format_line_is_refused() {
    let e = error("name \"x\"\n");
    check(&e, 1, 1, "format", FORMAT_LINE);
}

#[test]
fn an_unknown_word_names_the_ones_it_could_be() {
    let e = error(&format!("{FORMAT_LINE}\n\ntempo 5\n"));
    check(&e, 3, 1, "tempo", "ticks_per_row");
}

#[test]
fn a_bad_note_is_located_to_its_cell() {
    let e = error(&song_with(
        "02 C-5 01 ... ... | H-5 01 ... ... | ... .. ... ... | ... .. ... ...\n",
    ));
    check(&e, 5, 21, "H-5", "C-3");
}

#[test]
fn a_row_with_the_wrong_number_of_cells_says_the_shape() {
    let e = error(&song_with("00 C-5 01 ... ... | ... .. ... ...\n"));
    check(&e, 5, 1, "2 cells", "four cells");
}

#[test]
fn rows_out_of_order_are_refused() {
    let e = error(&song_with(&format!("05 {BLANK4}\n03 {BLANK4}\n")));
    check(&e, 6, 1, "row 03", "ascending");
}

#[test]
fn an_instrument_number_outside_its_bank_is_refused() {
    let e = error(&format!("{FORMAT_LINE}\ninstrument wave 16 \"x\"\n"));
    check(&e, 2, 17, "16", "1-15");
}

#[test]
fn a_field_another_bank_uses_is_refused_with_the_banks_fields() {
    let e = error(&format!(
        "{FORMAT_LINE}\ninstrument wave 1 \"x\"\n  duty 50%\n"
    ));
    check(&e, 3, 3, "duty", "level");
}

#[test]
fn a_value_out_of_range_says_the_range_and_the_escape() {
    let e = error(&format!(
        "{FORMAT_LINE}\ninstrument square 1 \"x\"\n  envelope 20 down 0\n"
    ));
    check(&e, 3, 12, "20", "0-15");
    assert!(e.remedy.contains('!'), "{}", e.remedy);
}

#[test]
fn an_order_naming_a_pattern_with_no_rows_is_refused() {
    let e = error(&format!(
        "{FORMAT_LINE}\norders\n00 P0 P1 P2 P9\npattern P0 P1 P2 P3\n00 {BLANK4}\n"
    ));
    check(&e, 3, 13, "P9", "pattern block");
}

#[test]
fn a_pattern_shown_twice_must_read_the_same() {
    let e = error(&format!(
        "{FORMAT_LINE}\norders\n00 P0 P1 P2 P3\n01 P4 P1 P2 P3\n\
         pattern P0 P1 P2 P3\n00 {BLANK4}\n\
         pattern P4 P1 P2 P3\n07 ... .. ... ... | C-5 01 ... ... | ... .. ... ... | ... .. ... ...\n"
    ));
    check(&e, 8, 21, "P1", "every copy");
    assert!(e.message.contains("line 5"), "{}", e.message);
}

#[test]
fn an_unterminated_string_is_refused() {
    let e = error(&format!("{FORMAT_LINE}\nname \"Song\n"));
    check(&e, 2, 6, "string", "\"");
}

#[test]
fn a_routine_without_end_is_refused() {
    let e = error(&format!("{FORMAT_LINE}\nroutine 1\n| nop\norders\n"));
    check(&e, 4, 1, "routine 1", "end");
}

#[test]
fn a_cell_in_a_column_with_no_pattern_must_be_blank() {
    let e = error(&format!(
        "{FORMAT_LINE}\npattern P0 - - -\n00 ... .. ... ... | C-5 01 ... ... | ... .. ... ... | ... .. ... ...\n"
    ));
    check(&e, 3, 21, "no pattern", "blank");
}
