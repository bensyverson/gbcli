//! Reading one request line (design §5), refusing what it doesn't know: a
//! misspelled field must never be silently ignored, since `"revv"` would
//! skip the stale-rev guard and `"frame"` would run the default budget.

use std::fmt;

use serde::Deserialize;
use serde_json::{Map, Value};

use crate::request::{Op, Request};
use crate::rev::Rev;
use crate::strict::Strict;

/// A request as read, before its op and args become an [`Op`]. Its own
/// fields refuse strangers here, which `#[serde(flatten)]` can't do: a
/// flattened enum only ever sees the keys it asks for, so an unknown key
/// beside it would vanish. The args structs refuse theirs themselves.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct WireRequest {
    id: String,
    #[serde(default)]
    session: Option<String>,
    #[serde(default, rename = "as")]
    actor: Option<String>,
    #[serde(default)]
    rev: Option<Rev>,
    #[serde(default)]
    allow_rom_change: bool,
    op: Value,
    /// `null` reads as absent.
    #[serde(default)]
    args: Option<Strict>,
}

impl TryFrom<WireRequest> for Request {
    type Error = serde_json::Error;

    fn try_from(w: WireRequest) -> Result<Request, serde_json::Error> {
        let build = |args: Option<Value>| {
            let mut tagged = Map::new();
            tagged.insert("op".into(), w.op.clone());
            if let Some(args) = args {
                tagged.insert("args".into(), args);
            }
            Op::deserialize(Value::Object(tagged))
        };
        // Absent, null and `{}` args mean the same: every field defaulted, or
        // no args at all for a unit op. The first error is the one to report:
        // it names a required field (`reads`) rather than `args`.
        let args = w.args.map(|s| s.0);
        let empty = match &args {
            None => true,
            Some(Value::Object(fields)) => fields.is_empty(),
            Some(_) => false,
        };
        let op =
            build(Some(args.unwrap_or_else(|| Value::Object(Map::new())))).or_else(|first| {
                if empty {
                    build(None).map_err(|_| first)
                } else {
                    Err(first)
                }
            })?;
        Ok(Request {
            id: w.id,
            session: w.session,
            actor: w.actor,
            rev: w.rev,
            allow_rom_change: w.allow_rom_change,
            op,
        })
    }
}

/// Why a line isn't a request, with the corrected form when there is one.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RequestError {
    /// The line's `id`, when it had a readable one, to echo back.
    pub id: Option<String>,
    pub message: String,
    pub remedy: String,
}

impl fmt::Display for RequestError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.message)
    }
}

impl std::error::Error for RequestError {}

const SCHEMA_REMEDY: &str = "one JSON object per line; see `gbcli help schema`";

/// Parses one JSON line into a request. An unknown field is named, with the
/// nearest known one as the remedy.
pub fn parse_request(line: &str) -> Result<Request, RequestError> {
    serde_json::from_str::<Request>(line).map_err(|e| {
        let id = serde_json::from_str::<Value>(line)
            .ok()
            .and_then(|value| value.get("id")?.as_str().map(str::to_string));
        let message = e.to_string();
        let remedy = match unknown_field(&message) {
            Some((field, known)) => match nearest(&field, &known) {
                Some(near) => format!("did you mean `{near}`? known here: {}", known.join(", ")),
                None => format!("`{field}` isn't a field here; known: {}", known.join(", ")),
            },
            None => SCHEMA_REMEDY.into(),
        };
        RequestError {
            id,
            message: format!("not a request: {message}"),
            remedy,
        }
    })
}

/// The field and the known ones from serde's `unknown field` message:
/// "unknown field `revv`, expected one of `id`, `rev`, …" (or "expected `a`
/// or `b`", or "there are no fields").
fn unknown_field(message: &str) -> Option<(String, Vec<String>)> {
    let rest = message.strip_prefix("unknown field `")?;
    let (field, rest) = rest.split_once('`')?;
    let known = rest
        .split('`')
        .skip(1)
        .step_by(2)
        .map(str::to_string)
        .collect();
    Some((field.to_string(), known))
}

/// The known name closest to `field` by edit distance, if any is close.
fn nearest(field: &str, known: &[String]) -> Option<String> {
    let limit = (field.chars().count() / 2).max(2);
    known
        .iter()
        .map(|name| (edit_distance(field, name), name))
        .filter(|(d, _)| *d <= limit)
        .min_by_key(|(d, _)| *d)
        .map(|(_, name)| name.clone())
}

