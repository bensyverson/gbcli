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

/// Every filter hit of one instruction, in access order: a hit that decides
/// nothing (a watch whose condition is false) must not hide a later one in
/// the same instruction (`inc [hl]` reads, then writes).
///
/// Never empty, and never full: an instruction makes at most five memory
/// accesses of its own (`call`: three fetches, two writes), but a CGB
/// general-purpose HDMA reads up to 2 KiB of source in the step of the
/// instruction that starts it, so a fixed buffer would drop hits. The first
/// hit is stored inline; only a step with more allocates.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Hits {
    first: Hit,
    rest: Vec<Hit>,
}

impl Hits {
    pub fn one(hit: Hit) -> Hits {
        Hits {
            first: hit,
            rest: Vec::new(),
        }
    }

    /// Appends a later hit of the same instruction.
    pub fn push(&mut self, hit: Hit) {
        self.rest.push(hit);
    }

    /// The first hit.
    pub fn first(&self) -> Hit {
        self.first
    }

    pub fn iter(&self) -> impl Iterator<Item = &Hit> {
        std::iter::once(&self.first).chain(&self.rest)
    }
}

/// Why the core stopped. Only what a core can produce; the engine's stop
/// record adds the decisions it makes on top (breakpoint ids, predicates,
/// lockstep divergence).
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum StopReason {
    /// A budget limit was reached.
    Budget(BudgetKind),
    /// Filters matched; engine-internal, never reaches a client as is.
    Candidate(Hits),
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
#[derive(Clone, Debug, PartialEq, Eq)]
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

#[cfg(test)]
mod tests {
    use super::*;

    fn hit(addr: u16) -> Hit {
        Hit {
            access: Access::Write,
            addr,
            bank: 0,
            old: None,
            new: 0,
            pc: 0,
            vetoed: false,
        }
    }

    #[test]
    fn hits_keep_every_access_in_order_however_many() {
        let mut hits = Hits::one(hit(1));
        for addr in 2..=0x800 {
            hits.push(hit(addr));
        }
        assert_eq!(hits.first().addr, 1);
        let addrs: Vec<u16> = hits.iter().map(|h| h.addr).collect();
        assert_eq!(addrs, (1..=0x800).collect::<Vec<u16>>());
    }
}
