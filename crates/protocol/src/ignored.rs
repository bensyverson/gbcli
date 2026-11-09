//! Writes the hardware ignored, on the wire: a run's or step's
//! `ignored_writes`, every field of its warnings (leaf si5fnG).

use gbcli_core::IgnoredWriteKind;
use serde::{Deserialize, Serialize};

/// The program's writes of one kind the PPU ignored during a run or step,
/// and the first of them.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct IgnoredWritesRow {
    /// `vram` (written in mode 3) or `oam` (written in modes 2-3).
    #[serde(with = "IgnoredWriteKindDef")]
    pub kind: IgnoredWriteKind,
    /// How many, the first included.
    pub count: u64,
    /// The PC of the instruction that made the first.
    pub pc: u16,
    /// The symbol containing `pc`, as the stop record names it.
    pub symbol: Option<String>,
    /// The address the first was aimed at.
    pub addr: u16,
    /// The line (LY) the first landed on.
    pub line: u8,
    /// The frame the first landed in, counted as the stop record's.
    pub frame: i64,
}

#[derive(Serialize, Deserialize)]
#[serde(remote = "IgnoredWriteKind", rename_all = "snake_case")]
enum IgnoredWriteKindDef {
    Vram,
    Oam,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ResultBody;

    #[test]
    fn a_row_names_its_kind_and_every_field() {
        let row = IgnoredWritesRow {
            kind: IgnoredWriteKind::Oam,
            count: 23,
            pc: 0x2A31,
            symbol: Some("HudDraw+$12".into()),
            addr: 0xFE04,
            line: 2,
            frame: 140,
        };
        let json = serde_json::to_value(&row).unwrap();
        assert_eq!(
            json,
            serde_json::json!({
                "kind": "oam", "count": 23, "pc": 0x2A31, "symbol": "HudDraw+$12",
                "addr": 0xFE04, "line": 2, "frame": 140,
            })
        );
        let back: IgnoredWritesRow = serde_json::from_value(json).unwrap();
        assert_eq!(back, row);
    }

    #[test]
    fn a_result_without_ignored_writes_leaves_the_field_out() {
        let json = serde_json::to_value(ResultBody::default()).unwrap();
        assert!(json.get("ignored_writes").is_none(), "{json}");
        let with = ResultBody {
            ignored_writes: Some(Vec::new()),
            ..ResultBody::default()
        };
        assert!(
            serde_json::to_value(with)
                .unwrap()
                .get("ignored_writes")
                .is_some()
        );
    }
}
