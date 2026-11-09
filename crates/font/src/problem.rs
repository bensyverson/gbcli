//! Why a working copy (a PNG grid and its `.chars`, or a BDF file) didn't
//! convert: where (a cell of the grid, a line of a text file, a glyph),
//! what, and what to do. A conversion reports every problem it finds, not
//! only the first.

use std::fmt;

use crate::key::Key;

/// One thing a working copy gets wrong.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Problem {
    /// The grid cell at fault, as (column, row) of 8x8 cells.
    pub cell: Option<(u32, u32)>,
    /// The line at fault in a text file (`.chars`, BDF), 1-based.
    pub line: Option<u32>,
    /// The column at fault on that line, 1-based in characters.
    pub column: Option<u32>,
    /// The glyph at fault, when it has a key.
    pub glyph: Option<Key>,
    pub message: String,
    pub remedy: String,
}

impl Problem {
    pub(crate) fn new(message: impl Into<String>, remedy: impl Into<String>) -> Problem {
        Problem {
            cell: None,
            line: None,
            column: None,
            glyph: None,
            message: message.into(),
            remedy: remedy.into(),
        }
    }

    pub(crate) fn at_cell(mut self, column: u32, row: u32) -> Problem {
        self.cell = Some((column, row));
        self
    }

    pub(crate) fn at_line(mut self, line: usize, column: Option<usize>) -> Problem {
        self.line = Some(line as u32);
        self.column = column.map(|c| c as u32);
        self
    }

    pub(crate) fn of(mut self, key: &Key) -> Problem {
        self.glyph = Some(key.clone());
        self
    }
}

impl fmt::Display for Problem {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if let Some((x, y)) = self.cell {
            write!(f, "cell {x},{y}: ")?;
        }
        if let Some(line) = self.line {
            match self.column {
                Some(c) => write!(f, "{line}:{c}: ")?,
                None => write!(f, "{line}: ")?,
            }
        }
        f.write_str(&self.message)
    }
}
