//! Why a song didn't load: where, what, and what to do about it.

use std::fmt;

/// A song text that doesn't parse, located to a line and column (both
/// 1-based; the column counts characters).
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

/// A `.uge` file that doesn't read, located to a byte offset.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct UgeError {
    pub offset: usize,
    pub message: String,
    pub remedy: String,
}

impl fmt::Display for UgeError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "byte {}: {}", self.offset, self.message)
    }
}

impl std::error::Error for UgeError {}

/// Either kind of song file failing to load.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum SongError {
    Text(ParseError),
    Uge(UgeError),
}

impl SongError {
    pub fn message(&self) -> String {
        self.to_string()
    }

    pub fn remedy(&self) -> &str {
        match self {
            SongError::Text(e) => &e.remedy,
            SongError::Uge(e) => &e.remedy,
        }
    }
}

impl fmt::Display for SongError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            SongError::Text(e) => e.fmt(f),
            SongError::Uge(e) => e.fmt(f),
        }
    }
}

impl std::error::Error for SongError {}

impl From<ParseError> for SongError {
    fn from(e: ParseError) -> SongError {
        SongError::Text(e)
    }
}

impl From<UgeError> for SongError {
    fn from(e: UgeError) -> SongError {
        SongError::Uge(e)
    }
}
