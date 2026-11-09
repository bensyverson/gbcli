//! `run --expect`: conditions that must hold at the stop, evaluated beside
//! the reads with the same grammar and never-written rules. One false fails
//! the run with `predicate_false` (exit 1); one that can't be evaluated fails
//! it with its own error, since how it was asked comes before its answer.
//! Either way the run is saved and its whole result rides on the error.

use gbcli_core::Gap;
use gbcli_engine::{Machine, Since};
use gbcli_protocol::{ErrorBody, ErrorCode, ExpectValue, ResultBody};

use super::super::Failure;
use super::super::reads::{CompiledRead, compile_conditions, eval_reads_each};

/// Compiles the conditions before the machine moves.
pub(super) fn compile(machine: &Machine, exprs: &[String]) -> Result<Vec<CompiledRead>, Failure> {
    compile_conditions(machine, exprs)
}

/// The conditions at the stop, each with its verdict.
pub(super) struct Expected(Vec<ExpectValue>);

/// Evaluates the conditions where `machine` stopped; a gap a condition
/// can't be answered for goes to `gaps`, as a read's does.
pub(super) fn evaluate(
    machine: &Machine,
    expect: &[CompiledRead],
    since: &Since,
    allow_uninit: bool,
    warnings: &mut Vec<String>,
    gaps: &mut Vec<Gap>,
) -> Expected {
    let values = eval_reads_each(machine, expect, since, allow_uninit, warnings, gaps);
    Expected(
        values
            .into_iter()
            .map(|v| ExpectValue {
                ok: v.value.is_some_and(|n| n != 0),
                expr: v.expr,
                value: v.value,
                error: v.error,
            })
            .collect(),
    )
}

impl Expected {
    /// Whether every condition held (true when there were none).
    pub(super) fn held(&self) -> bool {
        self.0.iter().all(|e| e.ok)
    }

    /// The rows for the result, when the run asked for any.
    pub(super) fn rows(&self) -> Option<Vec<ExpectValue>> {
        (!self.0.is_empty()).then(|| self.0.clone())
    }

    /// `result` when every condition held; else the failure carrying it.
    pub(super) fn verdict(&self, result: ResultBody) -> Result<ResultBody, Failure> {
        let failed: Vec<&ExpectValue> = self.0.iter().filter(|e| !e.ok).collect();
        let Some(first) = failed.first() else {
            return Ok(result);
        };
        let of = match failed.len() {
            1 => String::new(),
            n => format!(" ({n} of {} failed)", self.0.len()),
        };
        let unanswered = failed.iter().find_map(|e| Some((e, e.error.as_ref()?)));
        let mut body = match unanswered {
            Some((e, error)) => ErrorBody::new(
                error.code,
                format!(
                    "--expect `{}` couldn't be evaluated: {}{of}",
                    e.expr, error.message
                ),
                error.remedy.clone(),
            ),
            None => ErrorBody::new(
                ErrorCode::PredicateFalse,
                format!(
                    "--expect `{}` is false: it is {}{of}",
                    first.expr,
                    first.value.unwrap_or_default()
                ),
                "the expect rows give each value at the stop; the run is saved, so read \
                 more there, or fix the game or the expectation",
            ),
        };
        body.result = Some(Box::new(result));
        Err(Failure::new(body))
    }
}
