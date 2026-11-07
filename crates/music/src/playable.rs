//! What of a song hUGEDriver never plays, though the song keeps it so text
//! and `.uge` stay lossless.

use std::fmt;

use crate::model::{BANK_SIZE, Bank, HIGHEST_JUMP, PLAYED_SUBPATTERN_ROWS, Song};

/// Part of an instrument's subpattern hUGEDriver never plays.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Unplayable {
    /// Content in rows past [`PLAYED_SUBPATTERN_ROWS`]; `first` is the
    /// first such row.
    Rows {
        bank: Bank,
        /// 1-15.
        slot: usize,
        first: usize,
    },
    /// A played row's jump past [`HIGHEST_JUMP`].
    Jump {
        bank: Bank,
        /// 1-15.
        slot: usize,
        row: usize,
        jump: i32,
    },
}

impl fmt::Display for Unplayable {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let last = PLAYED_SUBPATTERN_ROWS - 1;
        match *self {
            Unplayable::Rows { bank, slot, first } => write!(
                f,
                "{} instrument {slot}'s subpattern has content at row {first:02}, but \
                 hUGEDriver plays only rows 00-{last:02}",
                bank.name(),
            ),
            Unplayable::Jump {
                bank,
                slot,
                row,
                jump,
            } => write!(
                f,
                "{} instrument {slot}'s subpattern row {row:02} jumps with J{jump:02}, but \
                 hUGEDriver's jumps reach J01-J{HIGHEST_JUMP:02} (rows 00-{last:02})",
                bank.name(),
            ),
        }
    }
}

/// Every part of `song`'s subpatterns hUGEDriver never plays, by
/// instrument: its jumps past [`HIGHEST_JUMP`], then its rows past
/// [`PLAYED_SUBPATTERN_ROWS`].
pub fn unplayable(song: &Song) -> Vec<Unplayable> {
    let mut found = Vec::new();
    for (index, ins) in song.instruments.iter().enumerate() {
        let (bank, slot) = (Bank::of_slot(index), index % BANK_SIZE + 1);
        let (played, rest) = ins.subpattern.split_at(PLAYED_SUBPATTERN_ROWS);
        for (row, cell) in played.iter().enumerate() {
            if cell.volume > HIGHEST_JUMP {
                found.push(Unplayable::Jump {
                    bank,
                    slot,
                    row,
                    jump: cell.volume,
                });
            }
        }
        if let Some(i) = rest.iter().position(|c| !c.is_blank()) {
            found.push(Unplayable::Rows {
                bank,
                slot,
                first: PLAYED_SUBPATTERN_ROWS + i,
            });
        }
    }
    found
}
