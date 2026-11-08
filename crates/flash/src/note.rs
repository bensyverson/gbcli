//! What the chip records when a program's commands miss the board: the
//! report a silent hardware failure never gives.

use std::fmt;

use crate::profile::DataLines;

/// An operation the chip runs.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Operation {
    Program,
    SectorErase,
    ChipErase,
}

impl Operation {
    pub fn name(self) -> &'static str {
        match self {
            Operation::Program => "byte program",
            Operation::SectorErase => "sector erase",
            Operation::ChipErase => "chip erase",
        }
    }
}

/// Why a note was taken.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum NoteKind {
    /// An unlock cycle's address with the byte this board's data lines would
    /// need under the other wiring: the program targets that board.
    DataLines {
        /// The byte the chip saw.
        seen: u8,
        /// The byte the chip expects.
        expected: u8,
        /// The wiring the program was written for.
        written_for: DataLines,
    },
    /// The first unlock byte at another board's unlock address.
    UnlockAddress { expected: u16 },
    /// A command write on a board whose /WE is on the AUDIO pin: the chip
    /// never sees a CPU write.
    WritePinAudio,
    /// A command sequence broken off by a write that isn't its next cycle.
    BrokenSequence { after: &'static str },
    /// A program that would turn 0 bits to 1, which only an erase does.
    ProgramSetsBits { was: u8, wanted: u8 },
    /// A write while the chip was busy, which the chip ignores.
    WriteWhileBusy { op: Operation },
    /// The power went off during an operation; the model counts it as done,
    /// where a real chip leaves it partial.
    CutOff { op: Operation },
    /// On a mask-ROM cart: the first unlock cycle of `board`
    /// ([`FlashProfile::unlocked_by`](crate::FlashProfile::unlocked_by)), so
    /// the program expects a flash chip the cart doesn't have.
    NoChip { board: &'static str },
}

/// One note: when, the write that caused it, and why.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct FlashNote {
    /// Emulated time, in ticks of 2^23 Hz.
    pub at: u64,
    pub bus_addr: u16,
    /// The byte the CPU wrote.
    pub value: u8,
    pub kind: NoteKind,
}

/// The notes taken since they were last taken, and how many were dropped
/// past the cap.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct FlashNotes {
    pub notes: Vec<FlashNote>,
    pub dropped: u64,
}

impl fmt::Display for FlashNote {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let (addr, value) = (self.bus_addr, self.value);
        match self.kind {
            NoteKind::DataLines {
                seen,
                expected,
                written_for,
            } => {
                let wiring = match written_for {
                    DataLines::SwappedD0D1 => "swapped",
                    DataLines::Straight => "straight",
                };
                write!(
                    f,
                    "${addr:04X}<-${value:02X} is not an unlock cycle on this board: the chip \
                     reads ${seen:02X} and expects ${expected:02X}. ${value:02X} is what a board \
                     with D0 and D1 {wiring} needs, so the program was written for that wiring; \
                     its flash commands do nothing here"
                )
            }
            NoteKind::UnlockAddress { expected } => write!(
                f,
                "${addr:04X}<-${value:02X} looks like an unlock cycle, but this board unlocks \
                 at ${expected:04X}; the program was written for another board's addresses"
            ),
            NoteKind::WritePinAudio => write!(
                f,
                "${addr:04X}<-${value:02X} never reaches the flash chip: this board's /WE is on \
                 the cart's AUDIO pin, which the Game Boy can't drive, so no program can write \
                 its flash"
            ),
            NoteKind::BrokenSequence { after } => write!(
                f,
                "${addr:04X}<-${value:02X} broke off a flash command after {after}; the chip \
                 went back to reading"
            ),
            NoteKind::ProgramSetsBits { was, wanted } => write!(
                f,
                "programming ${wanted:02X} over ${was:02X} at ${addr:04X} can't set bits from 0 \
                 to 1 (only an erase does); the byte is now ${:02X}",
                was & wanted
            ),
            NoteKind::WriteWhileBusy { op } => write!(
                f,
                "${addr:04X}<-${value:02X} came while the flash chip was busy with a {}; the \
                 chip ignored it",
                op.name()
            ),
            NoteKind::CutOff { op } => write!(
                f,
                "the power went off during a {}; this model counts it as done, but a real chip \
                 is left partly written",
                op.name()
            ),
            NoteKind::NoChip { board } => write!(
                f,
                "${addr:04X}<-${value:02X} is a flash chip's unlock cycle, as the {board} board \
                 decodes it, and this cart is mask ROM: the program sends flash commands, which \
                 do nothing here"
            ),
        }
    }
}
