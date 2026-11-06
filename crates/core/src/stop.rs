//! Why a run or step ended.

use crate::run::{BudgetKind, StepUnit, Usage};
use crate::time::Ticks;

/// The kind of access a filter matched.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Access {
    Exec,
    Read,
    Write,
}

/// A filter hit: the core's half of a breakpoint or watch. The engine decides
/// whether it is a real stop (design §2.1).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Hit {
    pub access: Access,
    /// The canonical address (echo RAM reported at its WRAM address).
    pub addr: u16,
    /// The bank mapped at `addr` at the time of the access.
    pub bank: u16,
    /// The byte before the access (for an execution hit, the opcode), when it
    /// can be known without a bus read.
    pub old: Option<u8>,
    /// The byte read, or the value written (or attempted, if vetoed).
    pub new: u8,
    /// The PC of the instruction that made the access.
    pub pc: u16,
    /// Whether a write veto kept memory unchanged.
    pub vetoed: bool,
}

/// Why the core stopped. Only what a core can produce; the engine's stop
/// record adds the decisions it makes on top (breakpoint ids, predicates,
/// lockstep divergence).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum StopReason {
    /// A budget limit was reached.
    Budget(BudgetKind),
    /// A filter matched; engine-internal, never reaches a client as is.
    Candidate(Hit),
    /// The vblank `stop_at_frame` asked for.
    Frame,
    /// A single step completed.
    Step(StepUnit),
    /// The next instruction is an illegal opcode, which hangs the CPU. The core
    /// stops before executing it.
    Locked { opcode: u8 },
    /// The CPU is in STOP mode and no key is held to wake it.
    Stopped,
}

/// A stop, always at an instruction boundary.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Stop {
    pub reason: StopReason,
    /// The PC of the next instruction.
    pub pc: u16,
    /// The bank mapped at `pc`.
    pub bank: u16,
    /// Emulated time since power-on.
    pub at: Ticks,
    /// What the run used of its budget.
    pub used: Usage,
}
