//! Session metadata and log lines: what `session.json` holds, and what the
//! event log and gap ledger store (design §1, §10).

use std::collections::BTreeMap;

use gbcli_core::Model;
use serde::{Deserialize, Serialize};

use crate::checkpoint::{DebuggerConfig, ModelDef, RenderConfig};
use crate::error::ErrorCode;
use crate::request::Op;
use crate::rev::Rev;
use crate::stop::{StopReason, StopRecord, TimeBase};

/// What a session name may be, as a remedy states it.
pub const SESSION_NAME_RULE: &str = "[a-z0-9-], 1 to 40 characters, not starting with -";

/// Whether `name` can name a session ([`SESSION_NAME_RULE`]): `-` first
/// would read as a flag on the command line.
pub fn is_session_name(name: &str) -> bool {
    (1..=40).contains(&name.len())
        && !name.starts_with('-')
        && name
            .chars()
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-')
}

/// `sessions/<name>/session.json`: everything a machine is rebuilt from, plus
/// the revision counter (design §1).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct SessionFile {
    pub name: String,
    /// The ROM path, relative to the bundle's project directory when it is
    /// inside it, else absolute.
    pub rom: String,
    /// The `.sym` path, by the same rule.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sym: Option<String>,
    #[serde(with = "ModelDef")]
    pub model: Model,
    pub seed: u64,
    pub rev: Rev,
    pub config: RenderConfig,
    pub debugger: DebuggerConfig,
    /// The stop record of the last run, for `show` (design §12).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub last_stop: Option<StopRecord>,
    /// Set when the session was created, and different every time, so a
    /// session removed and created again is told apart from the old one at
    /// the same rev. 0 in a session made before the field existed.
    #[serde(default)]
    pub incarnation: u64,
    /// The undo walk the head is part of, if its last change was an undo
    /// (design §10); any other head change ends it.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub undo: Option<UndoWalk>,
    /// The cart's flash board, by profile name (`--flash`), fixed when the
    /// session is created. Absent on mask ROM and in sessions made before
    /// flash sessions existed.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub flash: Option<String>,
}

/// Where consecutive undos have got to (design §10).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct UndoWalk {
    /// The history entry the head was restored from; the next undo restores
    /// the one before it.
    pub restored: Rev,
    /// The head when the walk began. History is kept back from here, not from
    /// the head, until the walk ends, so each undo doesn't cost an entry.
    pub origin: Rev,
}

/// What the ROM header's CGB flag ($0143) says the game supports.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CgbFlag {
    /// Bit 7 clear: a DMG game, which a CGB runs in its DMG mode.
    Dmg,
    /// Bit 7 set (`rgbfix -c`, `$80`): CGB mode on a CGB, DMG mode on a DMG.
    Dual,
    /// `$C0` (`rgbfix -C`): made for a CGB only.
    Only,
}

impl CgbFlag {
    /// The wire spelling.
    pub fn name(self) -> &'static str {
        match self {
            CgbFlag::Dmg => "dmg",
            CgbFlag::Dual => "dual",
            CgbFlag::Only => "only",
        }
    }
}

/// Which machine a session runs and what its ROM was built for: `show`'s
/// `machine`.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct MachineView {
    #[serde(with = "ModelDef")]
    pub model: Model,
    /// The loaded ROM's header CGB flag.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cgb: Option<CgbFlag>,
}

/// One row of `gbcli sessions`.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct SessionRow {
    pub name: String,
    pub rom: String,
    /// The model the session was created with; absent in a response from
    /// before the field, where it is unknown.
    #[serde(default, with = "opt_model", skip_serializing_if = "Option::is_none")]
    pub model: Option<Model>,
    /// The header CGB flag of the ROM file the session names, as it is on
    /// disk now; absent when the file can't be read.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cgb: Option<CgbFlag>,
    /// The head's frame, counted from `head_since` as a stop record counts
    /// it: the boot handoff, or power-on before it. Absent with no head.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub head_frame: Option<i64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub head_since: Option<TimeBase>,
    pub rev: Rev,
    /// The session directory's size in bytes, excluding named checkpoints.
    pub size_bytes: u64,
    /// Host time of the session's last use, in Unix seconds. Used only for
    /// pruning, never by emulation.
    pub last_used: u64,
    pub checkpoints: usize,
}

/// An optional [`Model`] by its wire name, through [`ModelDef`].
mod opt_model {
    use gbcli_core::Model;
    use serde::{Deserialize, Deserializer, Serialize, Serializer};

    use crate::checkpoint::ModelDef;

    #[derive(Serialize, Deserialize)]
    struct Named(#[serde(with = "ModelDef")] Model);

    pub fn serialize<S: Serializer>(model: &Option<Model>, s: S) -> Result<S::Ok, S::Error> {
        model.map(Named).serialize(s)
    }

    pub fn deserialize<'de, D: Deserializer<'de>>(d: D) -> Result<Option<Model>, D::Error> {
        Ok(Option::<Named>::deserialize(d)?.map(|Named(m)| m))
    }
}

/// One line of `events.jsonl`: who changed what, and the revisions either side.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct EventLogLine {
    /// Host time, in Unix seconds.
    pub at: u64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub actor: Option<String>,
    /// The op and its args, spelled as in the request (design §10).
    #[serde(flatten)]
    pub op: Op,
    /// The request's `allow_rom_change` (design §6: a forced load is logged).
    #[serde(default, skip_serializing_if = "crate::request::is_false")]
    pub allow_rom_change: bool,
    pub ok: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub code: Option<ErrorCode>,
    pub rev_before: Rev,
    pub rev_after: Rev,
    /// Gap-ledger counts for this session after the request, when it added any.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub gaps: BTreeMap<String, u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub stop: Option<StopSummary>,
}

