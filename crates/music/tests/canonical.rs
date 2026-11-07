//! A `.uge` read the way hUGETracker reads it: every channel's orders at one
//! length and every pattern an order names present, so the song a reader
//! returns survives text and `.uge` unchanged.

mod common;

use common::samples;
use gbcli_music::{BLANK_ROWS, Pattern, Song, parse_text, read_song, to_text, write_uge};

/// Patterns `keys`, each with a note on row 0, and `orders`.
fn song(keys: &[i32], orders: [Vec<i32>; 4]) -> Song {
    let patterns = keys
        .iter()
        .map(|&key| {
            let mut rows = BLANK_ROWS;
            rows[0].note = 24;
            Pattern { key, rows }
        })
        .collect();
    Song {
        patterns,
        orders,
        ..Song::default()
    }
}

#[track_caller]
fn survives_text(song: &Song) {
    let back = parse_text(&to_text(song)).unwrap();
    assert!(back == *song, "text changed the song");
    let back = read_song(&write_uge(song)).unwrap().song;
    assert!(back == *song, ".uge changed the song");
}

#[test]
fn an_order_naming_a_missing_pattern_reads_as_a_blank_pattern() {
    // hUGETracker's TPatternMap.GetOrCreateNew: a blank pattern, added last.
    let written = song(&[0, 1], [vec![0], vec![1], vec![1], vec![5]]);
    let read = read_song(&write_uge(&written)).unwrap().song;
    let keys: Vec<i32> = read.patterns.iter().map(|p| p.key).collect();
    assert_eq!(keys, [0, 1, 5]);
    assert_eq!(read.pattern(5).unwrap().rows, BLANK_ROWS);
    survives_text(&read);
}

#[test]
fn a_shorter_channels_orders_read_padded_with_pattern_0() {
    // hUGETracker shows a missing order as 0 (tracker.pas
    // CopyOrderMatrixToOrderGrid) and saves every channel at that length.
    let written = song(&[0, 1], [vec![0, 1], vec![1], vec![1], vec![]]);
    let read = read_song(&write_uge(&written)).unwrap().song;
    assert_eq!(
        read.orders,
        [vec![0, 1], vec![1, 0], vec![1, 0], vec![0, 0]]
    );
    assert!(to_text(&read).contains("\n01 P1 P0 P0 P0\n"));
    survives_text(&read);
}

#[test]
fn padding_creates_pattern_0_when_the_song_has_none() {
    let written = song(&[1, 2], [vec![1, 1], vec![2], vec![2], vec![2]]);
    let read = read_song(&write_uge(&written)).unwrap().song;
    let keys: Vec<i32> = read.patterns.iter().map(|p| p.key).collect();
    assert_eq!(keys, [1, 2, 0]);
    survives_text(&read);
}

#[test]
fn a_newer_uge_says_its_version_through_read_song() {
    let s = samples().into_iter().find(|s| s.version == 6).unwrap();
    let mut b = s.bytes.clone();
    b[..4].copy_from_slice(&7u32.to_le_bytes());
    let e = read_song(&b).unwrap_err();
    assert!(e.message().contains("version 7"), "{}", e.message());
    assert!(e.remedy().contains("newer gbcli"), "{}", e.remedy());
}
