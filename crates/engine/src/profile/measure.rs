//! `measure`: emulated cycles from an arrival at one address to the next
//! arrival at another, over several iterations (design §11).

use std::fmt;
use std::sync::atomic::{AtomicBool, Ordering};

use gbcli_core::{
    AddrFilter, BankSel, Budget, Core, CoreError, FirstStep, InStop, RunRequest, StopReason, Ticks,
    Usage,
};
use gbcli_protocol::{KeyEvent, Location};

use super::{CpuSpeed, Span};
use crate::debug::{CHUNK, Schedule};
use crate::machine::Machine;
use crate::mem;

/// KEY1, whose bit 0 arms a CGB speed switch for the next STOP.
const KEY1: u16 = 0xFF4D;

/// What to measure.
#[derive(Clone, Debug)]
pub struct MeasureSpec<'a> {
    /// Each iteration starts at the first arrival here. The current PC counts,
    /// unless the CPU is halted there: then the arrival comes at the wake.
    pub from: Location,
    /// And ends at the next arrival here, after at least one instruction.
    pub to: Location,
    /// At least one.
    pub iterations: u32,
    /// For the whole measurement, all iterations together.
    pub budget: Budget,
    /// Set from elsewhere to stop the measurement: a user break or wall guard.
    pub interrupt: Option<&'a AtomicBool>,
    /// Presses and releases at frame offsets from the measurement's start,
    /// applied to the copy as a run applies them.
    pub keys: &'a [KeyEvent],
}

/// The cycles of each iteration, in order.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Measurement {
    /// The CPU speed every iteration ran at.
    pub speed: CpuSpeed,
    pub samples: Vec<Span>,
    /// Frames completed while measuring, so the key events past the end
    /// can be named.
    pub frames: u64,
}

/// Minimum, maximum and mean of one unit over the iterations.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Stats {
    pub min: u64,
    pub max: u64,
    pub mean: f64,
}

impl Measurement {
    pub fn n(&self) -> usize {
        self.samples.len()
    }

    /// CPU T-cycles at the measured speed.
    pub fn t(&self) -> Stats {
        self.stats(|s| s.t(self.speed))
    }

    /// CPU M-cycles (four T) at the measured speed.
    pub fn m(&self) -> Stats {
        self.stats(|s| s.m(self.speed))
    }

    /// The share of a 70,224-dot frame, as a percentage. A frame takes the
    /// same emulated time at either speed, so this is the same at both.
    pub fn frame_percent(&self) -> (f64, f64, f64) {
        let pct = |ticks: u64| ticks as f64 * 100.0 / Ticks::PER_FRAME as f64;
        let ticks = self.stats(|s| s.ticks.0);
        (
            pct(ticks.min),
            pct(ticks.max),
            ticks.mean * 100.0 / Ticks::PER_FRAME as f64,
        )
    }

    /// Whether any iteration took more than `max_t` T-cycles (`--max`).
    pub fn exceeds(&self, max_t: u64) -> bool {
        self.t().max > max_t
    }

    fn stats(&self, f: impl Fn(&Span) -> u64) -> Stats {
        let values: Vec<u64> = self.samples.iter().map(f).collect();
        let sum: u64 = values.iter().sum();
        Stats {
            min: values.iter().copied().min().unwrap_or(0),
            max: values.iter().copied().max().unwrap_or(0),
            mean: sum as f64 / values.len().max(1) as f64,
        }
    }
}

/// Which half of an iteration a measurement stopped in.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Leg {
    /// Running to `from`.
    ToStart,
    /// Running from `from` to `to`.
    ToEnd,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum MeasureError {
    /// `iterations` was 0.
    NoIterations,
    /// The CPU speed changed during, or between, the iterations: T-cycles
    /// would mean two different things.
    SpeedSwitch {
        iteration: u32,
    },
    /// The budget ran out after `completed` iterations.
    Budget {
        completed: u32,
        leg: Leg,
    },
    /// The interrupt flag was set after `completed` iterations.
    Interrupted {
        completed: u32,
        leg: Leg,
    },
    /// The CPU locked up on an illegal opcode, or entered STOP with nothing
    /// to wake it, so the next arrival can never come.
    Halted {
        completed: u32,
        leg: Leg,
        reason: StopReason,
    },
    Core(CoreError),
}

impl fmt::Display for MeasureError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let leg = |leg: &Leg| match leg {
            Leg::ToStart => "before reaching --from",
            Leg::ToEnd => "before reaching --to",
        };
        match self {
            MeasureError::NoIterations => write!(f, "measure needs at least one iteration"),
            MeasureError::SpeedSwitch { iteration } => write!(
                f,
                "iteration {iteration} spans a CPU speed switch, so T-cycles are ambiguous"
            ),
            MeasureError::Budget { completed, leg: l } => {
                write!(f, "budget ran out {} after {completed} iterations", leg(l))
            }
            MeasureError::Interrupted { completed, leg: l } => {
                write!(f, "interrupted {} after {completed} iterations", leg(l))
            }
            MeasureError::Halted {
                completed, leg: l, ..
            } => write!(
                f,
                "the CPU stopped for good {} after {completed} iterations",
                leg(l)
            ),
            MeasureError::Core(e) => write!(f, "{e}"),
        }
    }
}

impl std::error::Error for MeasureError {}

impl From<CoreError> for MeasureError {
    fn from(e: CoreError) -> Self {
        MeasureError::Core(e)
    }
}

