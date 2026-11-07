//! The song text form: a tracker grid an agent can read and edit, and
//! hUGETracker's whole song around it. Specified in design §12 (the
//! 2026-11-07 music update); `gbcli help music` teaches it.
//!
//! Every value a `.uge` holds has a spelling, so a song's text parses back
//! to the same song, once [`crate::Song::normalize`]d as every `.uge` read
//! is (the text has no spelling for uneven orders); [`to_text`] writes one
//! canonical spelling (all 64 rows, every field), so text → song → text is a
//! fixpoint. The parser is more
//! lenient than the writer: rows, fields, instruments and waves left out
//! take hUGETracker's defaults.

mod fields;
mod grid;
mod instrument;
mod lex;
mod parse;
mod spell;
mod values;
mod write;

pub use parse::parse_text;
pub use write::to_text;

/// The first line of every song text.
pub const FORMAT_LINE: &str = "format gbcli-song 1";
