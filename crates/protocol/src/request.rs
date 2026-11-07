//! One request per JSON line (design §5). The server and the CLI build the
//! same structs; `op` and `args` are adjacent tags at the top level.

use serde::{Deserialize, Serialize};

use crate::audio::AudioArgs;
use crate::battery::{PowerCycleArgs, SavArgs};
use crate::checkpoint::WatchAccess;
use crate::equiv::EquivArgs;
use crate::keys::KeyEvent;
use crate::listing::SymArgs;
use crate::opcode::OpcodeArgs;
use crate::parse::WireRequest;
use crate::profile::{MeasureArgs, ProfileArgs, RunMeasure};
use crate::rev::Rev;
use crate::shot::{FilmstripArgs, ShotArgs, ShotReadArgs};
use crate::stop::{StepKind, TimeBase};
use crate::toolchain::{BuildArgs, NewArgs};
use crate::video::ViewArgs;

/// A request: who asks, which session and revision it saw, and what to do.
/// Reading one refuses fields it doesn't know, at the top level and in the
/// args (see [`crate::parse_request`]).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(try_from = "WireRequest")]
pub struct Request {
    /// Chosen by the client and echoed back; required.
    pub id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub session: Option<String>,
    #[serde(default, rename = "as", skip_serializing_if = "Option::is_none")]
    pub actor: Option<String>,
    /// The revision the caller read; a mutation may pass it (design §5).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub rev: Option<Rev>,
    /// Load a stored state (the head, `--at`, a checkpoint, a history entry)
    /// under a rebuilt ROM even when the rebuilt-ROM check refuses it
    /// (design §6). One field for every op that loads a state.
    #[serde(default, skip_serializing_if = "is_false")]
    pub allow_rom_change: bool,
    #[serde(flatten)]
    pub op: Op,
}

/// What a request does. `{"op":"run","args":{…}}`; a unit op is `{"op":"show"}`.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "op", content = "args", rename_all = "snake_case")]
pub enum Op {
    Run(RunArgs),
    Step(StepArgs),
    Show,
    Read(ReadArgs),
    Break(BreakArgs),
    Watch(WatchArgs),
    Delete(DeleteArgs),
    Checkpoint(CheckpointArgs),
    Restore(RestoreArgs),
    Undo,
    Write(WriteArgs),
    Sessions,
    Prune(PruneArgs),
    Rm(RmArgs),
    Events(EventsArgs),
    Gaps,
    Shot(ShotReadArgs),
    View(ViewArgs),
    Sym(SymArgs),
    Checkpoints,
    Measure(MeasureArgs),
    Equiv(Box<EquivArgs>),
    /// The RGBDS loop (design §12): no session, except for a build's restore.
    Build(BuildArgs),
    Doctor,
    New(NewArgs),
    /// An instruction's size, cycles and flags; needs no session.
    Opcode(OpcodeArgs),
    /// Battery saves (design §6): power-on again keeping the battery, and
    /// the battery written out as a `.sav`.
    PowerCycle(PowerCycleArgs),
    Sav(SavArgs),
}

impl Op {
    /// The wire name of the op, for the event log and gap ledger.
    pub fn name(&self) -> &'static str {
        match self {
            Op::Run(_) => "run",
            Op::Step(_) => "step",
            Op::Show => "show",
            Op::Read(_) => "read",
            Op::Break(_) => "break",
            Op::Watch(_) => "watch",
            Op::Delete(_) => "delete",
            Op::Checkpoint(_) => "checkpoint",
            Op::Restore(_) => "restore",
            Op::Undo => "undo",
            Op::Write(_) => "write",
            Op::Sessions => "sessions",
            Op::Prune(_) => "prune",
            Op::Rm(_) => "rm",
            Op::Events(_) => "events",
            Op::Gaps => "gaps",
            Op::Shot(_) => "shot",
            Op::View(_) => "view",
            Op::Sym(_) => "sym",
            Op::Checkpoints => "checkpoints",
            Op::Measure(_) => "measure",
            Op::Equiv(_) => "equiv",
            Op::Build(_) => "build",
            Op::Doctor => "doctor",
            Op::New(_) => "new",
            Op::Opcode(_) => "opcode",
            Op::PowerCycle(_) => "power_cycle",
            Op::Sav(_) => "sav",
        }
    }

    /// Whether applying it changes the head, and so bumps the revision.
    pub fn mutates_head(&self) -> bool {
        match self {
            Op::Build(args) => args.restore.is_some(),
            op => matches!(
                op,
                Op::Run(_)
                    | Op::Step(_)
                    | Op::Restore(_)
                    | Op::Undo
                    | Op::Write(_)
                    | Op::PowerCycle(_)
            ),
        }
    }

    /// Whether answering it needs the project's bundle: every op but
    /// `doctor`, `new`, `opcode` and a `build` that restores nothing,
    /// which a client with no `.gbcli/` yet can still send.
    pub fn needs_bundle(&self) -> bool {
        match self {
            Op::Doctor | Op::New(_) | Op::Opcode(_) => false,
            Op::Build(args) => args.restore.is_some(),
            _ => true,
        }
    }

    /// Whether applying it changes the session: its head, or its named
    /// checkpoints, breakpoints or watches. A request's `rev` guards these.
    pub fn changes_session(&self) -> bool {
        self.mutates_head()
            || matches!(
                self,
                Op::Checkpoint(_) | Op::Break(_) | Op::Watch(_) | Op::Delete(_) | Op::Equiv(_)
            )
    }
}

