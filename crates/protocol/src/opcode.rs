//! `opcode`: an instruction's size, cycles and flags from the sourced opcode
//! table (gbdev's, CC0), found by byte or by the instruction's text. Needs
//! no session; the engine's `disasm::lookup` answers it.

use serde::{Deserialize, Serialize};

/// What to look up: `$20`, `$CB37`, `jr nz`, `ld a, [hl+]`, `ld [wFoo], a`.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct OpcodeArgs {
    pub query: String,
}

/// One opcode the query names.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct OpcodeRow {
    /// `$20`, or `$CB37` for a CB-prefixed opcode.
    pub code: String,
    /// As RGBDS writes it, with the table's placeholders for data:
    /// `jr nz, e8`, `ld a, [hl+]`, `ldh [a8], a`.
    pub asm: String,
    /// Bytes, including the 0xCB prefix.
    pub bytes: u8,
    /// T-cycles at single speed; for a conditional branch, when taken.
    pub t: u8,
    /// A conditional branch's T-cycles when not taken.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub t_not_taken: Option<u8>,
    /// Z, N, H and C in that order: the flag's letter when the result sets
    /// it, 0 or 1 when forced, `-` when untouched.
    pub flags: String,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Op, parse_request};

    #[test]
    fn an_opcode_request_parses_and_needs_no_session() {
        let line = r#"{"id":"1","op":"opcode","args":{"query":"jr nz"}}"#;
        let request = parse_request(line).expect("parses");
        let op = Op::Opcode(OpcodeArgs {
            query: "jr nz".into(),
        });
        assert_eq!(request.op, op);
        assert_eq!(op.name(), "opcode");
        assert!(!op.needs_bundle() && !op.changes_session() && !op.mutates_head());
        assert!(parse_request(r#"{"id":"1","op":"opcode","args":{"q":"x"}}"#).is_err());
    }

    #[test]
    fn a_row_drops_the_not_taken_cost_it_lacks() {
        let row = OpcodeRow {
            code: "$00".into(),
            asm: "nop".into(),
            bytes: 1,
            t: 4,
            t_not_taken: None,
            flags: "----".into(),
        };
        let json = serde_json::to_string(&row).unwrap();
        assert_eq!(
            json,
            r#"{"code":"$00","asm":"nop","bytes":1,"t":4,"flags":"----"}"#
        );
    }
}
