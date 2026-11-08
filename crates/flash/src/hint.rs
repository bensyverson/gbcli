//! A mask-ROM cart's watch for flash commands: the program that sends them
//! was written for a flash-save board, and saying which routes its author
//! to modeling one.

use crate::note::{FlashNote, FlashNotes, NoteKind};
use crate::profile::FlashProfile;

/// Fed a mask-ROM cart's CPU writes to $0000-$7FFF, it keeps the first that
/// is a named board's first unlock cycle until taken: one note per take,
/// however many commands the program sends.
#[derive(Clone, Debug, Default)]
pub struct UnlockWatch {
    seen: Option<FlashNote>,
}

impl UnlockWatch {
    /// A CPU write at `now`; `chip_addr` is the mapped bank's base plus the
    /// offset, as [`FlashChip::write`](crate::FlashChip::write) takes it.
    pub fn write(&mut self, now: u64, bus_addr: u16, chip_addr: u32, value: u8) {
        if self.seen.is_some() {
            return;
        }
        if let Some(board) = FlashProfile::unlocked_by(chip_addr, value) {
            self.seen = Some(FlashNote {
                at: now,
                bus_addr,
                value,
                kind: NoteKind::NoChip { board },
            });
        }
    }

    /// The note since the last take, if any.
    pub fn take(&mut self) -> FlashNotes {
        FlashNotes {
            notes: self.seen.take().into_iter().collect(),
            dropped: 0,
        }
    }
}
