//! gbcli's strict subset of YAFF 1.0 (monobit's `YAFF.md`, version 1.0.4).
//! `syntax` reads the whole YAFF grammar into a [`syntax::Document`], so
//! a file outside the subset is refused for what it is, at its line;
//! `strict` holds the document to gbcli's rules and makes a [`Font`];
//! `write` writes a font canonically, so a read and a write round-trip.
//!
//! The subset, which `gbcli help fonts` teaches: global comments and
//! properties as YAFF has them; then glyphs, each one label (`'A':`,
//! `u+0041:` or `"name":`) over exactly 8 rows of 1 to 8 `.` and `@`. No
//! codepoint labels, grapheme clusters, unquoted (deprecated) labels,
//! second labels, empty glyphs, per-glyph properties, kerning or bearings:
//! spacing is drawn into the glyph.

mod strict;
pub mod syntax;
mod write;

use crate::error::FontError;
use crate::model::Font;

/// A font file's extension.
pub const EXTENSION: &str = "yaff";

/// Reads `text` as a gbcli font.
pub fn read(text: &str) -> Result<Font, FontError> {
    strict::font(syntax::parse(text)?)
}

/// `font` as canonical YAFF: what [`read`] reads back unchanged.
pub fn write(font: &Font) -> String {
    write::write(font)
}

#[cfg(test)]
mod tests;
