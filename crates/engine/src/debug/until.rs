//! A `--until` predicate: the triggers derived from what it reads, when one
//! fired, and evaluating it as a stop trigger (design §8, §11).

use gbcli_core::{Access, Core, Stop, StopReason as CoreReason, StoreFilter, Usage};
use gbcli_protocol::StopReason;

use super::record::{base, record};
use super::{DebugError, RunOptions};
use crate::cond::{Condition, EvalCtx, EvalError, EvalWarning, Triggers, UninitRead, Verdict};
use crate::machine::Machine;

pub(crate) struct Until<'a> {
    pub condition: &'a Condition,
    /// What can make it change. The every-instruction trigger already sees
    /// every write, so it arms no stores; every other trigger keeps them, so a
    /// mid-line or mid-frame change is not missed.
    pub triggers: Triggers,
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
        }
    }

    /// Whether the core must stop after every instruction for this predicate.
    pub fn fine(&self) -> bool {
        self.triggers.scanline || self.triggers.every_instruction
    }

    /// The stop record's reason: the predicate text and the cadence it used.
    pub fn reason(&self) -> StopReason {
        StopReason::Predicate {
            text: self.condition.source().to_string(),
            cadence: self.triggers.cadence(),
        }
    }

    /// Whether any trigger fired in the core call that ended with `stop`:
    /// the set, not the costliest alone. A write to an armed byte, or any
    /// write hit at all (re-evaluating costs less than telling them apart).
    pub fn fired(&self, stop: &Stop, ly_before: u8, core: &dyn Core) -> bool {
        let t = &self.triggers;
        let wrote = matches!(&stop.reason, CoreReason::Candidate(hits)
            if hits.iter().any(|h| h.access == Access::Write));
        wrote
            || (t.every_instruction && stop.used.instructions > 0)
            || (t.vblank && stop.used.frames > 0)
            || (t.scanline && core.ppu().ly != ly_before)
    }

    /// Whether the predicate holds now. A never-written byte is "not yet"; any
    /// other failure ends the run with the machine where it is.
    pub fn holds(
        &self,
        m: &Machine,
        opts: &RunOptions<'_>,
        used: Usage,
        warnings: &mut Vec<EvalWarning>,
    ) -> Result<bool, DebugError> {
        match self.verdict(m, opts, warnings) {
            Ok(verdict) => Ok(verdict == Verdict::Holds),
            Err(error) => Err(DebugError::Until {
                error,
                stop: Box::new(record(m, self.failed(), used, &opts.since)),
            }),
        }
    }

    /// The byte the predicate waits on now, if it reads one never written.
    pub fn waiting(&self, m: &Machine, opts: &RunOptions<'_>) -> Option<UninitRead> {
        match self.verdict(m, opts, &mut Vec::new()) {
            Ok(Verdict::NotYet(read)) => Some(read),
            _ => None,
        }
    }

    /// The reason a record carries when the predicate couldn't be evaluated.
    fn failed(&self) -> StopReason {
        StopReason::PredicateError {
            text: self.condition.source().to_string(),
            cadence: self.triggers.cadence(),
        }
    }

    /// Evaluates with the time base the stop record uses.
    fn verdict(
        &self,
        m: &Machine,
        opts: &RunOptions<'_>,
        warnings: &mut Vec<EvalWarning>,
    ) -> Result<Verdict, EvalError> {
        let ctx = EvalCtx {
            core: m.core(),
            symbols: m.symbols(),
            access: None,
            base: base(m, &opts.since).1,
            allow_uninit: opts.allow_uninit,
        };
        self.condition.verdict(&ctx, warnings)
    }
}
