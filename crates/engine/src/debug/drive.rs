//! The run loop: hands the core filters, decides on its candidates, and
//! resumes until a real stop. Every pass is bounded by the budget.

use std::sync::atomic::Ordering;

use gbcli_core::{
    Access, AddrFilter, BankSel, Core, Hit, Region, RunRequest, StopReason as CoreReason, Usage,
};
use gbcli_protocol::{Cadence, Location, StepKind, StopReason, TimeMark, WatchAccess};

use super::record::record;
use super::until::{Until, evaluate};
use super::{DebugError, Outcome, RunOptions};
use crate::cond::eval::frame_number;
use crate::cond::{AccessValues, Condition, EvalCtx, EvalWarning};
use crate::machine::Machine;
use crate::mem;

/// Instructions per core call between the engine's own checks (HALT with
/// interrupts off, user break). Small enough to notice either promptly,
/// large enough that the checks cost nothing measurable.
const CHUNK: u64 = 4096;

/// The register that unmaps the boot ROM: its first write is the handoff.
const BOOT_OFF: u16 = 0xFF50;

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

fn matches(at: &Location, addr: u16, bank: u16) -> bool {
    at.addr == addr && at.bank.is_none_or(|b| b == bank)
}

/// HALT that nothing can end: no interrupt enabled in IE.
fn halted_forever(core: &dyn Core) -> bool {
    core.registers().halted
        && core
            .region(Region::Ie)
            .is_ok_and(|ie| ie.bytes.first().is_some_and(|&ie| ie & 0x1F == 0))
}

struct Compiled {
    breakpoints: Vec<(u32, Location, Option<Condition>)>,
    watches: Vec<(u32, WatchAccess, Location, Option<Condition>, bool)>,
}

