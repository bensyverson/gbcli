//! What of a subpattern hUGEDriver plays: hUGETracker exports rows 00-31
//! (utils.pas `SubpatternToBytes`) and the driver's jump is five bits
//! (hUGEDriver.asm `do_table`), so rows 32-63 and jumps past J31 never play.
//! The song keeps them (lossless) and says so.

use gbcli_music::{BANK_SIZE, Bank, Cell, Song, Unplayable, unplayable};

#[test]
fn a_subpattern_within_rows_00_31_and_jumps_to_j31_plays_whole() {
    let mut song = Song::default();
    let sub = &mut song.instruments[2].subpattern;
    sub[31] = Cell {
        note: 40,
        volume: 31,
        ..Cell::BLANK
    };
    assert_eq!(unplayable(&song), []);
}

#[test]
fn rows_past_31_and_jumps_past_j31_are_named() {
    let mut song = Song::default();
    let wave = BANK_SIZE + 3;
    song.instruments[wave].subpattern[40].note = 40;
    song.instruments[wave].subpattern[50].note = 41;
    song.instruments[2].subpattern[5].volume = 40;
    let found = unplayable(&song);
    assert_eq!(
        found,
        [
            Unplayable::Jump {
                bank: Bank::Square,
                slot: 3,
                row: 5,
                jump: 40
            },
            Unplayable::Rows {
                bank: Bank::Wave,
                slot: 4,
                first: 40
            },
        ]
    );
    let shown: Vec<String> = found.iter().map(ToString::to_string).collect();
    assert!(shown[0].contains("square instrument 3"), "{}", shown[0]);
    assert!(
        shown[0].contains("J40") && shown[0].contains("J31"),
        "{}",
        shown[0]
    );
    assert!(
        shown[1].contains("row 40") && shown[1].contains("00-31"),
        "{}",
        shown[1]
    );
}
