//! The piano roll drawn from a song file, and reading either form.

mod common;

use common::{assert_golden, samples};
use gbcli_music::{
    BLANK_ROWS, Cell, EXAMPLE, Pattern, Song, SongError, SongFormat, parse_text, piano_roll,
    read_song, write_uge,
};
use gbcli_view::LabelKey;

#[test]
fn the_examples_piano_roll_is_pinned() {
    let song = parse_text(EXAMPLE).unwrap();
    let made = piano_roll(&song).unwrap();
    assert!(made.notes > 10, "{} notes", made.notes);
    assert_eq!(made.rows, 128, "two orders of 64 rows");
    let detail = made.image.label_text(LabelKey::Detail).unwrap();
    assert!(detail.contains("song"), "{detail}");
    assert_golden("example_piano.png", &made.image.png().unwrap());
}

#[test]
fn a_long_song_fits_the_widest_roll() {
    let monkeys = samples()
        .into_iter()
        .find(|s| s.name.starts_with("Chavez"))
        .unwrap();
    let song = read_song(&monkeys.bytes).unwrap().song;
    let made = piano_roll(&song).unwrap();
    assert!(made.image.width <= gbcli_audio::MAX_WIDTH);
    assert!(made.rows > 1000, "{} rows", made.rows);
}

/// A song whose orders 00-03 play P0-P3 on channel 1 (the rest rest), with
/// `effects` as (pattern, row, effect) on channel 1.
fn walk(effects: &[(usize, usize, &str)]) -> u32 {
    let blank = "... .. ... ...";
    let mut text = format!("{}\norders\n", gbcli_music::FORMAT_LINE);
    for i in 0..4 {
        text += &format!("{i:02} P{i} P9 P9 P9\n");
    }
    for p in 0..4 {
        text += &format!("pattern P{p} P9 P9 P9\n");
        for (_, row, fx) in effects.iter().filter(|(q, _, _)| *q == p) {
            text += &format!("{row:02} C-5 01 ... {fx} | {blank} | {blank} | {blank}\n");
        }
    }
    parse_text(&text)
        .map(|s| piano_roll(&s).unwrap().rows)
        .unwrap()
}

#[test]
fn the_walk_follows_hugedrivers_position_jump_and_pattern_break() {
    // hUGEDriver (727ab8e, hUGEDriver.asm fx_pos_jump, fx_pattern_break and
    // the row-end code): B00 goes to the next order, Bxx to order xx - 1;
    // Dxx starts the next order at row xx - 1, and D00 does nothing.
    assert_eq!(walk(&[]), 4 * 64);
    assert_eq!(walk(&[(0, 33, "B00")]), 34 + 3 * 64);
    assert_eq!(walk(&[(0, 9, "B03")]), 10 + 2 * 64);
    assert_eq!(walk(&[(0, 9, "D03")]), 10 + 62 + 2 * 64);
    assert_eq!(walk(&[(0, 9, "D00")]), 4 * 64);
    // A jump back to an order already played ends the walk: the song loops.
    assert_eq!(walk(&[(1, 0, "B01")]), 64 + 1);
}

#[test]
fn a_song_with_no_orders_has_nothing_to_draw() {
    let song = parse_text(&format!("{}\n", gbcli_music::FORMAT_LINE)).unwrap();
    assert!(piano_roll(&song).is_err());
}

#[test]
fn read_song_tells_the_two_forms_apart() {
    let uge = &samples()[0];
    let loaded = read_song(&uge.bytes).unwrap();
    assert_eq!(loaded.format, SongFormat::Uge);
    assert_eq!(loaded.uge_version, Some(uge.version));
    let loaded = read_song(EXAMPLE.as_bytes()).unwrap();
    assert_eq!(loaded.format, SongFormat::Text);
    assert_eq!(loaded.uge_version, None);
    match read_song(b"tempo 5\n") {
        Err(SongError::Text(e)) => assert_eq!((e.line, e.column), (1, 1)),
        other => panic!("{other:?}"),
    }
}

