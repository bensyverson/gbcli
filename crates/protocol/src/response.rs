//! One final response per request (design §5): success, or one error with its
//! exit code and a remedy.

use serde::{Deserialize, Serialize};

use crate::checkpoint::{Location, WatchAccess};
use crate::error::ErrorBody;
use crate::request::WriteKind;
use crate::rev::Rev;
use crate::session::{EventLogLine, GapCount, SessionRow};
use crate::stop::StopRecord;

/// The answer to one request, echoed with its id.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Response {
    pub id: String,
    pub ok: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub rev: Option<Rev>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub result: Option<ResultBody>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub error: Option<ErrorBody>,
    /// Human-facing notes (never-written reads, a changed ROM that still
    /// loaded). The CLI prints them to stderr; they never fail a request.
    pub warnings: Vec<String>,
}

impl Response {
    pub fn ok(id: impl Into<String>, rev: Option<Rev>, result: ResultBody) -> Self {
        Response {
            id: id.into(),
            ok: true,
            rev,
            result: Some(result),
            error: None,
            warnings: Vec::new(),
        }
    }

    pub fn failed(id: impl Into<String>, rev: Option<Rev>, error: ErrorBody) -> Self {
        Response {
            id: id.into(),
            ok: false,
            rev,
            result: None,
            error: Some(error),
            warnings: Vec::new(),
        }
    }

    pub fn with_warnings(mut self, warnings: Vec<String>) -> Self {
        self.warnings = warnings;
        self
    }
}

/// What a successful request produced. Only the fields its op fills are on
/// the wire, so the shape stays narrow.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct ResultBody {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub stop: Option<StopRecord>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reads: Option<Vec<ReadValue>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub checkpoint: Option<CheckpointMade>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub restored: Option<Restored>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub writes: Option<Vec<WriteMade>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub breakpoint: Option<BreakpointMade>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub watch: Option<WatchMade>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub deleted: Option<Deleted>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub registers: Option<RegistersView>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ppu: Option<PpuView>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sessions: Option<Vec<SessionRow>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub events: Option<Vec<EventLogLine>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub gaps: Option<Vec<GapCount>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pruned: Option<Vec<String>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub removed: Option<String>,
}

/// One `--read` result.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ReadValue {
    pub expr: String,
    pub value: i64,
}

/// A checkpoint that was written.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct CheckpointMade {
    pub name: String,
    /// The bundle-relative path.
    pub path: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub frame: Option<u64>,
    pub rev: Rev,
}

/// A state that was restored (a checkpoint, a history entry, or `undo`).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Restored {
    /// The checkpoint name, or `None` for a history entry.
    pub name: Option<String>,
    pub rev: Rev,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub frame: Option<u64>,
}

