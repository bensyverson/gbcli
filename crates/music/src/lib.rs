//! gbcli's music: hUGETracker `.uge` songs (read v0–v6, write v6), a
//! canonical tracker-grid text form an agent can read and edit, conversion
//! both ways, and a piano roll drawn from a song file without building it.
//!
//! The text form is specified in design §12 (the 2026-11-07 music update)
//! and taught by `gbcli help music`; [`EXAMPLE`] is a complete song in it.

mod error;
mod example;
mod model;
mod notes;
mod piano;
mod playable;
mod text;
mod uge;

pub use error::{ParseError, SongError, UgeError};
pub use example::EXAMPLE;
pub use model::{
    BANK_SIZE, BLANK_ROWS, Bank, Cell, DEFAULT_WAVES, HIGHEST_JUMP, HIGHEST_NOTE, INSTRUMENTS,
    Instrument, MIDDLE_NOTE, NO_NOTE, PLAYED_SUBPATTERN_ROWS, Pattern, ROUTINES, ROWS, Rows,
    ShortString, Song, WAVE_LEN, WAVES,
};
pub use notes::{EFFECTS, Effect, note_name, period_of};
pub use piano::{PianoMade, piano_roll};
pub use playable::{Unplayable, unplayable};
pub use text::{FORMAT_LINE, parse_text, to_text};
pub use uge::{FORMAT_VERSION, Uge, read_uge, write_uge};

/// Which form a song file is in.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SongFormat {
    Uge,
    Text,
}

impl SongFormat {
    pub fn name(self) -> &'static str {
        match self {
            SongFormat::Uge => "uge",
            SongFormat::Text => "text",
        }
    }
}

/// A song file, read in whichever form it is.
#[derive(Clone, Debug)]
pub struct Loaded {
    pub format: SongFormat,
    /// The `.uge` version read, when the file was a `.uge`.
    pub uge_version: Option<u32>,
    pub song: Song,
}

/// Reads `bytes` as a `.uge` when its first four bytes are a little-endian
/// number below 256 (a version; its bytes 2-4 are NUL, which song text never
/// holds), and as song text otherwise. A version past [`FORMAT_VERSION`] is
/// refused as a `.uge` from a newer hUGETracker.
pub fn read_song(bytes: &[u8]) -> Result<Loaded, SongError> {
    let version = bytes
        .get(..4)
        .map(|b| u32::from_le_bytes([b[0], b[1], b[2], b[3]]));
    if version.is_some_and(|v| v < 256) {
        let uge = read_uge(bytes)?;
        return Ok(Loaded {
            format: SongFormat::Uge,
            uge_version: Some(uge.version),
            song: uge.song,
        });
    }
    let text = std::str::from_utf8(bytes).map_err(|e| {
        let valid = &bytes[..e.valid_up_to()];
        let line = valid.iter().filter(|b| **b == b'\n').count() as u32 + 1;
        ParseError {
            line,
            column: 1,
            message: "this line is not UTF-8, so the file is neither a .uge nor song text".into(),
            remedy: format!(
                "a song file is a hUGETracker .uge, or UTF-8 text starting with `{FORMAT_LINE}`"
            ),
        }
    })?;
    Ok(Loaded {
        format: SongFormat::Text,
        uge_version: None,
        song: parse_text(text)?,
    })
}