fn compile(m: &Machine) -> Result<Compiled, DebugError> {
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
            .map(|b| Ok((b.id, b.at.clone(), cond(b.id, &b.condition)?)))
            .collect::<Result<_, DebugError>>()?,
        watches: config
            .watches
            .iter()
            .map(|w| {
                Ok((
                    w.id,
                    w.access,
                    w.at.clone(),
                    cond(w.id, &w.condition)?,
                    w.once,
                ))
            })
            .collect::<Result<_, DebugError>>()?,
    })
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

    let until = condition.map(Until::new);
    if let Some(until) = &until {
        for (addr, bank) in &until.writes {
            write.insert(*addr, *bank);
        }
    }

    let mut used = Usage::default();
    let mut warnings: Vec<EvalWarning> = Vec::new();
    let finish = |m: &Machine, reason, used: Usage, warnings: Vec<EvalWarning>| {
        Ok(Outcome {
            stop: record(m, reason, used, &opts.since),
            warnings,
        })
    };

    // Before the first instruction: a predicate that already holds must not
    // spend the budget. This also makes resuming idempotent.
    if let Some(until) = &until {
        match evaluate(m, opts, until.condition, &mut warnings) {
            Ok(true) => return finish(m, until.reason(), used, warnings),
            Ok(false) => {}
            Err(error) => {
                return Err(DebugError::Until {
                    error,
                    stop: Box::new(record(m, until.reason(), used, &opts.since)),
                });
            }
        }
    }

    loop {
        if let Some(kind) = opts.budget.exhausted(&used) {
            return finish(m, StopReason::BudgetExhausted { kind }, used, warnings);
        }
        if opts
            .interrupt
            .is_some_and(|flag| flag.load(Ordering::Relaxed))
        {
            return finish(m, StopReason::UserBreak, used, warnings);
        }
        if halted_forever(m.core()) {
            return finish(m, StopReason::HaltForever, used, warnings);
        }
        // The core skips an execution filter at the first instruction of each
        // call, so resuming from a breakpoint makes progress. After a
        // passed-over candidate, that skipped PC may be the goal: the boot ROM
        // falls through to $0100 right after the FF50 write the engine passed
        // over, so checking it here is what makes the handoff stoppable.
        if used.instructions > 0 {
            let pc = m.core().registers().pc;
            if exec.may_match(pc) {
                let bank = mem::bank_at(m.core(), pc);
                if exec.matches(pc, bank) {
                    let old = mem::peek(m.core(), pc).ok();
                    let hit = Hit {
                        access: Access::Exec,
                        addr: pc,
                        bank,
                        old,
                        new: old.unwrap_or(0xFF),
                        pc,
                        vetoed: false,
                    };
                    match decide(m, &compiled, &goal, hit, opts, &mut warnings) {
                        Err((id, reason, error)) => {
                            return Err(DebugError::Eval {
                                id,
                                error,
                                stop: Box::new(record(m, reason, used, &opts.since)),
                            });
                        }
                        Ok(Some(reason)) => return finish(m, reason, used, warnings),
                        Ok(None) => {}
                    }
                }
            }
        }
        let fine = goal.fine()
            || until.as_ref().is_some_and(|u| {
                matches!(u.cadence, Cadence::Scanline | Cadence::EveryInstruction)
            });
        let chunk = if fine { 1 } else { CHUNK };
        let before = m.core().registers();
        let ly_before = m.core().ppu().ly;
        let opcode = mem::peek(m.core(), before.pc).unwrap_or(0);
        let mut req = RunRequest::new(
            opts.budget
                .remaining_after(&used)
                .capped_instructions(chunk),
        );
        req.exec = &exec;
        req.read = &read;
        req.write = &write;
        req.stop_at_frame =
            goal == Goal::Frame || until.as_ref().is_some_and(|u| u.cadence == Cadence::Vblank);
        let stop = m.core_mut().run(&mut req)?;
        used += stop.used;
        let fired = match &until {
            Some(u) => {
                let wrote = matches!(stop.reason, CoreReason::Candidate(hit) if hit.access == Access::Write);
                wrote
                    || match u.cadence {
                        Cadence::Writes => false,
                        Cadence::Vblank => stop.used.frames > 0,
                        Cadence::Scanline => m.core().ppu().ly != ly_before,
                        Cadence::EveryInstruction => stop.used.instructions > 0,
                    }
            }
            None => false,
        };
        let decided = match stop.reason {
            CoreReason::Locked { opcode } => Some(StopReason::IllegalOpcode { opcode }),
            CoreReason::Stopped => Some(StopReason::Stopped),
            CoreReason::Frame => {
                if goal == Goal::Frame {
                    Some(StopReason::StepDone {
                        unit: StepKind::Frame,
                    })
                } else {
                    None
                }
            }
            CoreReason::Candidate(hit) => {
                let stopped = decide(m, &compiled, &goal, hit, opts, &mut warnings);
                match stopped {
                    // The stop is where the breakpoint or watch hit; the
                    // error says why its condition couldn't be decided.
                    Err((id, reason, error)) => {
                        return Err(DebugError::Eval {
                            id,
                            error,
                            stop: Box::new(record(m, reason, used, &opts.since)),
                        });
                    }
                    Ok(Some(reason)) => Some(reason),
                    Ok(None) => None,
                }
            }
            CoreReason::Budget(_) | CoreReason::Step(_) => None,
        };
        if let Some(reason) = decided {
            if let StopReason::Watchpoint { id, .. } = reason {
                remove_once(m, &compiled, id);
            }
            return finish(m, reason, used, warnings);
        }
        if fired {
            let Some(until) = &until else { continue };
            match evaluate(m, opts, until.condition, &mut warnings) {
                Ok(true) => {
                    // A breakpoint at this boundary wins; a predicate that
                    // lands on one reports the breakpoint instead.
                    match exec_breakpoint(m, &compiled, opts, used, &mut warnings) {
                        Ok(Some(reason)) => return finish(m, reason, used, warnings),
                        Ok(None) => return finish(m, until.reason(), used, warnings),
                        Err(error) => return Err(error),
                    }
                }
                Ok(false) => {}
                Err(error) => {
                    return Err(DebugError::Until {
                        error,
                        stop: Box::new(record(m, until.reason(), used, &opts.since)),
                    });
                }
            }
        }
        let after = m.core().registers();
        let done = match goal {
            Goal::Instruction => used.instructions >= 1,
            Goal::Out { sp } => is_return(opcode) && after.sp > sp && after.pc != before.pc,
            Goal::Scanline { ly } => m.core().ppu().ly != ly,
            _ => false,
        };
        if done {
            let unit = match goal {
                Goal::Instruction => StepKind::Instruction,
                Goal::Out { .. } => StepKind::Out,
                _ => StepKind::Scanline,
            };
            return finish(m, StopReason::StepDone { unit }, used, warnings);
        }
    }
}

