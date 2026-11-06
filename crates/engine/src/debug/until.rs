//! A `--until` predicate and the cadence derived from what it reads
//! (design §8, §11).

use gbcli_core::BankSel;
use gbcli_protocol::{Cadence, StopReason};

use super::RunOptions;
use super::record;
use crate::cond::{Condition, EvalCtx, EvalError, EvalWarning};
use crate::machine::Machine;

pub(crate) struct Until<'a> {
    pub condition: &'a Condition,
    pub cadence: Cadence,
    text: &'a str,
    /// The static addresses to arm with the write hook. Every instruction
    /// already catches every write, so a finer cadence leaves this empty; a
    /// coarser one keeps it, so a mid-line or mid-frame change is not missed.
    pub writes: Vec<(u16, BankSel)>,
}

impl<'a> Until<'a> {
    pub fn new(condition: &'a Condition) -> Until<'a> {
        let cadence = condition.cadence();
        Until {
            condition,
            cadence,
            text: condition.source(),
            writes: if cadence == Cadence::EveryInstruction {
                Vec::new()
            } else {
                condition.write_candidates()
            },
        }
    }

    /// The stop record's reason: the predicate text and the cadence it used.
    pub fn reason(&self) -> StopReason {
        StopReason::Predicate {
            text: self.text.to_string(),
            cadence: self.cadence,
        }
    }
}

/// Evaluates a predicate with the time base the stop record uses.
pub(crate) fn evaluate(
    m: &Machine,
    opts: &RunOptions<'_>,
    condition: &Condition,
    warnings: &mut Vec<EvalWarning>,
) -> Result<bool, EvalError> {
    let base = record::base(m, &opts.since).1;
    let ctx = EvalCtx {
        core: m.core(),
        symbols: m.symbols(),
        access: None,
        base,
        allow_uninit: opts.allow_uninit,
    };
    condition.holds(&ctx, warnings)
}
