//! Why a font didn't read: where, what, and what to do.

use std::fmt;

/// A font file that isn't gbcli's YAFF subset, located to a line and
/// column (both 1-based).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FontError {
    pub line: u32,
    pub column: u32,
    pub message: String,
    pub remedy: String,
}

impl FontError {
    pub(crate) fn new(
        line: u32,
        column: u32,
        message: impl Into<String>,
        remedy: impl Into<String>,
    ) -> FontError {
        FontError {
            line,
            column: column.max(1),
            message: message.into(),
            remedy: remedy.into(),
        }
    }
}

impl fmt::Display for FontError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}:{}: {}", self.line, self.column, self.message)
    }
}

impl std::error::Error for FontError {}