/// A one-order song at `ticks` per row whose channel 1 plays `rows` as
/// (row, note, effect code, parameter); channels 2-4 play a blank P1.
fn one_order(ticks: i32, rows: &[(usize, i32, i32, u8)]) -> Song {
    let mut p0 = Pattern {
        key: 0,
        rows: BLANK_ROWS,
    };
    for &(row, note, effect_code, effect_param) in rows {
        p0.rows[row] = Cell {
            note,
            instrument: 1,
            volume: 0,
            effect_code,
            effect_param,
        };
    }
    Song {
        ticks_per_row: ticks,
        patterns: vec![
            p0,
            Pattern {
                key: 1,
                rows: BLANK_ROWS,
            },
        ],
        orders: [vec![0], vec![1], vec![1], vec![1]],
        ..Song::default()
    }
}

#[test]
fn note_delay_700_never_plays_the_note() {
    // hUGEDriver.asm fx_note_delay: "0 never plays the note".
    let song = one_order(6, &[(0, 24, 7, 0x00)]);
    assert_eq!(piano_roll(&song).unwrap().notes, 0);
}

#[test]
fn a_note_delay_as_long_as_the_row_never_plays_the_note() {
    // fx_note_delay plays when the row's tick counter equals the delay, and
    // the counter only reaches ticks_per_row - 1.
    for delay in [6, 9] {
        let song = one_order(6, &[(0, 24, 7, delay)]);
        assert_eq!(
            piano_roll(&song).unwrap().notes,
            0,
            "7{delay:02X} at speed 6"
        );
    }
    let song = one_order(6, &[(0, 24, 7, 5)]);
    assert_eq!(piano_roll(&song).unwrap().notes, 1, "705 at speed 6 plays");
}

#[test]
fn a_skipped_delayed_note_leaves_the_previous_note_sounding() {
    let delayed = piano_roll(&one_order(6, &[(0, 24, 0, 0), (1, 28, 7, 0)])).unwrap();
    let plain = piano_roll(&one_order(6, &[(0, 24, 0, 0)])).unwrap();
    assert_eq!(delayed.notes, 1);
    assert!(
        delayed.image == plain.image,
        "the skipped note cut the first"
    );
}

#[test]
fn a_note_cut_as_late_as_the_row_never_fires() {
    // fx_note_cut cuts when the row's tick counter equals the parameter, so
    // E06 and E09 at speed 6 never fire and the note plays to the song's end.
    let plain = piano_roll(&one_order(6, &[(0, 24, 0, 0)])).unwrap();
    for cut in [6, 9] {
        let made = piano_roll(&one_order(6, &[(0, 24, 0xE, cut)])).unwrap();
        assert_eq!(made.notes, 1);
        assert!(
            made.image == plain.image,
            "E{cut:02X} at speed 6 cut the note"
        );
    }
    let cut = piano_roll(&one_order(6, &[(0, 24, 0xE, 5)])).unwrap();
    assert!(cut.image != plain.image, "E05 at speed 6 cuts");
}

#[test]
fn the_walk_plays_to_the_longest_channels_orders() {
    // hUGETracker shows a shorter channel's missing orders as pattern 0
    // (tracker.pas CopyOrderMatrixToOrderGrid).
    let mut song = one_order(6, &[(0, 24, 0, 0)]);
    song.orders = [vec![0, 0], vec![1], vec![1], vec![1]];
    assert_eq!(piano_roll(&song).unwrap().rows, 128);
}

#[test]
fn an_instrument_of_i32_min_draws_without_overflow() {
    let mut song = one_order(6, &[(0, 24, 0, 0)]);
    song.patterns[0].rows[0].instrument = i32::MIN;
    let song = read_song(&write_uge(&song)).unwrap().song;
    assert_eq!(piano_roll(&song).unwrap().notes, 1);
}