/// A write that was applied.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct WriteMade {
    pub kind: WriteKind,
    pub target: String,
    #[serde(with = "crate::hex::bytes")]
    pub bytes: Vec<u8>,
    /// The CPU address a poke or bus write landed on, when it is one address.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub addr: Option<u16>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct BreakpointMade {
    pub id: u32,
    pub at: Location,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct WatchMade {
    pub id: u32,
    pub access: WatchAccess,
    pub at: Location,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Deleted {
    pub id: u32,
    pub found: bool,
}

/// CPU registers with the pairs and flags spelled out (design §12 `show`).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct RegistersView {
    pub a: u8,
    pub f: u8,
    pub b: u8,
    pub c: u8,
    pub d: u8,
    pub e: u8,
    pub h: u8,
    pub l: u8,
    pub sp: u16,
    pub pc: u16,
    pub af: u16,
    pub bc: u16,
    pub de: u16,
    pub hl: u16,
    pub flags: FlagsView,
    pub ime: bool,
    pub halted: bool,
    pub double_speed: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct FlagsView {
    pub z: bool,
    pub n: bool,
    pub h: bool,
    pub c: bool,
}

impl RegistersView {
    pub fn of(r: &gbcli_core::Registers) -> Self {
        let pair = |hi: u8, lo: u8| u16::from_be_bytes([hi, lo]);
        RegistersView {
            a: r.a,
            f: r.f,
            b: r.b,
            c: r.c,
            d: r.d,
            e: r.e,
            h: r.h,
            l: r.l,
            sp: r.sp,
            pc: r.pc,
            af: pair(r.a, r.f),
            bc: pair(r.b, r.c),
            de: pair(r.d, r.e),
            hl: pair(r.h, r.l),
            flags: FlagsView {
                z: r.f & 0x80 != 0,
                n: r.f & 0x40 != 0,
                h: r.f & 0x20 != 0,
                c: r.f & 0x10 != 0,
            },
            ime: r.ime,
            halted: r.halted,
            double_speed: r.double_speed,
        }
    }
}

/// The PPU as the core reports it.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PpuView {
    pub ly: u8,
    pub mode: PpuModeView,
    pub lcd_on: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub dot: Option<u16>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PpuModeView {
    Hblank,
    Vblank,
    OamScan,
    Drawing,
}

impl PpuView {
    pub fn of(p: &gbcli_core::PpuStatus) -> Self {
        PpuView {
            ly: p.ly,
            mode: match p.mode {
                gbcli_core::PpuMode::HBlank => PpuModeView::Hblank,
                gbcli_core::PpuMode::VBlank => PpuModeView::Vblank,
                gbcli_core::PpuMode::OamScan => PpuModeView::OamScan,
                gbcli_core::PpuMode::Drawing => PpuModeView::Drawing,
            },
            lcd_on: p.lcd_on,
            dot: p.dot,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::error::ErrorCode;
    use crate::stop::{BudgetLimits, BudgetUse, StopReason, StopTime, TimeBase};

    fn stop() -> StopRecord {
        StopRecord {
            reason: StopReason::BudgetExhausted {
                kind: gbcli_core::BudgetKind::Frames,
            },
            pc: 0x0150,
            bank: 0,
            symbol: Some("Main".into()),
            time: StopTime {
                ticks: 100,
                t: 50,
                since: TimeBase::Boot,
                frame: 600,
                ly: 144,
                dot: None,
            },
            frame_shown: None,
            budget: BudgetUse {
                ticks: 100,
                instructions: 10,
                frames: 600,
                limits: BudgetLimits {
                    frames: Some(600),
                    ..BudgetLimits::default()
                },
                defaulted: true,
            },
        }
    }

    #[test]
    fn a_success_carries_its_result_and_echoes_the_id() {
        let response = Response::ok(
            "7",
            Some(Rev(7)),
            ResultBody {
                stop: Some(stop()),
                ..ResultBody::default()
            },
        );
        let json = serde_json::to_value(&response).unwrap();
        assert_eq!(json["id"], "7");
        assert_eq!(json["ok"], true);
        assert_eq!(json["rev"], "s7");
        assert_eq!(json["result"]["stop"]["reason"], "budget_exhausted");
        assert_eq!(json["warnings"], serde_json::json!([]));
        assert!(json.get("error").is_none());
    }

    #[test]
    fn a_failure_carries_one_error_with_its_exit_code() {
        let response = Response::failed(
            "8",
            Some(Rev(3)),
            ErrorBody::new(
                ErrorCode::BudgetExhausted,
                "ran 600 frames; the predicate never held",
                "raise --frames",
            ),
        );
        let json = serde_json::to_value(&response).unwrap();
        assert_eq!(json["ok"], false);
        assert_eq!(json["error"]["code"], "budget_exhausted");
        assert_eq!(json["error"]["exit"], 3);
        assert!(json.get("result").is_none());
    }

    #[test]
    fn registers_view_spells_out_pairs_and_flags() {
        let view = RegistersView::of(&gbcli_core::Registers {
            a: 0x12,
            f: 0xE0,
            b: 0x34,
            c: 0x56,
            d: 0x78,
            e: 0x9A,
            h: 0xBC,
            l: 0xDE,
            sp: 0xFFFE,
            pc: 0x0150,
            ime: true,
            halted: false,
            double_speed: false,
        });
        assert_eq!(view.af, 0x12E0);
        assert_eq!(view.bc, 0x3456);
        assert_eq!(view.hl, 0xBCDE);
        assert!(view.flags.z && view.flags.n && view.flags.h && !view.flags.c);
    }
}