/// The short form of a stop record kept in the event log.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct StopSummary {
    #[serde(flatten)]
    pub reason: StopReason,
    pub pc: u16,
    pub frame: i64,
    pub ticks: u64,
}

/// One line of `gaps.jsonl`: something the core said it can't do (design §10).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct GapLine {
    pub at: u64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub session: Option<String>,
    pub op: String,
    pub feature: String,
    pub reason: String,
}

/// A feature's count, as `gbcli gaps` prints.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct GapCount {
    pub feature: String,
    pub count: u64,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::request::RunArgs;

    #[test]
    fn a_session_file_round_trips_with_a_readable_model() {
        let file = SessionFile {
            name: "demo".into(),
            rom: "demo.gb".into(),
            sym: Some("demo.sym".into()),
            model: Model::DmgB,
            seed: 0,
            rev: Rev(3),
            config: RenderConfig {
                rtc: gbcli_core::RtcMode::Accurate,
                rendering: gbcli_core::Rendering::On,
                joypad_bounce: gbcli_core::JoypadBounce::Off,
                write_tracking: gbcli_core::WriteTracking::On,
            },
            debugger: DebuggerConfig::default(),
            last_stop: None,
            incarnation: 7,
            undo: Some(UndoWalk {
                restored: Rev(2),
                origin: Rev(3),
            }),
            flash: Some("generic-wr-aaa-a9".into()),
        };
        let json = serde_json::to_value(&file).unwrap();
        assert_eq!(json["flash"], "generic-wr-aaa-a9");
        assert_eq!(json["model"], "dmg-b");
        assert_eq!(json["rev"], "s3");
        assert_eq!(json["seed"], 0);
        assert_eq!(serde_json::from_value::<SessionFile>(json).unwrap(), file);
    }

    #[test]
    fn a_session_file_from_before_flash_sessions_reads_as_mask_rom() {
        let json = serde_json::json!({
            "name": "demo", "rom": "demo.gb", "model": "dmg-b", "seed": 0, "rev": "s1",
            "config": serde_json::to_value(RenderConfig {
                rtc: gbcli_core::RtcMode::Accurate,
                rendering: gbcli_core::Rendering::On,
                joypad_bounce: gbcli_core::JoypadBounce::Off,
                write_tracking: gbcli_core::WriteTracking::On,
            }).unwrap(),
            "debugger": serde_json::to_value(DebuggerConfig::default()).unwrap(),
        });
        let file: SessionFile = serde_json::from_value(json).unwrap();
        assert_eq!(file.flash, None);
        assert!(serde_json::to_value(&file).unwrap().get("flash").is_none());
    }

    /// A newer client reads an older serve's `sessions` rows, which carry no
    /// model: the model is unknown then, not guessed.
    #[test]
    fn a_session_row_from_before_the_model_reads_with_no_model() {
        let json = serde_json::json!({
            "name": "demo", "rom": "demo.gb", "rev": "s1", "size_bytes": 10,
            "last_used": 0, "checkpoints": 0,
        });
        let row: SessionRow = serde_json::from_value(json).unwrap();
        assert_eq!(row.model, None);
        assert!(serde_json::to_value(&row).unwrap().get("model").is_none());
        let named = SessionRow {
            model: Some(Model::CgbE),
            ..row
        };
        let json = serde_json::to_value(&named).unwrap();
        assert_eq!(json["model"], "cgb-e");
        assert_eq!(serde_json::from_value::<SessionRow>(json).unwrap(), named);
    }

    #[test]
    fn an_event_line_carries_the_op_and_its_args() {
        let line = EventLogLine {
            at: 1,
            actor: Some("claude".into()),
            op: Op::Run(RunArgs {
                frames: Some(3),
                ..RunArgs::default()
            }),
            allow_rom_change: true,
            ok: true,
            code: None,
            rev_before: Rev(1),
            rev_after: Rev(2),
            gaps: BTreeMap::new(),
            stop: None,
        };
        let json = serde_json::to_value(&line).unwrap();
        assert_eq!(json["op"], "run");
        assert_eq!(json["args"]["frames"], 3);
        assert_eq!(
            json["allow_rom_change"], true,
            "beside the op, as in the request"
        );
        assert_eq!(serde_json::from_value::<EventLogLine>(json).unwrap(), line);
    }

    #[test]
    fn an_event_line_keeps_its_gap_counts_out_of_the_way_when_empty() {
        let line = EventLogLine {
            at: 1,
            actor: Some("claude".into()),
            op: Op::Show,
            allow_rom_change: false,
            ok: true,
            code: None,
            rev_before: Rev(1),
            rev_after: Rev(2),
            gaps: BTreeMap::new(),
            stop: None,
        };
        let json = serde_json::to_value(&line).unwrap();
        assert!(json.get("gaps").is_none());
        assert_eq!(json["rev_before"], "s1");
        assert_eq!(json["rev_after"], "s2");
    }
}
