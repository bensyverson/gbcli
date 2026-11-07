//! `measure`: emulated cycles from an arrival at one address to the next
//! arrival at another, over several iterations (design §11).

use std::fmt;

use gbcli_core::{
    AddrFilter, BankSel, Budget, Core, CoreError, InStop, RunRequest, StopReason, Ticks, Usage,
};
use gbcli_protocol::Location;

use super::{CpuSpeed, Span};
use crate::machine::Machine;
use crate::mem;

/// KEY1, whose bit 0 arms a CGB speed switch for the next STOP.
const KEY1: u16 = 0xFF4D;

/// What to measure.
#[derive(Clone, Debug)]
pub struct MeasureSpec {
    /// Each iteration starts at the first arrival here (the current PC counts).
    pub from: Location,
    /// And ends at the next arrival here, after at least one instruction.
    pub to: Location,
    /// At least one.
    pub iterations: u32,
    /// For the whole measurement, all iterations together.
    pub budget: Budget,
}

/// The cycles of each iteration, in order.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Measurement {
    /// The CPU speed every iteration ran at.
    pub speed: CpuSpeed,
    pub samples: Vec<Span>,
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
    /// Measures `spec` on a copy of this machine, which is left unchanged.
    pub fn measure(&self, spec: &MeasureSpec) -> Result<Measurement, MeasureError> {
        if spec.iterations == 0 {
            return Err(MeasureError::NoIterations);
        }
        let mut copy = self.try_clone()?;
        measure_on(copy.core_mut(), spec)
    }
}

fn measure_on(core: &mut dyn Core, spec: &MeasureSpec) -> Result<Measurement, MeasureError> {
    let from = filter(core, &spec.from);
    let to = filter(core, &spec.to);
    let mut key1 = AddrFilter::new();
    key1.insert(KEY1, BankSel::Any);
    let speed = CpuSpeed::of(core);
    let mut run = Runner {
        core,
        key1: &key1,
        budget: spec.budget,
        used: Usage::default(),
        speed,
    };
    let mut samples = Vec::new();
    for iteration in 0..spec.iterations {
        let at = |leg| (iteration, leg);
        if !run.at(&from) {
            run.until(&from, at(Leg::ToStart))?;
        }
        let start = run.core.now();
        run.until(&to, at(Leg::ToEnd))?;
        samples.push(Span {
            ticks: run.core.now() - start,
        });
    }
    Ok(Measurement { speed, samples })
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
    core: &'a mut dyn Core,
    key1: &'a AddrFilter,
    budget: Budget,
    used: Usage,
    speed: CpuSpeed,
}

impl Runner<'_> {
    /// Whether PC is at `filter` now.
    fn at(&self, filter: &AddrFilter) -> bool {
        let pc = self.core.registers().pc;
        filter.matches(pc, mem::bank_at(self.core, pc))
    }

    /// Runs until PC arrives at `filter` (the core never stops on an
    /// execution hit before its first instruction, so this always moves).
    /// Every KEY1 write stops the core and the speed is checked at every
    /// stop: a switch clears KEY1's arm bit, so switching back needs another
    /// KEY1 write, where the first switch shows. A switch and back can't
    /// hide between two checks.
    fn until(
        &mut self,
        filter: &AddrFilter,
        (iteration, leg): (u32, Leg),
    ) -> Result<(), MeasureError> {
        let completed = iteration;
        loop {
            let mut req = RunRequest::new(self.budget.remaining_after(&self.used));
            // No key comes during a measurement, so a STOP nothing can wake
            // ends it rather than spending the budget.
            req.in_stop = InStop::End;
            req.exec = filter;
            req.write = self.key1;
            let stop = self.core.run(&mut req)?;
            self.used += stop.used;
            if CpuSpeed::of(self.core) != self.speed {
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
                StopReason::Budget(_) => return Err(MeasureError::Budget { completed, leg }),
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
