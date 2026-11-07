//! Versions 0-5 to version 6, as hUGETracker's song.pas `UpgradeSong`
//! chain does it (v1 → v2 → v3 → v4 → v6; v5 shares v4's record).
//!
//! - **v1/v2 → v3:** 15 instruments, each with a type, become three banks
//!   of 15: instrument n goes to slot n of its type's bank; the other slots
//!   get hUGETracker's defaults. Waves drop their 33rd byte. v1 has no
//!   routines.
//! - **v3 → v4:** after the rest of the chain, every note with an
//!   instrument in a pattern channel 4 plays (orders, unread last entry
//!   excluded) is renoted: the new noise instruments read a note as a noise
//!   clock, so the old note's period is turned into the note whose clock
//!   the old driver would have produced. A note outside C-3..B-8 is left
//!   alone (hUGETracker has no period for it).
//! - **v4 → v6:** each noise instrument's six-step noise macro becomes its
//!   subpattern: rows 1-6 get the macro's offsets, and row min(ticks per
//!   row, 7) jumps to itself; the subpattern is enabled when any step is
//!   non-zero. Other subpatterns start blank and disabled; pattern cells
//!   gain a volume of 0; the timer is off. hUGETracker does this for every
//!   noise instrument; GB Studio's loader differs (it enables every
//!   converted macro), and hUGETracker is followed here.

use crate::model::{
    BANK_SIZE, Bank, Instrument, MIDDLE_NOTE, NO_NOTE, Pattern, ROUTINES, ShortString, Song,
    WAVE_LEN, WAVES,
};
use crate::notes::period_of;

/// A v1-v5 instrument: the v6 fields it shares, and the v4 noise macro.
pub struct OldInstrument {
    pub v6: Instrument,
    pub noise_macro: [i8; 6],
}

/// A v1-v5 file as read: 15 instruments before v3, 45 after.
pub struct OldSong {
    pub version: u32,
    pub name: ShortString,
    pub artist: ShortString,
    pub comment: ShortString,
    pub instruments: Vec<OldInstrument>,
    pub waves: [[u8; WAVE_LEN]; WAVES],
    pub ticks_per_row: i32,
    pub patterns: Vec<Pattern>,
    pub orders: [Vec<i32>; 4],
    pub routines: [Vec<u8>; ROUTINES],
}

/// The noise clock note for each NR43 shift, for a dividing ratio of 0
/// (constants.pas `Ch4FreqToNoteCodeMap` at 2^(19 − shift)).
const NOISE_NOTE_BY_SHIFT: [i32; 16] = [
    63, 62, 61, 59, 55, 51, 47, 43, 39, 35, 31, 27, 23, 19, 15, 11,
];

pub fn upgrade(old: OldSong) -> Song {
    let renote_noise = old.version <= 3;
    let ticks_per_row = old.ticks_per_row;
    let mut instruments: Vec<OldInstrument> = if old.version < 3 {
        banked(old.instruments)
    } else {
        old.instruments
    };
    for (index, ins) in instruments.iter_mut().enumerate() {
        if Bank::of_slot(index) == Bank::Noise {
            ins.v6.subpattern = macro_subpattern(&ins.noise_macro, ticks_per_row);
            ins.v6.subpattern_enabled = u8::from(ins.noise_macro.iter().any(|m| *m != 0));
        }
    }
    let mut song = Song {
        name: old.name,
        artist: old.artist,
        comment: old.comment,
        instruments: instruments.into_iter().map(|i| i.v6).collect(),
        waves: old.waves,
        ticks_per_row,
        timer_enabled: 0,
        timer_divider: 0,
        patterns: old.patterns,
        orders: old.orders,
        routines: old.routines,
    };
    if renote_noise {
        renote_noise_patterns(&mut song);
    }
    song
}

/// v1/v2's 15 typed instruments in v3's three banks (song.pas
/// `UpgradeSong(TSongV2)`). An instrument of an unknown type is dropped, as
/// there.
fn banked(old: Vec<OldInstrument>) -> Vec<OldInstrument> {
    let mut banks: Vec<OldInstrument> = (0..3 * BANK_SIZE)
        .map(|i| OldInstrument {
            v6: Instrument::default_for(i),
            noise_macro: [0; 6],
        })
        .collect();
    for (n, ins) in old.into_iter().enumerate() {
        if let Ok(bank) = usize::try_from(ins.v6.type_code)
            && bank < 3
        {
            banks[bank * BANK_SIZE + n] = ins;
        }
    }
    banks
}

/// song.pas `ConvertNoiseMacro`.
fn macro_subpattern(noise_macro: &[i8; 6], ticks_per_row: i32) -> crate::model::Rows {
    let mut rows = crate::model::BLANK_ROWS;
    for (j, m) in noise_macro.iter().enumerate() {
        rows[j + 1].note = i32::from(*m) + MIDDLE_NOTE;
    }
    let wrap = ticks_per_row.min(7);
    if wrap >= 1 {
        rows[(wrap - 1) as usize].volume = wrap;
    }
    rows
}

/// song.pas `UpgradeSong(TSongV3)`'s `ConvertPattern`, over the patterns
/// channel 4 plays.
fn renote_noise_patterns(song: &mut Song) {
    let noise = song.orders[3].clone();
    for pattern in song.patterns.iter_mut().filter(|p| noise.contains(&p.key)) {
        for cell in &mut pattern.rows {
            if cell.instrument == 0 || cell.note == NO_NOTE {
                continue;
            }
            if let Some(period) = period_of(cell.note) {
                let shift = 15 - usize::from(period >> 7);
                cell.note = NOISE_NOTE_BY_SHIFT[shift];
            }
        }
    }
}
