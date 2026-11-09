//! Pixel fonts (project/2026-11-09-pixel-fonts-design.md, Ben's
//! decisions): `FontAsset`, the seam a client reads fonts through (the
//! `assets` op and a build's images, in `ImagesMade.fonts`), and the
//! `font` op behind `gbcli font`. `gbcli-font` reads and sets fonts;
//! `gbcli-build` fills these. Defined once here: LoadHigh imports them.

use serde::{Deserialize, Serialize};

/// A font, as a build emits it or an `assets` request asked for it.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct FontAsset {
    /// The font file, as given (`assets/fonts/main.yaff`).
    pub source: String,
    /// The labels' stem (`Main`: `MainFontGlyphs`, `MainFontWidths`); the
    /// constants' is its upper-case form (`MAIN_FONT_GLYPHS`).
    pub label: String,
    /// Whether every glyph of the font (not only those here) is 8 wide.
    pub fixed: bool,
    /// In glyph-number order: a build's numbers, which its charmap maps
    /// to, or for a request the font's order among the glyphs asked for.
    pub glyphs: Vec<FontGlyph>,
}

/// One glyph: its key, its width and its 8 rows.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct FontGlyph {
    /// One character (one Unicode scalar value, in NFC), or a name of two
    /// or more ASCII letters, digits, `_` and `-` for a symbol Unicode
    /// lacks. A one-character key is always a character.
    pub key: String,
    /// 1 to 8: how far the pen moves after it (`gbcli help fonts`).
    pub width: u8,
    /// Row 0 first; bit 7 is the leftmost pixel and a set bit is ink. The
    /// bits right of `width` are clear.
    pub rows: [u8; 8],
    pub origin: GlyphOrigin,
}

/// Where a glyph came from.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum GlyphOrigin {
    /// Drawn in the font file.
    Drawn,
    /// Made from a base and a combining mark the font draws, for a letter
    /// it lacks.
    Composed,
}

/// `op: "font"`. Paths are relative to the server's working directory.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FontArgs {
    /// The font: a `.yaff` file.
    pub file: String,
    /// Text to set by the layout rules; a named glyph is written `<name>`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sample: Option<String>,
    /// Write a 4x preview here: the sample, or without one every glyph.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub preview: Option<String>,
    /// Fail (`predicate_false`) when the sample is wider than this many
    /// pixels. Needs `sample`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub fit: Option<u32>,
}

/// A font read, and what was made from it.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct FontMade {
    /// As given.
    pub path: String,
    pub glyphs: u32,
    pub fixed: bool,
    /// The narrowest and widest glyph; absent for a font with none.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub min_width: Option<u8>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max_width: Option<u8>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sample: Option<FontSample>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub preview: Option<FontPreview>,
}

/// A sample set in the font: the reference a renderer matches.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct FontSample {
    pub text: String,
    /// Pixels from the line's left edge to the pen's last position.
    pub width: u32,
    /// The tiles it touches.
    pub tiles: u32,
    /// The characters the font has no glyph for, each once; they take no
    /// room.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub missing: Vec<String>,
    /// The tiles, 1bpp: 8 bytes each, row 0 first, bit 7 the leftmost.
    #[serde(with = "crate::hex::bytes")]
    pub data: Vec<u8>,
    /// The width it had to fit in, when asked.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub fit: Option<u32>,
}

/// The preview written.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct FontPreview {
    pub path: String,
    pub bytes: u64,
}

/// Why a font didn't read: the line and column at fault, when there is one.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct FontProblem {
    pub path: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub line: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub column: Option<u32>,
    pub message: String,
    pub remedy: String,
}

#[cfg(test)]
mod tests;
