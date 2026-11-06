//! Run requests: what to run for, what to watch, and where events go.

use crate::filter::AddrFilter;
use crate::time::Ticks;

/// Which limit of a [`Budget`] ended a run.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BudgetKind {
    Ticks,
    Instructions,
    Frames,
}

/// How far a run may go: whichever limit is reached first. Always finite,
/// since every constructor sets at least one limit.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Budget {
    ticks: Option<u64>,
    instructions: Option<u64>,
    frames: Option<u64>,
}

/// What a run has used so far, in each budget unit.
///
/// An instruction is one CPU step: an instruction, an interrupt dispatch, or
/// one HALT/STOP tick. A frame is one vblank of any kind, including the
/// synthetic ones the core produces while the LCD is off.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Usage {
    pub ticks: u64,
    pub instructions: u64,
    pub frames: u64,
}

impl Budget {
    pub fn ticks(n: u64) -> Self {
        Budget::default_with().with_ticks(n)
    }
    pub fn instructions(n: u64) -> Self {
        Budget::default_with().with_instructions(n)
    }
    pub fn frames(n: u64) -> Self {
        Budget::default_with().with_frames(n)
    }

    /// Adds (or replaces) the tick limit.
    pub fn with_ticks(mut self, n: u64) -> Self {
        self.ticks = Some(n);
        self
    }
    /// Adds (or replaces) the instruction limit.
    pub fn with_instructions(mut self, n: u64) -> Self {
        self.instructions = Some(n);
        self
    }
    /// Adds (or replaces) the frame limit.
    pub fn with_frames(mut self, n: u64) -> Self {
        self.frames = Some(n);
        self
    }

    const fn default_with() -> Self {
        Budget {
            ticks: None,
            instructions: None,
            frames: None,
        }
    }

    /// The limit `used` has reached, if any. Frames are checked first, then
    /// instructions, then ticks, so a tie names the coarsest unit.
    pub fn exhausted(&self, used: &Usage) -> Option<BudgetKind> {
        let reached = |limit: Option<u64>, used: u64| limit.is_some_and(|limit| used >= limit);
        if reached(self.frames, used.frames) {
            Some(BudgetKind::Frames)
        } else if reached(self.instructions, used.instructions) {
            Some(BudgetKind::Instructions)
        } else if reached(self.ticks, used.ticks) {
            Some(BudgetKind::Ticks)
        } else {
            None
        }
    }
}

/// The unit of a single step.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum StepUnit {
    Instruction,
    MCycle,
    Dot,
}

/// One run: a budget, the candidate filters, and optional event taps.
///
/// The core only filters; the engine decides (design §2.1). A filter hit ends
/// the run with [`StopReason::Candidate`](crate::StopReason::Candidate).
pub struct RunRequest<'a> {
    pub budget: Budget,
    /// Candidate breakpoints, checked before each instruction executes.
    pub exec: &'a AddrFilter,
    /// Candidate read watches, checked after the access.
    pub read: &'a AddrFilter,
    /// Candidate write watches, checked as the access happens.
    pub write: &'a AddrFilter,
    /// Addresses whose writes are vetoed: memory keeps its old value and the
    /// run stops with the attempted value. Implies a write candidate.
    pub veto: &'a AddrFilter,
    /// Stop after the next vblank.
    pub stop_at_frame: bool,
    /// Taps and the profiler; pure data push, no core access.
    pub sink: Option<&'a mut dyn EventSink>,
}

static EMPTY: AddrFilter = AddrFilter::EMPTY;

impl<'a> RunRequest<'a> {
    /// A request with no filters, no frame stop and no sink.
    pub fn new(budget: Budget) -> Self {
        RunRequest {
            budget,
            exec: &EMPTY,
            read: &EMPTY,
            write: &EMPTY,
            veto: &EMPTY,
            stop_at_frame: false,
            sink: None,
        }
    }
}

/// A PPU mode as STAT reports it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PpuMode {
    HBlank,
    VBlank,
    OamScan,
    Drawing,
}

/// Something a tap or the profiler receives during a run.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Event {
    IoWrite { reg: u8, value: u8, at: Ticks },
    PpuMode { mode: PpuMode, ly: u8, at: Ticks },
    Scanline { ly: u8, at: Ticks },
    Exec { pc: u16, bank: u16, cycles: u32 },
}

/// Receives events during a run. Must not call back into the core.
pub trait EventSink {
    fn event(&mut self, event: Event);
}

#[cfg(test)]
mod tests {
    use super::*;

    fn used(ticks: u64, instructions: u64, frames: u64) -> Usage {
        Usage {
            ticks,
            instructions,
            frames,
        }
    }

    #[test]
    fn a_tick_budget_ends_at_its_limit() {
        let budget = Budget::ticks(100);
        assert_eq!(budget.exhausted(&used(99, 1_000, 1_000)), None);
        assert_eq!(budget.exhausted(&used(100, 0, 0)), Some(BudgetKind::Ticks));
    }

    #[test]
    fn an_instruction_budget_ends_at_its_limit() {
        let budget = Budget::instructions(5);
        assert_eq!(budget.exhausted(&used(u64::MAX, 4, 9)), None);
        assert_eq!(
            budget.exhausted(&used(0, 5, 0)),
            Some(BudgetKind::Instructions)
        );
    }

    #[test]
    fn a_frame_budget_ends_at_its_limit() {
        let budget = Budget::frames(2);
        assert_eq!(budget.exhausted(&used(0, 0, 1)), None);
        assert_eq!(budget.exhausted(&used(0, 0, 2)), Some(BudgetKind::Frames));
    }

    #[test]
    fn whichever_limit_is_reached_first_ends_the_run() {
        let budget = Budget::frames(10).with_ticks(500);
        assert_eq!(budget.exhausted(&used(500, 0, 3)), Some(BudgetKind::Ticks));
        assert_eq!(budget.exhausted(&used(10, 0, 10)), Some(BudgetKind::Frames));
    }

    #[test]
    fn a_tie_names_the_coarsest_unit() {
        let budget = Budget::frames(1).with_instructions(7).with_ticks(9);
        assert_eq!(budget.exhausted(&used(9, 7, 1)), Some(BudgetKind::Frames));
        assert_eq!(
            budget.exhausted(&used(9, 7, 0)),
            Some(BudgetKind::Instructions)
        );
    }
}
