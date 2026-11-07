//! A `--until` predicate and the triggers derived from what it reads
//! (design §8, §11).

use gbcli_core::{Core, StoreFilter};
use gbcli_protocol::StopReason;

use super::RunOptions;
use super::record;
use crate::cond::{Condition, EvalCtx, EvalError, EvalWarning, Triggers};
use crate::machine::Machine;

pub(crate) struct Until<'a> {
    pub condition: &'a Condition,
    /// What can make it change. Every instruction already catches every
    /// write, so that cadence arms no stores; the others keep theirs, so a
    /// mid-line or mid-frame change is not missed.
    pub triggers: Triggers,
    text: &'a str,
}

impl<'a> Until<'a> {
    pub fn new(condition: &'a Condition, core: &dyn Core) -> Until<'a> {
        let mut triggers = condition.triggers(core);
        if triggers.every_instruction {
            triggers.stores = StoreFilter::new();
        }
        Until {
            condition,
            triggers,
            text: condition.source(),
        }
    }

    /// The stop record's reason: the predicate text and the cadence it used.
    pub fn reason(&self) -> StopReason {
        StopReason::Predicate {
            text: self.text.to_string(),
            cadence: self.triggers.cadence(),
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
