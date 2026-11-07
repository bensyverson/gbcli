//! The run loop: hands the core filters, decides on its candidates, and
//! resumes until a real stop. Every pass is bounded by the budget.

use std::sync::atomic::Ordering;

use gbcli_core::{
    Access, AddrFilter, BankSel, Core, EventSink, Region, RunRequest, StopReason as CoreReason,
    StoreFilter, Usage,
};
use gbcli_protocol::{Location, StepKind, StopReason, WatchAccess};

use super::decide::{Notes, compile, decide, exec_breakpoint, exec_hit, reached, remove_once};
use super::keys::Schedule;
use super::record::record;
use super::until::Until;
use super::{DebugError, Outcome, RunOptions};
use crate::cond::Condition;
use crate::machine::Machine;
use crate::mem;

/// Instructions per core call between the engine's own checks (HALT with
/// interrupts off, user break). Small enough to notice either promptly,
/// large enough that the checks cost nothing measurable.
const CHUNK: u64 = 4096;

/// The register that unmaps the boot ROM: its first write is the handoff.
pub(super) const BOOT_OFF: u16 = 0xFF50;

/// What ends a run besides breakpoints, watches and the budget.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Goal {
    Run,
    Instruction,
    /// Until PC is `ret` with SP back at `sp` or above (a recursive call
    /// reaching `ret` deeper doesn't count).
    Over {
        ret: u16,
        bank: u16,
        sp: u16,
    },
    /// Until a RET-family instruction leaves SP above `sp`.
    Out {
        sp: u16,
    },
    Frame,
    Scanline {
        ly: u8,
    },
    To(Location),
}

impl Goal {
    /// Goals checked after every instruction.
    fn fine(&self) -> bool {
        matches!(
            self,
            Goal::Instruction | Goal::Out { .. } | Goal::Scanline { .. }
        )
    }
}

/// Step-over's goal: the return address of the CALL or RST at PC, or one
/// instruction if PC isn't at one.
pub fn over(m: &Machine) -> Goal {
    let core = m.core();
    let regs = core.registers();
    let len = match mem::peek(core, regs.pc) {
        Ok(0xCD | 0xC4 | 0xCC | 0xD4 | 0xDC) => 3,
        Ok(0xC7 | 0xCF | 0xD7 | 0xDF | 0xE7 | 0xEF | 0xF7 | 0xFF) => 1,
        _ => return Goal::Instruction,
    };
    Goal::Over {
        ret: regs.pc.wrapping_add(len),
        bank: mem::bank_at(core, regs.pc),
        sp: regs.sp,
    }
}

fn is_return(opcode: u8) -> bool {
    matches!(opcode, 0xC9 | 0xD9 | 0xC0 | 0xC8 | 0xD0 | 0xD8)
}

fn bank_sel(at: &Location) -> BankSel {
    at.bank.map_or(BankSel::Any, BankSel::Bank)
}

/// HALT that nothing can end: no interrupt enabled in IE.
fn halted_forever(core: &dyn Core) -> bool {
    core.registers().halted
        && core
            .region(Region::Ie)
            .is_ok_and(|ie| ie.bytes.first().is_some_and(|&ie| ie & 0x1F == 0))
}

pub fn drive(m: &mut Machine, opts: &RunOptions<'_>, goal: Goal) -> Result<Outcome, DebugError> {
    run(m, opts, goal, None)
}

/// Runs until `condition` holds, evaluating it on the cadence it needs.
pub fn drive_until(
    m: &mut Machine,
    opts: &RunOptions<'_>,
    condition: &Condition,
) -> Result<Outcome, DebugError> {
    run(m, opts, Goal::Run, Some(condition))
}

