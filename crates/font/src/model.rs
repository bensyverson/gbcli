//! A font as gbcli holds it: its comments and properties, kept so a
//! rewrite loses nothing, and its glyphs in file order, which is glyph
//! number order.

use crate::key::Key;

/// Every glyph's height in pixels.
pub const HEIGHT: usize = 8;
/// The widest a glyph may be: one tile.
pub const MAX_WIDTH: u8 = 8;

/// The rows above the baseline when a font doesn't say (its `ascent`
/// property): row 7 is the descender.
pub const DEFAULT_ASCENT: u8 = 7;

/// A font: glyphs in glyph-number order, never deduplicated.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Font {
    /// The comment at the top of the file, before a blank line.
    pub comment: Vec<String>,
    /// The global properties (`name: …`), in file order.
    pub properties: Vec<Property>,
    pub glyphs: Vec<Glyph>,
    /// Comments after the last glyph.
    pub tail: Vec<String>,
}

/// A global property; a value of several lines holds them joined by `\n`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Property {
    /// The comment lines just above it.
    pub comment: Vec<String>,
    pub key: String,
    pub value: String,
}

/// One glyph: 8 rows of `width` pixels.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Glyph {
    /// The comment lines just above it.
    pub comment: Vec<String>,
    pub key: Key,
    /// 1 to [`MAX_WIDTH`]: how far the pen moves after it.
    pub width: u8,
    /// Row 0 first; bit 7 is the leftmost pixel, a set bit is ink, and the
    /// bits right of `width` are clear.
    pub rows: [u8; HEIGHT],
}

impl Font {
    /// Whether every glyph is a whole tile wide (an empty font is not).
    pub fn fixed(&self) -> bool {
        !self.glyphs.is_empty() && self.glyphs.iter().all(|g| g.width == MAX_WIDTH)
    }

    /// The narrowest and widest glyph, when there is one.
    pub fn widths(&self) -> Option<(u8, u8)> {
        let min = self.glyphs.iter().map(|g| g.width).min()?;
        let max = self.glyphs.iter().map(|g| g.width).max()?;
        Some((min, max))
    }

    /// The rows above the baseline: the `ascent` property when it is a
    /// whole number from 1 to 8, else [`DEFAULT_ASCENT`].
    pub fn ascent(&self) -> u8 {
        self.property("ascent")
            .and_then(|v| v.parse::<u8>().ok())
            .filter(|a| (1..=HEIGHT as u8).contains(a))
            .unwrap_or(DEFAULT_ASCENT)
    }

    /// The value of the property `key` (`_` and `-` alike, any case).
    pub fn property(&self, key: &str) -> Option<&str> {
        let norm = |k: &str| k.to_ascii_lowercase().replace('_', "-");
        self.properties
            .iter()
            .find(|p| norm(&p.key) == norm(key))
            .map(|p| p.value.as_str())
    }

    /// The glyph number of `key`.
    pub fn find(&self, key: &Key) -> Option<usize> {
        self.glyphs.iter().position(|g| &g.key == key)
    }
}

impl Glyph {
    /// Whether the pixel at `column` of `row` is ink.
    pub fn ink(&self, column: u8, row: usize) -> bool {
        column < self.width && self.rows[row] & (0x80 >> column) != 0
    }
}