impl Machine {
    /// Measures `spec` on a copy of this machine, which doesn't run: copying
    /// saves it, which may only catch its APU up (see `Core::save`).
    pub fn measure(&mut self, spec: &MeasureSpec) -> Result<Measurement, MeasureError> {
        if spec.iterations == 0 {
            return Err(MeasureError::NoIterations);
        }
        let mut copy = self.try_clone()?;
        measure_on(&mut copy, spec)
    }
}

fn measure_on(m: &mut Machine, spec: &MeasureSpec) -> Result<Measurement, MeasureError> {
    let from = filter(m.core(), &spec.from);
    let to = filter(m.core(), &spec.to);
    let mut key1 = AddrFilter::new();
    key1.insert(KEY1, BankSel::Any);
    let speed = CpuSpeed::of(m.core());
    // As a run applies them: the events at frame 0 before the first step.
    let mut keys = Schedule::new(spec.keys);
    keys.apply_due(m, 0);
    let mut run = Runner {
        m,
        keys,
        key1: &key1,
        budget: spec.budget,
        used: Usage::default(),
        speed,
        interrupt: spec.interrupt,
    };
    let mut samples = Vec::new();
    for iteration in 0..spec.iterations {
        let at = |leg| (iteration, leg);
        if !run.at(&from) {
            run.until(&from, at(Leg::ToStart))?;
        }
        let start = run.m.core().now();
        run.until(&to, at(Leg::ToEnd))?;
        samples.push(Span {
            ticks: run.m.core().now() - start,
        });
    }
    Ok(Measurement {
        speed,
        samples,
        frames: run.used.frames,
    })
}

/// An exec filter for `at`, armed for the bank it reaches on this machine.
fn filter(core: &dyn Core, at: &Location) -> AddrFilter {
    let bank = at.bank.map_or(BankSel::Any, |bank| {
        BankSel::Bank(mem::effective_bank(core, bank, at.addr))
    });
    let mut filter = AddrFilter::new();
    filter.insert(gbcli_core::canonical_addr(at.addr), bank);
    filter
}

struct Runner<'a> {
    m: &'a mut Machine,
    /// The events still to come, applied at the vblank that ends their frame.
    keys: Schedule,
    key1: &'a AddrFilter,
    budget: Budget,
    used: Usage,
    speed: CpuSpeed,
    interrupt: Option<&'a AtomicBool>,
}

impl Runner<'_> {
    /// Whether the CPU has arrived at `filter`: it is about to execute the
    /// instruction there. Halted, PC already points past the HALT but the
    /// next step is a wait or a wake, so it hasn't (design §11 ruling).
    fn at(&self, filter: &AddrFilter) -> bool {
        let core = self.m.core();
        let regs = core.registers();
        !regs.halted && filter.matches(regs.pc, mem::bank_at(core, regs.pc))
    }

    /// Runs until PC arrives at `filter`, after at least one instruction: the
    /// leg's first run leaves its first step unchecked, so this always moves.
    /// A run resumed after a KEY1 write checks its first step, since the
    /// instruction after the write may be the arrival. Every KEY1 write
    /// stops the core and the speed is checked at every stop: a switch clears KEY1's arm bit, so switching back needs another
    /// KEY1 write, where the first switch shows. A switch and back can't
    /// hide between two checks.
    fn until(
        &mut self,
        filter: &AddrFilter,
        (iteration, leg): (u32, Leg),
    ) -> Result<(), MeasureError> {
        let completed = iteration;
        let mut first_step = FirstStep::Run;
        loop {
            if self.interrupt.is_some_and(|f| f.load(Ordering::Relaxed)) {
                return Err(MeasureError::Interrupted { completed, leg });
            }
            // In chunks, so the flag is polled promptly; a chunk's end is no
            // arrival, so the next one checks its first step.
            let mut req = RunRequest::new(
                self.budget
                    .remaining_after(&self.used)
                    .capped_instructions(CHUNK),
            );
            req.first_step = first_step;
            first_step = FirstStep::Check;
            // With no key still to come, a STOP nothing can wake ends the
            // measurement rather than spending the budget; while one is
            // coming, the core stops at each vblank so it lands on time.
            let pending = self.keys.pending();
            req.in_stop = if pending {
                InStop::PassTime
            } else {
                InStop::End
            };
            req.stop_at_frame = pending;
            req.exec = filter;
            req.write = self.key1;
            let stop = self.m.core_mut().run(&mut req)?;
            self.used += stop.used;
            self.keys.apply_due(self.m, self.used.frames);
            if CpuSpeed::of(self.m.core()) != self.speed {
                return Err(MeasureError::SpeedSwitch {
                    iteration: iteration + 1,
                });
            }
            match stop.reason {
                StopReason::Candidate(hits)
                    if hits.iter().any(|h| h.access == gbcli_core::Access::Exec) =>
                {
                    return Ok(());
                }
                // A KEY1 write: the speed check above is all it was for.
                StopReason::Candidate(_) => {}
                StopReason::Budget(_) if self.budget.exhausted(&self.used).is_some() => {
                    return Err(MeasureError::Budget { completed, leg });
                }
                StopReason::Budget(_) => {}
                reason @ (StopReason::Locked { .. } | StopReason::Stopped) => {
                    return Err(MeasureError::Halted {
                        completed,
                        leg,
                        reason,
                    });
                }
                StopReason::Frame | StopReason::Step(_) => {}
            }
        }
    }
}
