//! `.uge`, hUGETracker's song file: a little-endian dump of its Pascal
//! song record (src/song.pas, src/hugedatatypes.pas at 3fedced3).
//!
//! Version 6, in file order: the version (u32); name, artist and comment
//! (`ShortString`s: a length byte and 255 bytes); 45 instruments of 1,385
//! bytes (15 square, 15 wave, 15 noise), each with a 64-row subpattern; 16
//! waves of 32 bytes; ticks per row (i32); timer enabled (u8) and divider
//! (i32); the pattern count, then per pattern its key (i32) and 64 cells of
//! 17 bytes (note, instrument, volume and effect code as i32, then the
//! effect parameter); per channel an order count and that many keys; 16
//! routines (u32 length and bytes).
//!
//! Writing a song read from a v6 file gives back its bytes except:
//!
//! - **`ShortString` tails.** Pascal copies a string into its 256-byte slot
//!   and leaves whatever an earlier, longer string left behind the length
//!   byte. Only the first `length` bytes are the string; the writer zeroes
//!   the rest.
//! - **The last order entry of each channel.** The stored count is one more
//!   than the orders the song plays, and hUGETracker never reads the extra
//!   entry (song.pas, `OptimizeSong` and the v3 upgrade both stop one short;
//!   GB Studio's loader drops it). The writer stores 0 there.
//!
//! Everything else, including fields an instrument's bank doesn't use, the
//! unused volume column of pattern cells and the pattern storage order,
//! survives.

mod read;
mod upgrade;
mod write;

pub use read::read_uge;
pub use write::write_uge;

use crate::model::Song;

/// The version this crate writes, and the newest it reads.
pub const FORMAT_VERSION: u32 = 6;

/// A `.uge` file, read and upgraded to version 6.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Uge {
    /// The version the file was written in.
    pub version: u32,
    pub song: Song,
}

/// Bytes in a `ShortString` slot.
pub(crate) const SHORT_STRING: usize = 256;
/// Bytes in a version 6 cell.
pub(crate) const CELL_V6: usize = 17;
/// Bytes in a version 1-5 cell, which has no volume.
pub(crate) const CELL_V1: usize = 13;
