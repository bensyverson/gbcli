//! One final response per request (design §5): success, or one error with its
//! exit code and a remedy.

use serde::{Deserialize, Serialize};

use crate::assets::AssetsMade;
use crate::audio::AudioMade;
use crate::battery::{FlashNoteRow, PowerCycleMade, SavMade};
use crate::checkpoint::{Location, WatchAccess};
use crate::equiv::EquivMade;
use crate::error::{ErrorBody, ErrorCode};
use crate::keys::KeyEvent;
use crate::listing::{CheckpointRow, SymbolRow};
use crate::opcode::OpcodeRow;
use crate::pixels::{PixelsMade, PixelsProblem};
use crate::profile::{FrameBudgetMade, MeasureMade, ProfileMade};
use crate::raster::RasterMade;
use crate::request::WriteKind;
use crate::rev::Rev;
use crate::session::{EventLogLine, GapCount, SessionRow};
use crate::shot::{FilmstripMade, ShotMade};
use crate::song::{SongMade, SongProblem};
use crate::stop::StopRecord;
use crate::toolchain::{BuildMade, DoctorMade, NewMade};
use crate::version::VersionMade;
use crate::video::ViewMade;
use crate::views::{PpuView, RegistersView};

/// The answer to one request, echoed with its id: a result or an error,
/// never both and never neither. On the wire it is
/// `{"id","ok","rev"?,"result"|"error","warnings"}`, and `ok` is derived from
/// which one is there, so it can't disagree.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(into = "WireResponse", try_from = "WireResponse")]
pub struct Response {
    pub id: String,
    pub rev: Option<Rev>,
    pub outcome: Result<ResultBody, ErrorBody>,
    /// Human-facing notes (never-written reads, a changed ROM that still
    /// loaded). The CLI prints them to stderr; they never fail a request.
    pub warnings: Vec<String>,
}

/// The wire shape of a [`Response`].
#[derive(Serialize, Deserialize)]
struct WireResponse {
    id: String,
    ok: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    rev: Option<Rev>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    result: Option<ResultBody>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    error: Option<ErrorBody>,
    warnings: Vec<String>,
}

impl From<Response> for WireResponse {
    fn from(r: Response) -> Self {
        let (result, error) = match r.outcome {
            Ok(result) => (Some(result), None),
            Err(error) => (None, Some(error)),
        };
        WireResponse {
            id: r.id,
            ok: error.is_none(),
            rev: r.rev,
            result,
            error,
            warnings: r.warnings,
        }
    }
}

impl TryFrom<WireResponse> for Response {
    type Error = String;

    fn try_from(w: WireResponse) -> Result<Self, String> {
        let outcome = match (w.ok, w.result, w.error) {
            (true, Some(result), None) => Ok(result),
            (false, None, Some(error)) => Err(error),
            (ok, result, error) => {
                return Err(format!(
                    "\"ok\":{ok} with {} and {}: a response has a result when ok, an error when not",
                    if result.is_some() {
                        "a result"
                    } else {
                        "no result"
                    },
                    if error.is_some() {
                        "an error"
                    } else {
                        "no error"
                    },
                ));
            }
        };
        Ok(Response {
            id: w.id,
            rev: w.rev,
            outcome,
            warnings: w.warnings,
        })
    }
}

impl Response {
    pub fn ok(id: impl Into<String>, rev: Option<Rev>, result: ResultBody) -> Self {
        Response {
            id: id.into(),
            rev,
            outcome: Ok(result),
            warnings: Vec::new(),
        }
    }

    pub fn failed(id: impl Into<String>, rev: Option<Rev>, error: ErrorBody) -> Self {
        Response {
            id: id.into(),
            rev,
            outcome: Err(error),
            warnings: Vec::new(),
        }
    }

    /// Whether the request succeeded: the wire's `ok`.
    pub fn is_ok(&self) -> bool {
        self.outcome.is_ok()
    }

    pub fn result(&self) -> Option<&ResultBody> {
        self.outcome.as_ref().ok()
    }

