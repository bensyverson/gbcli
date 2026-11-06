//! Session metadata and log lines: what `session.json` holds, and what the
//! event log and gap ledger store (design §1, §10).

use std::collections::BTreeMap;

use gbcli_core::Model;
use serde::{Deserialize, Serialize};

use crate::checkpoint::{DebuggerConfig, ModelDef, RenderConfig};
use crate::error::ErrorCode;
use crate::rev::Rev;
use crate::stop::{StopReason, StopRecord};

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
}

/// One row of `gbcli sessions`.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct SessionRow {
    pub name: String,
    pub rom: String,
    /// The frame the head holds, if there is one.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub head_frame: Option<u64>,
    pub rev: Rev,
    /// The session directory's size in bytes, excluding named checkpoints.
    pub size_bytes: u64,
    /// Host time of the session's last use, in Unix seconds. Used only for
    /// pruning, never by emulation.
    pub last_used: u64,
    pub checkpoints: usize,
}

/// One line of `events.jsonl`: who changed what, and the revisions either side.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct EventLogLine {
    /// Host time, in Unix seconds.
    pub at: u64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub actor: Option<String>,
    /// The op's wire name.
    pub op: String,
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
        };
        let json = serde_json::to_value(&file).unwrap();
        assert_eq!(json["model"], "dmg-b");
        assert_eq!(json["rev"], "s3");
        assert_eq!(json["seed"], 0);
        assert_eq!(serde_json::from_value::<SessionFile>(json).unwrap(), file);
    }

    #[test]
    fn an_event_line_keeps_its_gap_counts_out_of_the_way_when_empty() {
        let line = EventLogLine {
            at: 1,
            actor: Some("claude".into()),
            op: "run".into(),
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
