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
/// Never empty. An instruction makes at most five memory accesses (`call`:
/// three fetches, two writes), so [`Hits::CAPACITY`] is never reached on a
/// faithful core; if one were, the excess is dropped and [`overflowed`]
/// says so.
///
/// [`overflowed`]: Hits::overflowed
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Hits {
    hits: [Option<Hit>; Hits::CAPACITY],
    overflowed: bool,
}

impl Hits {
    pub const CAPACITY: usize = 8;

    pub fn one(hit: Hit) -> Hits {
        let mut hits = [None; Hits::CAPACITY];
        hits[0] = Some(hit);
        Hits {
            hits,
            overflowed: false,
        }
    }

    /// Appends a later hit of the same instruction.
    pub fn push(&mut self, hit: Hit) {
        match self.hits.iter_mut().find(|h| h.is_none()) {
            Some(slot) => *slot = Some(hit),
            None => self.overflowed = true,
        }
    }

    /// The first hit.
    pub fn first(&self) -> Hit {
        self.hits[0].expect("Hits is never empty")
    }

    pub fn iter(&self) -> impl Iterator<Item = &Hit> {
        self.hits.iter().map_while(Option::as_ref)
    }

    /// Whether hits were dropped for lack of room.
    pub fn overflowed(&self) -> bool {
        self.overflowed
    }
}

/// Why the core stopped. Only what a core can produce; the engine's stop
/// record adds the decisions it makes on top (breakpoint ids, predicates,
/// lockstep divergence).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
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
    fn hits_keep_every_access_in_order_and_flag_overflow() {
        let mut hits = Hits::one(hit(1));
        hits.push(hit(2));
        assert_eq!(hits.first().addr, 1);
        let addrs: Vec<u16> = hits.iter().map(|h| h.addr).collect();
        assert_eq!(addrs, [1, 2]);
        assert!(!hits.overflowed());
        for addr in 3..=9 {
            hits.push(hit(addr));
        }
        assert_eq!(hits.iter().count(), Hits::CAPACITY);
        assert!(hits.overflowed());
    }
}