fn run(
    m: &mut Machine,
    opts: &RunOptions<'_>,
    goal: Goal,
    condition: Option<&Condition>,
) -> Result<Outcome, DebugError> {
    let compiled = compile(m)?;
    let goal = match goal {
        Goal::To(at) => Goal::To(reached(m, &at)),
        other => other,
    };
    let (mut exec, mut read, mut write) = (AddrFilter::new(), AddrFilter::new(), AddrFilter::new());
    for (_, at, _) in &compiled.breakpoints {
        exec.insert(at.addr, bank_sel(at));
    }
    for (_, access, at, _, _) in &compiled.watches {
        if matches!(access, WatchAccess::Read | WatchAccess::Access) {
            read.insert(at.addr, bank_sel(at));
        }
        if matches!(access, WatchAccess::Write | WatchAccess::Access) {
            write.insert(at.addr, bank_sel(at));
        }
    }
    match &goal {
        Goal::To(at) => exec.insert(at.addr, bank_sel(at)),
        Goal::Over { ret, bank, .. } => exec.insert(*ret, BankSel::Bank(*bank)),
        _ => {}
    }
    if m.boot.is_none() {
        write.insert(BOOT_OFF, BankSel::Any);
    }

    // Input before the run (design §11 step 3): the events at frame 0 first.
    let mut keys = Schedule::new(opts.keys);
    keys.apply_due(m, 0);

    let until = condition.map(|c| Until::new(c, m.core()));
    let no_stores = StoreFilter::new();
    let stores = until.as_ref().map_or(&no_stores, |u| &u.triggers.stores);
    let fine = goal.fine() || until.as_ref().is_some_and(Until::fine);

    let mut used = Usage::default();
    let mut notes = Notes::default();
    let finish = |m: &Machine, reason, used: Usage, notes: Notes| {
        let waiting = until.as_ref().and_then(|u| u.waiting(m, opts));
        Ok(Outcome {
            stop: record(m, reason, used, &opts.since),
            warnings: notes.finish(m, waiting),
        })
    };

    // Before the first instruction: a predicate that already holds must not
    // spend the budget. This also makes resuming idempotent.
    if let Some(until) = &until
        && until.holds(m, opts, used, &mut notes.warnings)?
    {
        return finish(m, until.reason(), used, notes);
    }

    // When the last core call ended on an exec candidate the engine passed
    // over, the time of that boundary: its breakpoints have been decided.
    let mut exec_decided: Option<u64> = None;
    loop {
        if let Some(kind) = opts.budget.exhausted(&used) {
            return finish(m, StopReason::BudgetExhausted { kind }, used, notes);
        }
        if opts
            .interrupt
            .is_some_and(|flag| flag.load(Ordering::Relaxed))
        {
            return finish(m, StopReason::UserBreak, used, notes);
        }
        if halted_forever(m.core()) {
            return finish(m, StopReason::HaltForever, used, notes);
        }
        // The core skips an execution filter at the first instruction of each
        // call, so resuming from a breakpoint makes progress. So the engine
        // checks that PC itself: after a chunk that ended there, or after a
        // passed-over write (the boot ROM falls through to $0100 right after
        // the FF50 write). After a passed-over exec candidate it has already
        // decided, and evaluating its conditions again would repeat them.
        let decided_here = exec_decided == Some(m.core().now().0);
        if used.instructions > 0
            && !decided_here
            && let Some(hit) = exec_hit(m, &exec)
            && let Some(reason) = decide(m, &compiled, &goal, hit, opts, used, &mut notes)?
        {
            return finish(m, reason, used, notes);
        }
        let before = m.core().registers();
        let ly_before = m.core().ppu().ly;
        let opcode = mem::peek(m.core(), before.pc).unwrap_or(0);
        let mut req = RunRequest::new(
            opts.budget
                .remaining_after(&used)
                .capped_instructions(if fine { 1 } else { CHUNK }),
        );
        req.exec = &exec;
        req.read = &read;
        req.write = &write;
        req.stores = stores;
        req.stop_at_frame = goal == Goal::Frame
            || until.as_ref().is_some_and(|u| u.triggers.vblank)
            || keys.pending()
            || opts.on_frame.is_some();
        let mut tap = opts.on_event.map(|t| t.0.borrow_mut());
        req.sink = tap.as_deref_mut().map(|s| s as &mut dyn EventSink);
        let stop = m.core_mut().run(&mut req);
        drop(tap);
        let stop = stop?;
        used += stop.used;
        keys.apply_due(m, used.frames);
        if let Some(sink) = opts.on_frame
            && stop.used.frames > 0
        {
            (sink.0)(m, used.frames);
        }
        let fired = until
            .as_ref()
            .is_some_and(|u| u.fired(&stop, ly_before, m.core()));
        exec_decided = None;
        let decided = match stop.reason {
            CoreReason::Locked { opcode } => Some(StopReason::IllegalOpcode { opcode }),
            CoreReason::Stopped => Some(StopReason::Stopped),
            CoreReason::Frame if goal == Goal::Frame => Some(StopReason::StepDone {
                unit: StepKind::Frame,
            }),
            CoreReason::Candidate(hits) => {
                if hits.iter().any(|h| h.access == Access::Exec) {
                    exec_decided = Some(m.core().now().0);
                }
                // In access order: the first hit that is a real stop wins.
                let mut found = None;
                for hit in hits.iter() {
                    found = decide(m, &compiled, &goal, *hit, opts, used, &mut notes)?;
                    if found.is_some() {
                        break;
                    }
                }
                found
            }
            CoreReason::Frame | CoreReason::Budget(_) | CoreReason::Step(_) => None,
        };
        if let Some(reason) = decided {
            if let StopReason::Watchpoint { id, .. } = reason {
                remove_once(m, &compiled, id);
            }
            return finish(m, reason, used, notes);
        }
        if let Some(until) = &until
            && fired
            && until.holds(m, opts, used, &mut notes.warnings)?
        {
            // A breakpoint at this boundary wins; a predicate that lands on
            // one reports the breakpoint instead. One already decided here
            // didn't hold.
            let breakpoint = if exec_decided.is_some() {
                None
            } else {
                exec_breakpoint(m, &compiled, opts, used, &mut notes)?
            };
            let reason = breakpoint.unwrap_or_else(|| until.reason());
            return finish(m, reason, used, notes);
        }
        let after = m.core().registers();
        let unit = match goal {
            Goal::Instruction if used.instructions >= 1 => Some(StepKind::Instruction),
            Goal::Out { sp } if is_return(opcode) && after.sp > sp && after.pc != before.pc => {
                Some(StepKind::Out)
            }
            Goal::Scanline { ly } if m.core().ppu().ly != ly => Some(StepKind::Scanline),
            _ => None,
        };
        if let Some(unit) = unit {
            return finish(m, StopReason::StepDone { unit }, used, notes);
        }
    }
}