    pub fn error(&self) -> Option<&ErrorBody> {
        self.outcome.as_ref().err()
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
    pub shot: Option<ShotMade>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub filmstrip: Option<FilmstripMade>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub measure: Option<MeasureMade>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub profile: Option<ProfileMade>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub frame_budget: Option<FrameBudgetMade>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub raster: Option<RasterMade>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub equiv: Option<EquivMade>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub audio: Option<AudioMade>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub oam_log: Option<crate::video::OamLogMade>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub view: Option<ViewMade>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub symbols: Option<Vec<SymbolRow>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub checkpoints: Option<Vec<CheckpointRow>>,
    /// Key events the run stopped before reaching (design §11 step 3).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub keys_unapplied: Option<Vec<KeyEvent>>,
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
    /// Idle sessions `prune` kept because they hold named checkpoints.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub kept: Option<Vec<String>>,
    /// Idle sessions `prune` skipped because another process held their lock.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub busy: Option<Vec<String>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub removed: Option<String>,
    /// `build`: what it assembled, wrote and reported.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub build: Option<BuildMade>,
    /// `assets`: the images converted, as a build records them.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub assets: Option<AssetsMade>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub doctor: Option<DoctorMade>,
    /// `new`: the project it scaffolded.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub created: Option<NewMade>,
    /// `opcode`: every opcode the query names, in opcode order.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub opcodes: Option<Vec<OpcodeRow>>,
    /// `song`: the song read and the files written.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub song: Option<SongMade>,
    /// `song`: why the song file didn't load (with `song_unreadable`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub song_problem: Option<SongProblem>,
    /// `pixels`: the image read and the files written.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pixels: Option<PixelsMade>,
    /// `pixels`: why the image didn't load (with `pixels_unreadable`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pixels_problem: Option<PixelsProblem>,
    /// `power_cycle`: what the cycle carried across.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub power_cycle: Option<PowerCycleMade>,
    /// `sav`: the battery save written.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sav: Option<SavMade>,
    /// `run`: what the flash model noted during the run (commands that
    /// missed the board, or a mask-ROM cart's flash commands); each is also
    /// a warning.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub flash_notes: Option<Vec<FlashNoteRow>>,
    /// `version`: gbcli's version and the protocol's.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub version: Option<VersionMade>,
}

/// One `--read` result: an expression's `value`, or for a whole OAM entry
/// (`oam[N]`) its fields in `oam`, or, for a run's read that failed, its
/// `error`.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ReadValue {
    pub expr: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub value: Option<i64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub oam: Option<OamEntryRead>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub error: Option<ReadFailed>,
}

/// Why a run's read has no value: the error the `read` op would fail with,
/// kept in the read's own row so the stop and the other reads stand.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ReadFailed {
    pub code: ErrorCode,
    pub message: String,
    pub remedy: String,
}

/// `oam[N]` as a read (design §7): the entry's four bytes, then what the
/// attribute byte says.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct OamEntryRead {
    pub index: u8,
    pub y: u8,
    pub x: u8,
    pub tile: u8,
    pub attr: u8,
    /// Bit 7: background and window colors 1–3 are drawn over the object.
    pub priority: bool,
    pub yflip: bool,
    pub xflip: bool,
    /// Outside CGB mode only: bit 4, OBP0 or OBP1. CGB mode ignores it.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub palette: Option<u8>,
    /// CGB mode only: the tile's VRAM bank (bit 3).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cgb_bank: Option<u8>,
    /// CGB mode only: the object palette 0–7 (bits 0–2).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cgb_palette: Option<u8>,
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
    fn ok_cannot_disagree_with_result_and_error() {
        let error = r#"{"code":"bad_request","exit":2,"message":"m","remedy":"r"}"#;
        for line in [
            format!(r#"{{"id":"1","ok":true,"error":{error},"warnings":[]}}"#),
            r#"{"id":"1","ok":false,"result":{},"warnings":[]}"#.to_string(),
            r#"{"id":"1","ok":true,"warnings":[]}"#.to_string(),
            format!(r#"{{"id":"1","ok":true,"result":{{}},"error":{error},"warnings":[]}}"#),
        ] {
            assert!(serde_json::from_str::<Response>(&line).is_err(), "{line}");
        }
        let failed = Response::failed("1", None, ErrorBody::plain(ErrorCode::BadRequest, "m"));
        assert!(!failed.is_ok());
        let json = serde_json::to_value(&failed).unwrap();
        assert_eq!(json["ok"], false);
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
