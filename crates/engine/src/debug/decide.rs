//! Deciding on a candidate: whether a goal is reached, or a breakpoint or
//! watch whose condition holds. A condition that reads a never-written byte
//! is "not yet" (design §8): no stop, no error, and a warning if the run ends
//! with the byte still unwritten.

use std::collections::BTreeMap;

use gbcli_core::{Access, HaltWake, Hit, Usage};
use gbcli_protocol::{Location, StepKind, StopReason, TimeMark, WatchAccess};

use super::drive::{BOOT_OFF, Goal};
use super::record::{base, record};
use super::{DebugError, RunOptions};
use crate::cond::eval::frame_number;
use crate::cond::{
    AccessValues, Condition, EvalCtx, EvalError, EvalWarning, StopTrigger, UninitRead, Verdict,
};
use crate::machine::Machine;
use crate::mem;

/// The machine's breakpoints and watches, conditions compiled.
pub(super) struct Compiled {
    pub breakpoints: Vec<(u32, Location, Option<Condition>)>,
    pub watches: Vec<(u32, WatchAccess, Location, Option<Condition>, bool)>,
}

/// `at` with its bank as the hardware reaches it ([`mem::effective_bank`]):
/// the core reports a hit in the bank of the byte it reached, so a location
/// named through an aliased bank must be armed and matched in that bank.
pub(super) fn reached(m: &Machine, at: &Location) -> Location {
    Location {
        bank: at.bank.map(|b| mem::effective_bank(m.core(), b, at.addr)),
        ..at.clone()
    }
}

pub(super) fn compile(m: &Machine) -> Result<Compiled, DebugError> {
    let config = m.debugger_config();
    let cond = |id: u32, text: &Option<String>| {
        text.as_deref()
            .map(|t| Condition::compile(t, m.symbols()))
            .transpose()
            .map_err(|error| DebugError::Condition { id, error })
    };
    Ok(Compiled {
        breakpoints: config
            .breakpoints
            .iter()
            .map(|b| Ok((b.id, reached(m, &b.at), cond(b.id, &b.condition)?)))
            .collect::<Result<_, DebugError>>()?,
        watches: config
            .watches
            .iter()
            .map(|w| {
                Ok((
                    w.id,
                    w.access,
                    reached(m, &w.at),
                    cond(w.id, &w.condition)?,
                    w.once,
                ))
            })
            .collect::<Result<_, DebugError>>()?,
    })
}

/// What a run reports beside its stop.
#[derive(Default)]
pub(super) struct Notes {
    /// Never-written reads that `allow_uninit` let through.
    pub warnings: Vec<EvalWarning>,
    /// Per breakpoint or watch, the byte its condition last waited on.
    waiting: BTreeMap<u32, UninitRead>,
}

impl Notes {
    /// Records what a breakpoint's or watch's condition said; whether it holds.
    fn holds(&mut self, id: u32, verdict: Verdict) -> bool {
        match verdict {
            Verdict::NotYet(read) => {
                self.waiting.insert(id, read);
                false
            }
            other => {
                self.waiting.remove(&id);
                other == Verdict::Holds
            }
        }
    }

    /// The warnings for the outcome: the run's own, then each condition
    /// still waiting on a byte that is still unwritten, then STOP if the CPU
    /// is in it. `until` is the predicate's own wait, if any.
    pub fn finish(self, m: &Machine, until: Option<UninitRead>) -> Vec<EvalWarning> {
        let mut warnings = self.warnings;
        let waits = until.map(|read| (StopTrigger::Until, read)).into_iter();
        let points = self
            .waiting
            .into_iter()
            .filter(|(_, read)| read.still_unwritten(m.core()))
            .map(|(id, read)| (StopTrigger::Condition { id }, read));
        warnings.extend(
            waits
                .chain(points)
                .map(|(trigger, read)| EvalWarning::NotYet { trigger, read }),
        );
        if m.core().in_stop() {
            warnings.push(EvalWarning::InStop);
        }
        warnings
    }
}

fn bank_matches(at: &Location, addr: u16, bank: u16) -> bool {
    at.addr == addr && at.bank.is_none_or(|b| b == bank)
}

/// The breakpoint that would stop at the current boundary, if any. A
/// predicate stop yields to one, since both describe the same instruction.
pub(super) fn exec_breakpoint(
    m: &mut Machine,
    compiled: &Compiled,
    opts: &RunOptions<'_>,
    used: Usage,
    notes: &mut Notes,
) -> Result<Option<StopReason>, DebugError> {
    if compiled.breakpoints.is_empty() {
        return Ok(None);
    }
    let regs = m.core().registers();
    let hit = Hit {
        access: Access::Exec,
        addr: regs.pc,
        bank: mem::bank_at(m.core(), regs.pc),
        old: mem::peek(m.core(), regs.pc).ok(),
        new: 0,
        pc: regs.pc,
        vetoed: false,
    };
    decide(m, compiled, &Goal::Run, hit, opts, used, notes)
}