/// The names a request uses: everything after `--`.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RunArgs {
    /// A named checkpoint, or `~N` for a history entry.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub at: Option<String>,
    /// Where to start from when there is no head: the boot handoff (the
    /// default), or power-on, which also skips the head.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub from: Option<Start>,
    /// What `frame` and `t` count from (design §9).
    #[serde(default, skip_serializing_if = "Option::is_none", with = "since_word")]
    pub since: Option<TimeBase>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub until: Option<String>,
    /// An address: a temporary breakpoint.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub to: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub frames: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub instructions: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ticks: Option<u64>,
    /// Applied in order, before the run (design §11 step 2).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub writes: Vec<WriteArg>,
    /// Key presses and releases at frame offsets from the run's start
    /// (design §11 step 3), in the order given.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub keys: Vec<KeyEvent>,
    /// Conditions evaluated at the stop (design §11 step 6).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub read: Vec<String>,
    /// A screenshot of the stop, after the reads (design §11 step 6). Boxed
    /// to keep `Op` small.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub shot: Option<Box<ShotArgs>>,
    /// What the run told the APU and what it played, collected during the
    /// run and written after the shot (design §9). Boxed to keep `Op` small.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub audio: Option<Box<AudioArgs>>,
    /// Frames collected during the run, written after the shot (design §9).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub filmstrip: Option<Box<FilmstripArgs>>,
    /// One measurement on a copy of the stop (design §11 step 5).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub measure: Option<Box<RunMeasure>>,
    /// Cycles per routine over the run.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub profile: Option<ProfileArgs>,
    /// Active and idle cycles of each frame the run completed, and lag frames.
    #[serde(default, skip_serializing_if = "is_false")]
    pub frame_budget: bool,
    /// The IO-register writes the run made, by frame and line.
    #[serde(default, skip_serializing_if = "is_false")]
    pub raster: bool,
    /// Save the stop state under this name.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub checkpoint: Option<String>,
    #[serde(default, skip_serializing_if = "is_false")]
    pub allow_uninit: bool,
}

/// Where a run starts when it doesn't start from the head (design §9).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Start {
    /// The boot handoff, from the boot cache when it exists.
    Boot,
    /// Power-on, skipping the head and the boot cache: `--from power-on`.
    PowerOn,
}

/// `--since` on the wire is one word: `boot`, `power-on`, or a checkpoint
/// name. A checkpoint can't be named `boot` or `power-on` and be meant here.
pub(crate) mod since_word {
    use serde::{Deserialize, Deserializer, Serializer};

    use crate::stop::TimeBase;

    pub(crate) fn serialize<S: Serializer>(
        base: &Option<TimeBase>,
        s: S,
    ) -> Result<S::Ok, S::Error> {
        match base {
            None => s.serialize_none(),
            Some(TimeBase::Boot) => s.serialize_str("boot"),
            Some(TimeBase::PowerOn) => s.serialize_str("power-on"),
            Some(TimeBase::Checkpoint(name)) => s.serialize_str(name),
        }
    }

    pub(crate) fn deserialize<'de, D: Deserializer<'de>>(
        d: D,
    ) -> Result<Option<TimeBase>, D::Error> {
        Ok(
            Option::<String>::deserialize(d)?.map(|word| match word.as_str() {
                "boot" => TimeBase::Boot,
                "power-on" => TimeBase::PowerOn,
                _ => TimeBase::Checkpoint(word),
            }),
        )
    }
}

pub(crate) fn is_false(b: &bool) -> bool {
    !*b
}