/// The breakpoint that would stop at the current boundary, if any. A
/// predicate stop yields to one, since both describe the same instruction.
fn exec_breakpoint(
    m: &mut Machine,
    compiled: &Compiled,
    opts: &RunOptions<'_>,
    used: Usage,
    warnings: &mut Vec<EvalWarning>,
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
    match decide(m, compiled, &Goal::Run, hit, opts, warnings) {
        Ok(stop) => Ok(stop),
        Err((id, reason, error)) => Err(DebugError::Eval {
            id,
            error,
            stop: Box::new(record(m, reason, used, &opts.since)),
        }),
    }
}

fn remove_once(m: &mut Machine, compiled: &Compiled, id: u32) {
    if compiled.watches.iter().any(|w| w.0 == id && w.4) {
        m.delete(id);
    }
}

/// A stop or not; or the stop a condition couldn't decide, and why.
type Decision = Result<Option<StopReason>, (u32, StopReason, crate::cond::EvalError)>;

/// Whether a candidate is a real stop: a goal reached, or a breakpoint or
/// watch whose condition holds.
fn decide(
    m: &mut Machine,
    compiled: &Compiled,
    goal: &Goal,
    hit: Hit,
    opts: &RunOptions<'_>,
    warnings: &mut Vec<EvalWarning>,
) -> Decision {
    if hit.access == Access::Write && hit.addr == BOOT_OFF && m.boot.is_none() {
        m.boot = Some(TimeMark {
            ticks: m.core().now().0,
            frame: frame_number(m.core()),
        });
    }
    let base = super::record::base(m, &opts.since).1;
    let ctx = |access| EvalCtx {
        core: m.core(),
        symbols: m.symbols(),
        access,
        base,
        allow_uninit: opts.allow_uninit,
    };
    let holds =
        |reason: &StopReason, cond: &Option<Condition>, access, warnings: &mut Vec<EvalWarning>| {
            let id = match reason {
                StopReason::Breakpoint { id } | StopReason::Watchpoint { id, .. } => *id,
                _ => 0,
            };
            match cond {
                None => Ok(true),
                Some(c) => c
                    .holds(&ctx(access), warnings)
                    .map_err(|e| (id, reason.clone(), e)),
            }
        };
    match hit.access {
        Access::Exec => {
            match goal {
                Goal::To(at) if matches(at, hit.addr, hit.bank) => {
                    return Ok(Some(StopReason::StepDone {
                        unit: StepKind::ToAddress,
                    }));
                }
                Goal::Over { ret, sp, .. }
                    if hit.addr == *ret && m.core().registers().sp >= *sp =>
                {
                    return Ok(Some(StopReason::StepDone {
                        unit: StepKind::Over,
                    }));
                }
                Goal::Instruction => {
                    return Ok(Some(StopReason::StepDone {
                        unit: StepKind::Instruction,
                    }));
                }
                _ => {}
            }
            for (id, at, cond) in &compiled.breakpoints {
                let reason = StopReason::Breakpoint { id: *id };
                if matches(at, hit.addr, hit.bank) && holds(&reason, cond, None, warnings)? {
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
                    && matches(at, hit.addr, hit.bank)
                    && holds(&reason, cond, values, warnings)?
                {
                    return Ok(Some(reason));
                }
            }
        }
    }
    Ok(None)
}