fn edit_distance(a: &str, b: &str) -> usize {
    let b: Vec<char> = b.chars().collect();
    let mut row: Vec<usize> = (0..=b.len()).collect();
    for (i, ca) in a.chars().enumerate() {
        let mut prev = row[0];
        row[0] = i + 1;
        for (j, cb) in b.iter().enumerate() {
            let cost = usize::from(ca != *cb);
            let next = (row[j] + 1).min(row[j + 1] + 1).min(prev + cost);
            prev = row[j + 1];
            row[j + 1] = next;
        }
    }
    row[b.len()]
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::request::{Op, RunArgs};

    #[test]
    fn an_unknown_top_level_field_is_refused_and_named() {
        let err = parse_request(r#"{"id":"7","revv":"s3","op":"show"}"#).unwrap_err();
        assert!(err.message.contains("revv"), "{}", err.message);
        assert!(err.remedy.contains("`rev`"), "{}", err.remedy);
        assert_eq!(err.id.as_deref(), Some("7"));
    }

    #[test]
    fn an_unknown_args_field_is_refused_and_named() {
        let err = parse_request(r#"{"id":"1","op":"run","args":{"frame":10}}"#).unwrap_err();
        assert!(err.message.contains("frame"), "{}", err.message);
        assert!(err.remedy.contains("`frames`"), "{}", err.remedy);
    }

    #[test]
    fn an_unknown_field_in_a_nested_write_is_refused() {
        let line = r#"{"id":"1","op":"write","args":{"writes":[{"kind":"poke","target":"$C000","bytes":"01","bank":3}]}}"#;
        let err = parse_request(line).unwrap_err();
        assert!(err.message.contains("bank"), "{}", err.message);
    }

    #[test]
    fn a_known_request_still_parses() {
        let request = parse_request(r#"{"id":"1","rev":"s2","op":"run","args":{"frames":10}}"#)
            .expect("parses");
        assert_eq!(
            request.op,
            Op::Run(RunArgs {
                frames: Some(10),
                ..RunArgs::default()
            })
        );
        assert!(parse_request(r#"{"id":"1","op":"show"}"#).is_ok());
    }

    #[test]
    fn a_line_without_an_id_is_refused() {
        assert!(parse_request(r#"{"op":"show"}"#).is_err());
    }

    fn op_of(line: &str) -> Op {
        match parse_request(line) {
            Ok(request) => request.op,
            Err(e) => panic!("{line}: {e} ({})", e.remedy),
        }
    }

    #[test]
    fn args_may_be_left_out_or_null_when_every_field_has_a_default() {
        use crate::request::{EventsArgs, StepArgs};
        use crate::stop::StepKind;
        for line in [
            r#"{"id":"1","op":"run"}"#,
            r#"{"id":"1","op":"run","args":null}"#,
        ] {
            assert_eq!(op_of(line), Op::Run(RunArgs::default()), "{line}");
        }
        let step = Op::Step(StepArgs {
            unit: StepKind::Instruction,
        });
        assert_eq!(op_of(r#"{"id":"1","op":"step"}"#), step);
        assert_eq!(
            op_of(r#"{"id":"1","op":"events"}"#),
            Op::Events(EventsArgs::default())
        );
    }

    #[test]
    fn a_unit_op_takes_empty_or_null_args() {
        for line in [
            r#"{"id":"1","op":"show","args":{}}"#,
            r#"{"id":"1","op":"show","args":null}"#,
            r#"{"id":"1","op":"undo","args":{}}"#,
        ] {
            assert!(parse_request(line).is_ok(), "{line}");
        }
        let err = parse_request(r#"{"id":"1","op":"show","args":{"x":1}}"#).unwrap_err();
        assert!(
            err.message.contains("x") || err.message.contains("args"),
            "{err}"
        );
    }

    #[test]
    fn an_op_with_a_required_arg_still_names_it_when_args_are_left_out() {
        let err = parse_request(r#"{"id":"1","op":"read"}"#).unwrap_err();
        assert!(err.message.contains("reads"), "{}", err.message);
    }

    #[test]
    fn a_duplicate_key_inside_args_is_refused_as_at_the_top_level() {
        assert!(parse_request(r#"{"id":"1","id":"2","op":"show"}"#).is_err());
        let err =
            parse_request(r#"{"id":"1","op":"run","args":{"frames":1,"frames":2}}"#).unwrap_err();
        assert!(err.message.contains("frames"), "{}", err.message);
        let nested = r#"{"id":"1","op":"write","args":{"writes":[{"kind":"poke","kind":"bus","target":"$C000","bytes":"01"}]}}"#;
        assert!(parse_request(nested).is_err());
    }
}