/// The exec hit at the current PC, when the execution filter matches it. In
/// HALT or STOP, PC already points past it, but that instruction runs only
/// after the wake (and any interrupt it dispatches), so there is none yet;
/// except as a CGB speed switch's pause ends, when the next step runs it.
pub(super) fn exec_hit(m: &Machine, exec: &gbcli_core::AddrFilter) -> Option<Hit> {
    let regs = m.core().registers();
    let pc = regs.pc;
    let ending = m.core().halt_wake() == Ok(Some(HaltWake::SpeedSwitchEnding));
    if !exec.may_match(pc) || (regs.halted && !ending) || m.core().in_stop() {
        return None;
    }
    let bank = mem::bank_at(m.core(), pc);
    let old = mem::peek(m.core(), pc).ok();
    exec.matches(pc, bank).then_some(Hit {
        access: Access::Exec,
        addr: pc,
        bank,
        old,
        new: old.unwrap_or(0xFF),
        pc,
        vetoed: false,
    })
}

pub(super) fn remove_once(m: &mut Machine, compiled: &Compiled, id: u32) {
    if compiled.watches.iter().any(|w| w.0 == id && w.4) {
        m.delete(id);
    }
}

/// Whether a candidate is a real stop: a goal reached, or a breakpoint or
/// watch whose condition holds. A condition that fails is an error whose
/// stop is where the breakpoint or watch hit.
pub(super) fn decide(
    m: &mut Machine,
    compiled: &Compiled,
    goal: &Goal,
    hit: Hit,
    opts: &RunOptions<'_>,
    used: Usage,
    notes: &mut Notes,
) -> Result<Option<StopReason>, DebugError> {
    if hit.access == Access::Write && hit.addr == BOOT_OFF && m.boot.is_none() {
        m.boot = Some(TimeMark {
            ticks: m.core().now().0,
            frame: frame_number(m.core()),
        });
    }
    let failed = |m: &Machine, id: u32, reason: StopReason, error: EvalError| DebugError::Eval {
        id,
        error,
        stop: Box::new(record(m, reason, used, &opts.since)),
    };
    let time_base = base(m, &opts.since).1;
    let mut holds =
        |m: &Machine, id: u32, reason: &StopReason, cond: &Option<Condition>, access| {
            let Some(cond) = cond else { return Ok(true) };
            let ctx = EvalCtx {
                core: m.core(),
                symbols: m.symbols(),
                access,
                base: time_base,
                allow_uninit: opts.allow_uninit,
            };
            match cond.verdict(&ctx, &mut notes.warnings) {
                Ok(verdict) => Ok(notes.holds(id, verdict)),
                Err(error) => Err(failed(m, id, reason.clone(), error)),
            }
        };
    match hit.access {
        Access::Exec => {
            let reached = match goal {
                Goal::To(at) => bank_matches(at, hit.addr, hit.bank).then_some(StepKind::ToAddress),
                Goal::Over { ret, sp, .. } => {
                    (hit.addr == *ret && m.core().registers().sp >= *sp).then_some(StepKind::Over)
                }
                Goal::Instruction => Some(StepKind::Instruction),
                _ => None,
            };
            if let Some(unit) = reached {
                return Ok(Some(StopReason::StepDone { unit }));
            }
            for (id, at, cond) in &compiled.breakpoints {
                let reason = StopReason::Breakpoint { id: *id };
                if bank_matches(at, hit.addr, hit.bank) && holds(m, *id, &reason, cond, None)? {
                    return Ok(Some(reason));
                }
            }
        }
        Access::Read | Access::Write => {
            let wanted = |access: WatchAccess| match access {
                WatchAccess::Read => hit.access == Access::Read,
                WatchAccess::Write => hit.access == Access::Write,
                WatchAccess::Access => true,
            };
            let values = Some(AccessValues {
                old: hit.old,
                new: hit.new,
            });
            for (id, access, at, cond, _) in &compiled.watches {
                let reason = StopReason::Watchpoint {
                    id: *id,
                    access: hit.access,
                    addr: hit.addr,
                    old: hit.old,
                    new: hit.new,
                    writer_pc: hit.pc,
                };
                if wanted(*access)
                    && bank_matches(at, hit.addr, hit.bank)
                    && holds(m, *id, &reason, cond, values)?
                {
                    return Ok(Some(reason));
                }
            }
        }
    }
    Ok(None)
}