fn instruction() -> StepKind {
    StepKind::Instruction
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct StepArgs {
    #[serde(default = "instruction")]
    pub unit: StepKind,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ReadArgs {
    /// Conditions, from the §7 address grammar and the §8 expression grammar.
    pub reads: Vec<String>,
    /// What `frame` and `t` count from (design §9).
    #[serde(default, skip_serializing_if = "Option::is_none", with = "since_word")]
    pub since: Option<TimeBase>,
    /// Read never-written bytes as their power-on values, with a warning
    /// (design §8), instead of failing with `uninitialized_read`.
    #[serde(default, skip_serializing_if = "is_false")]
    pub allow_uninit: bool,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct BreakArgs {
    pub at: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub condition: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct WatchArgs {
    pub access: WatchAccess,
    pub at: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub condition: Option<String>,
    #[serde(default, skip_serializing_if = "is_false")]
    pub once: bool,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DeleteArgs {
    pub id: u32,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CheckpointArgs {
    pub name: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RestoreArgs {
    pub name: String,
}

/// Which of the three writes (design §4).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum WriteKind {
    /// As the CPU would, with IO side effects.
    Bus,
    /// Backing memory, no side effects.
    Poke,
    /// The ROM file, recorded in the envelope's patch set.
    Patch,
}

/// One write: a target address and the bytes.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct WriteArg {
    pub kind: WriteKind,
    /// The §7 address grammar (`wSpeed`, `$C000`, `03:4123`, `rom:0x14E`).
    pub target: String,
    #[serde(with = "crate::hex::bytes")]
    pub bytes: Vec<u8>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct WriteArgs {
    pub writes: Vec<WriteArg>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PruneArgs {
    /// A duration such as `7d`, `12h` or `30m`.
    pub unused_for: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RmArgs {
    pub name: String,
    #[serde(default, skip_serializing_if = "is_false")]
    pub with_checkpoints: bool,
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct EventsArgs {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub limit: Option<usize>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::stop::TimeBase;

    fn parse(line: &str) -> Request {
        serde_json::from_str(line).expect("parses")
    }

    #[test]
    fn the_design_example_request_round_trips() {
        let line = r#"{"id":"7","session":"demo","as":"claude","rev":"s6","op":"run","args":{"at":"f30","until":"u16[wFrame] >= 60","read":["u16[wFrame]"]}}"#;
        let request = parse(line);
        assert_eq!(request.id, "7");
        assert_eq!(request.session.as_deref(), Some("demo"));
        assert_eq!(request.actor.as_deref(), Some("claude"));
        assert_eq!(request.rev, Some(Rev(6)));
        let Op::Run(args) = &request.op else {
            panic!("{request:?}")
        };
        assert_eq!(args.at.as_deref(), Some("f30"));
        assert_eq!(args.until.as_deref(), Some("u16[wFrame] >= 60"));
        assert_eq!(args.read, vec!["u16[wFrame]".to_string()]);
        assert_eq!(serde_json::to_string(&request).unwrap(), line);
    }

    #[test]
    fn a_unit_op_has_no_args_key() {
        let request = parse(r#"{"id":"1","op":"show"}"#);
        assert_eq!(request.op, Op::Show);
        assert_eq!(
            serde_json::to_string(&request).unwrap(),
            r#"{"id":"1","op":"show"}"#
        );
    }

    #[test]
    fn step_defaults_to_one_instruction() {
        let request = parse(r#"{"id":"1","op":"step","args":{}}"#);
        assert_eq!(
            request.op,
            Op::Step(StepArgs {
                unit: StepKind::Instruction
            })
        );
    }

    #[test]
    fn the_three_writes_carry_bytes_as_hex() {
        let request = parse(
            r#"{"id":"1","op":"write","args":{"writes":[{"kind":"poke","target":"wSpeed","bytes":"02"}]}}"#,
        );
        let Op::Write(args) = &request.op else {
            panic!("{request:?}")
        };
        assert_eq!(args.writes[0].bytes, vec![2]);
        assert_eq!(args.writes[0].target, "wSpeed");
    }

    #[test]
    fn from_and_since_are_typed_on_the_wire_as_plain_words() {
        let request = parse(r#"{"id":"1","op":"run","args":{"from":"power-on","since":"f30"}}"#);
        let Op::Run(args) = &request.op else {
            panic!("{request:?}")
        };
        assert_eq!(args.from, Some(Start::PowerOn));
        assert_eq!(args.since, Some(TimeBase::Checkpoint("f30".into())));
        let line = serde_json::to_string(&request).unwrap();
        assert!(line.contains(r#""from":"power-on""#), "{line}");
        assert!(line.contains(r#""since":"f30""#), "{line}");
        for (text, base) in [("boot", TimeBase::Boot), ("power-on", TimeBase::PowerOn)] {
            let line =
                format!(r#"{{"id":"1","op":"read","args":{{"reads":[],"since":"{text}"}}}}"#);
            let Op::Read(args) = parse(&line).op else {
                panic!()
            };
            assert_eq!(args.since, Some(base));
        }
    }

    #[test]
    fn allow_rom_change_rides_on_the_request_not_its_args() {
        let line = r#"{"id":"1","allow_rom_change":true,"op":"show"}"#;
        let request = parse(line);
        assert!(request.allow_rom_change);
        assert_eq!(serde_json::to_string(&request).unwrap(), line);
        let old = r#"{"id":"1","op":"run","args":{"allow_rom_change":true}}"#;
        assert!(
            serde_json::from_str::<Request>(old).is_err(),
            "no twin in the args"
        );
    }

    #[test]
    fn a_start_that_isnt_one_is_refused() {
        let line = r#"{"id":"1","op":"run","args":{"from":"sideways"}}"#;
        assert!(serde_json::from_str::<Request>(line).is_err());
    }

    #[test]
    fn an_unknown_op_is_refused() {
        assert!(serde_json::from_str::<Request>(r#"{"id":"1","op":"fly"}"#).is_err());
    }

    #[test]
    fn builds_without_id_revision_actor_are_refused() {
        assert!(serde_json::from_str::<Request>(r#"{"op":"show"}"#).is_err());
        assert!(serde_json::from_str::<Request>(r#"{"id":"1","rev":"s","op":"show"}"#).is_err());
    }
}
