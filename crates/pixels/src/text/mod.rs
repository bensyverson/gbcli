//! Pixel text: the parser and the canonical writer.

mod lex;
mod parse;
mod write;

pub use parse::{MAX_SIDE, parse_text};
pub use write::to_text;

/// A pixel text file's first line.
pub const FORMAT_LINE: &str = "format gbcli-pixels 1";

/// A pixel text file's extension.
pub const EXTENSION: &str = "pixels";

/// How the canonical writer lays the pixels out.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Layout {
    /// Pixel rows: one block per frame of `frame`'s size when the image is
    /// whole frames of it, else one per 8-row band.
    Rows { frame: Option<(u32, u32)> },
    /// The distinct 8x8 tiles, keyed in hexadecimal by first use, and a map.
    Tiles,
}
