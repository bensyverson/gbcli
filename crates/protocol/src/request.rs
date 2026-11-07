//! One request per JSON line (design §5). The server and the CLI build the
//! same structs; `op` and `args` are adjacent tags at the top level.

use serde::{Deserialize, Serialize};

use crate::checkpoint::WatchAccess;
use crate::parse::WireRequest;
use crate::rev::Rev;
use crate::stop::{StepKind, TimeBase};

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
        }
    }

    /// Whether applying it changes the head, and so bumps the revision.
    pub fn mutates_head(&self) -> bool {
        matches!(
            self,
            Op::Run(_) | Op::Step(_) | Op::Restore(_) | Op::Undo | Op::Write(_)
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
    /// Conditions evaluated at the stop (design §11 step 6).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub read: Vec<String>,
    /// Save the stop state under this name.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub checkpoint: Option<String>,
    #[serde(default, skip_serializing_if = "is_false")]
    pub allow_uninit: bool,
    #[serde(default, skip_serializing_if = "is_false")]
    pub allow_rom_change: bool,
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
mod since_word {
    use serde::{Deserialize, Deserializer, Serializer};

    use crate::stop::TimeBase;

    pub fn serialize<S: Serializer>(base: &Option<TimeBase>, s: S) -> Result<S::Ok, S::Error> {
        match base {
            None => s.serialize_none(),
            Some(TimeBase::Boot) => s.serialize_str("boot"),
            Some(TimeBase::PowerOn) => s.serialize_str("power-on"),
            Some(TimeBase::Checkpoint(name)) => s.serialize_str(name),
        }
    }

    pub fn deserialize<'de, D: Deserializer<'de>>(d: D) -> Result<Option<TimeBase>, D::Error> {
        Ok(
            Option::<String>::deserialize(d)?.map(|word| match word.as_str() {
                "boot" => TimeBase::Boot,
                "power-on" => TimeBase::PowerOn,
                _ => TimeBase::Checkpoint(word),
            }),
        )
    }
}

fn is_false(b: &bool) -> bool {
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
    #[serde(default, skip_serializing_if = "is_false")]
    pub allow_rom_change: bool,
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
