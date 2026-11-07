//! The `sym` and `checkpoints` read verbs (design §12): rows of what the
//! session can name.

use serde::{Deserialize, Serialize};

use crate::rev::Rev;

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SymArgs {
    /// A glob over symbol names: `*` any run of characters, `?` any one.
    /// Names match as spelled, as RGBDS matches them.
    pub pattern: String,
}

/// The memory area a symbol's address falls in, which says what its bank means.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SymbolArea {
    Rom0,
    Romx,
    Vram,
    Sram,
    Wram0,
    Wramx,
    Echo,
    Oam,
    Unusable,
    Io,
    Hram,
    Ie,
}

/// One symbol from the session's `.sym`.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct SymbolRow {
    pub name: String,
    pub bank: u16,
    pub addr: u16,
    pub area: SymbolArea,
}

/// A named checkpoint or a history entry.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct CheckpointRow {
    /// What `--at` takes: the name, or `~N` for a history entry.
    pub at: String,
    /// The head's revision when it was saved: a history entry's own, or for
    /// a named checkpoint the one its file records. Absent when unknown: a
    /// checkpoint written before files recorded it, or one that didn't load.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub rev: Option<Rev>,
    /// Frames since the boot handoff; absent before it.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub frame: Option<i64>,
    /// Whether it was saved from the ROM the session's file holds now;
    /// absent when the entry didn't load.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub rom_matches: Option<bool>,
    /// Why the entry didn't load. One bad entry is a row, not a failed listing.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

#[cfg(test)]
mod tests {
    use crate::parse::parse_request;
    use crate::request::Op;

    #[test]
    fn sym_takes_a_pattern_and_checkpoints_takes_nothing() {
        let request =
            parse_request(r#"{"id":"1","op":"sym","args":{"pattern":"Main*"}}"#).expect("parses");
        let Op::Sym(args) = &request.op else {
            panic!("{request:?}")
        };
        assert_eq!(args.pattern, "Main*");
        let listed = parse_request(r#"{"id":"1","op":"checkpoints"}"#).expect("parses");
        assert_eq!(listed.op, Op::Checkpoints);
        assert!(!request.op.mutates_head() && !listed.op.mutates_head());
    }
}
