//! The piano roll drawn from a song file, and reading either form.

mod common;

use common::{assert_golden, samples};
use gbcli_music::{EXAMPLE, SongError, SongFormat, parse_text, piano_roll, read_song};
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
