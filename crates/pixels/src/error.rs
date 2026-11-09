//! Why an image didn't load: where, what, and what to do about it.

use std::fmt;

/// Pixel text that doesn't parse, or a legend value that doesn't resolve,
/// located to a line and column (both 1-based; the column counts
/// characters).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ParseError {
    pub line: u32,
    pub column: u32,
    pub message: String,
    /// The next thing to try.
    pub remedy: String,
}

impl fmt::Display for ParseError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}:{}: {}", self.line, self.column, self.message)
    }
}

impl std::error::Error for ParseError {}

/// A PNG that doesn't decode, or that pixel text can't hold, with the
/// first pixel at fault when one is.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PngError {
    /// The pixel at fault, from the top left.
    pub pixel: Option<(u32, u32)>,
    pub message: String,
    pub remedy: String,
}

impl fmt::Display for PngError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self.pixel {
            Some((x, y)) => write!(f, "pixel {x},{y}: {}", self.message),
            None => f.write_str(&self.message),
        }
    }
}

impl std::error::Error for PngError {}

/// Either form of image failing to load.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum PixelsError {
    Text(ParseError),
    Png(PngError),
}

impl PixelsError {
    pub fn remedy(&self) -> &str {
        match self {
            PixelsError::Text(e) => &e.remedy,
            PixelsError::Png(e) => &e.remedy,
        }
    }
}

impl fmt::Display for PixelsError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            PixelsError::Text(e) => e.fmt(f),
            PixelsError::Png(e) => e.fmt(f),
        }
    }
}

impl std::error::Error for PixelsError {}

impl From<ParseError> for PixelsError {
    fn from(e: ParseError) -> PixelsError {
        PixelsError::Text(e)
    }
}

impl From<PngError> for PixelsError {
    fn from(e: PngError) -> PixelsError {
        PixelsError::Png(e)
    }
}

/// A picture that can't be written as text: past [`MAX_SIDE`](crate::MAX_SIDE)
/// a side, or `--tiles` on an image whose
/// size isn't whole tiles.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LayoutError {
    pub message: String,
    pub remedy: String,
}

impl fmt::Display for LayoutError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.message)
    }
}

impl std::error::Error for LayoutError {}
