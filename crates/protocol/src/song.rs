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
    /// RGBDS source for hUGEDriver, as the `song` op answers it.
    Asm,
}

/// The `song` op: a song file's RGBDS source for hUGEDriver, as `build`
/// writes it for a song under `assets/music`, for a client that assembles
/// it in a build of its own. Needs no session.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SongArgs {
    /// A `.uge` or song text, relative to the server's working directory.
    pub path: String,
    /// The label the source declares for `hUGE_init`, which scopes every
    /// other label in it: an RGBDS symbol, letters, digits and `_`, not
    /// starting with a digit.
    pub descriptor: String,
}

/// What the `song` op answers.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct SongAsm {
    /// As given.
    pub path: String,
    pub descriptor: String,
    /// The RGBDS source: it includes `hUGE.inc`, declares `descriptor::`
    /// in a ROMX section, and defines `<descriptor>_TIMER` and
    /// `<descriptor>_TMA`. The bytes `gbcli song FILE --asm OUT` writes.
    pub asm: String,
    /// The timer modulo (TMA) when the song wants `hUGE_dosound` called
    /// from the timer interrupt at 4096 / (256 − TMA) Hz, as its
    /// `<descriptor>_TIMER` says; none when once per frame.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub timer: Option<u8>,
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
    /// The asm's descriptor, the label to pass `hUGE_init`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub descriptor: Option<String>,
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
    use crate::{Op, ResultBody, parse_request};

    #[test]
    fn a_song_request_names_a_file_and_a_descriptor_and_needs_no_session() {
        let line = r#"{"id":"1","op":"song","args":{"path":"assets/song/theme.uge","descriptor":"song_theme"}}"#;
        let request = parse_request(line).expect("parses");
        let op = Op::Song(SongArgs {
            path: "assets/song/theme.uge".into(),
            descriptor: "song_theme".into(),
        });
        assert_eq!(request.op, op);
        assert_eq!(op.name(), "song");
        assert!(!op.needs_bundle() && !op.changes_session() && !op.mutates_head());
        let extra = r#"{"id":"1","op":"song","args":{"path":"a.song","descriptor":"d","x":1}}"#;
        assert!(parse_request(extra).is_err(), "unknown fields are refused");
        let bare = r#"{"id":"1","op":"song","args":{"path":"a.song"}}"#;
        assert!(parse_request(bare).is_err(), "the descriptor is required");
    }

    #[test]
    fn the_answer_carries_the_asm_as_text_and_the_timer_only_when_on() {
        let made = |timer| ResultBody {
            song_asm: Some(SongAsm {
                path: "a.song".into(),
                descriptor: "song_a".into(),
                asm: "song_a::\n".into(),
                timer,
            }),
            ..ResultBody::default()
        };
        assert_eq!(
            serde_json::to_string(&made(None)).unwrap(),
            r#"{"song_asm":{"path":"a.song","descriptor":"song_a","asm":"song_a::\n"}}"#
        );
        let on = serde_json::to_string(&made(Some(200))).unwrap();
        assert_eq!(
            on,
            r#"{"song_asm":{"path":"a.song","descriptor":"song_a","asm":"song_a::\n","timer":200}}"#
        );
        assert_eq!(
            serde_json::from_str::<ResultBody>(&on).unwrap(),
            made(Some(200))
        );
    }

    #[test]
    fn song_can_write_asm() {
        assert_eq!(
            serde_json::to_string(&SongOutputKind::Asm).unwrap(),
            r#""asm""#
        );
    }

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
