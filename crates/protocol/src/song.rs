//! What `gbcli song` reports (design §12, the 2026-11-07 music update): the
//! song it read, the files it wrote, or the problem that stopped it.
//! `gbcli-music` reads and converts; the CLI fills these and prints them.

use serde::{Deserialize, Serialize};

/// A song file's form.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum SongFileFormat {
    /// hUGETracker's binary `.uge`.
    Uge,
    /// The text form (`format gbcli-song 1`).
    Text,
}

/// What `song` wrote.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum SongOutputKind {
    Text,
    Uge,
    Piano,
}

/// A song file read, and what was made from it.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct SongMade {
    /// As given, relative to the working directory.
    pub path: String,
    pub format: SongFileFormat,
    /// The `.uge` version read (0-6), when the file was a `.uge`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub uge_version: Option<u32>,
    /// The song's name, with bytes that aren't UTF-8 replaced.
    pub name: String,
    /// Instruments that differ from hUGETracker's defaults: the ones the
    /// text lists.
    pub instruments: u32,
    pub patterns: u32,
    /// Order rows.
    pub orders: u32,
    pub wrote: Vec<SongOutput>,
}

/// A file `song` wrote.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct SongOutput {
    pub kind: SongOutputKind,
    pub path: String,
    pub bytes: u64,
    /// The piano roll's size, notes drawn and rows walked.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub width: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub height: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub notes: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub rows: Option<u32>,
}

/// Why a song file didn't load: in song text, a line and column; in a
/// `.uge`, a byte offset.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct SongProblem {
    pub path: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub line: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub column: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub offset: Option<u64>,
    pub message: String,
    pub remedy: String,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_text_problem_carries_its_line_and_column_and_no_offset() {
        let p = SongProblem {
            path: "a.song".into(),
            line: Some(5),
            column: Some(21),
            offset: None,
            message: "`H-5` is not a note".into(),
            remedy: "notes are C-3 to B-8".into(),
        };
        let json = serde_json::to_string(&p).unwrap();
        assert_eq!(
            json,
            r#"{"path":"a.song","line":5,"column":21,"message":"`H-5` is not a note","remedy":"notes are C-3 to B-8"}"#
        );
        assert_eq!(serde_json::from_str::<SongProblem>(&json).unwrap(), p);
    }
}
