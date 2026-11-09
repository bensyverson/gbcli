//! Cycles per routine: an execution sink keeps a shadow call stack and
//! charges every CPU step to the routine on top of it.
//!
//! **The shadow stack.** A taken CALL or RST, or an interrupt dispatch,
//! pushes a frame recording the callee's entry, the return address and SP
//! after the push (where the return address lives). A taken RET or RETI pops
//! the frame whose return-address slot it reads (SP before the RET equals the
//! frame's SP). Programs manage the stack themselves, so when the shadow and
//! real stacks disagree the real one wins:
//!
//! - **A RET that reads a slot above the root's stack** (one pushed before
//!   the profile began) returns from the root: any open frames unwind, and
//!   the routine it returns into becomes the root. The root's stack starts at
//!   SP when the profile began and rises with SP whenever no frame is open
//!   (`ld sp`, a `pop` at the root's level): at power-on SP is $0000, and
//!   only the boot ROM's `ld sp` says where the stack is.
//! - **A RET that reads no frame's slot** otherwise (`push hl; ret`, a
//!   computed jump) is a jump: nothing is popped (`ret_jumps`).
//! - **A RET that reads a deeper frame's slot** pops the frames above it as
//!   unwound, then that frame (`unwound`).
//! - **SP rising past a frame's slot without a RET** (`pop hl` or `add sp`
//!   dropping a return address, `ld sp`) unwinds that frame: its return
//!   address is gone, so the routine has ended (`unwound`).
//! - **A RET to an address other than the frame's return address** still pops
//!   that frame (`mismatched`), since the slot it read was the frame's.
//!
//! **Charging.** A step's cycles go to the routine on top before the step, so
//! a CALL is the caller's and a RET the callee's; an interrupt dispatch is the
//! handler's. Inclusive cost is counted once per routine however deep it
//! recurses.
//!
//! **The root** is the outermost routine the profile has seen running, named
//! by the address it was first seen at: at first the step the profile began
//! at, which may be mid-routine (`Leaf+$2`). When it returns (a RET or RETI
//! reading a return address pushed before the profile began), the routine it
//! returns into becomes the root, named by the return address
//! (`MainLoop+$3`): its entry was never seen. A root that returned keeps the
//! cycles it ran and counts its inclusive cost to its return; the current
//! root has no calls, and its inclusive cost is the whole profile, since it
//! was running, under its callees, all along.

use std::collections::HashMap;

use gbcli_core::{
    Budget, CoreError, CpuStep, Event, EventKinds, EventSink, RunRequest, StopReason, Ticks,
};

use crate::machine::Machine;
use crate::sym::Symbols;

/// A routine, by the address it was entered at.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Entry {
    pub bank: u16,
    pub addr: u16,
}

/// What one routine cost over a profile, in CPU T-cycles at the speed each
/// step ran at.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RoutineCost {
    pub entry: Entry,
    /// The `.sym` label at the entry, the hardware vector entered when no
    /// label is there (`STAT_vector`), or `Label+$n` inside one.
    pub name: Option<String>,
    /// Times entered by a CALL, RST or interrupt.
    pub calls: u64,
    /// Cycles of its own instructions.
    pub exclusive_t: u64,
    /// Cycles from entry to return, callees included.
    pub inclusive_t: u64,
}

/// How often the shadow stack had to follow the real one (see the module
/// documentation).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct ShadowStats {
    pub unwound: u64,
    pub mismatched: u64,
    pub ret_jumps: u64,
    /// Frames still open when the profile ended.
    pub open: u64,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Profile {
    /// Most exclusive cycles first.
    pub routines: Vec<RoutineCost>,
    pub total_t: u64,
    /// Of `total_t`, the steps where the CPU was halted or stopped.
    pub idle_t: u64,
    pub ticks: Ticks,
    pub shadow: ShadowStats,
}

struct Frame {
    entry: Entry,
    ret: u16,
    sp: u16,
    /// The profile's T count at entry.
    start_t: u64,
    /// Cycles charged to this frame so far, added to its routine at pop.
    exclusive_t: u64,
}

#[derive(Default)]
struct Cost {
    calls: u64,
    exclusive_t: u64,
    inclusive_t: u64,
    /// Open frames of this routine, so recursion is counted once.
    active: u32,
}

/// The sink behind [`Machine::profile`]; usable on any run whose request
/// takes a sink.
#[derive(Default)]
pub struct Routines {
    root: Option<Entry>,
    /// SP as the root runs: a RET reading a slot at or above it returns
    /// from the root.
    root_sp: u16,
    stack: Vec<Frame>,
    costs: HashMap<Entry, Cost>,
    total_t: u64,
    idle_t: u64,
    ticks: u64,
    shadow: ShadowStats,
    root_exclusive_t: u64,
}

impl EventSink for Routines {
    fn kinds(&self) -> EventKinds {
        EventKinds {
            exec: true,
            raster: false,
            ppu_modes: false,
        }
    }

    fn event(&mut self, event: Event) {
        if let Event::Exec {
            pc,
            bank,
            step,
            ticks,
            sp_before,
            sp_after,
            next_pc,
            next_bank,
            double_speed,
        } = event
        {
            if self.root.is_none() {
                self.set_root(Entry { bank, addr: pc }, sp_before);
            }
            let t = u64::from(ticks) / if double_speed { 1 } else { 2 };
            let callee = Entry {
                bank: next_bank,
                addr: next_pc,
            };
            if step == CpuStep::Interrupt {
                self.push(callee, pc, sp_after);
            }
            // Charged in the frame, not the map: this runs every step.
            match self.stack.last_mut() {
                Some(frame) => frame.exclusive_t += t,
                None => self.root_exclusive_t += t,
            }
            self.total_t += t;
            self.ticks += u64::from(ticks);
            if step == CpuStep::Idle {
                self.idle_t += t;
            }
            if let CpuStep::Instruction { opcode } = step {
                if let Some(len) = call_len(opcode)
                    && sp_after == sp_before.wrapping_sub(2)
                {
                    self.push(callee, pc.wrapping_add(len), sp_after);
                } else if is_return(opcode) && sp_after == sp_before.wrapping_add(2) {
                    self.ret(sp_before, callee, sp_after);
                }
            }
            while self.stack.last().is_some_and(|f| f.sp < sp_after) {
                self.shadow.unwound += 1;
                self.pop();
            }
            // SP rising with no frame open (`ld sp`, a `pop` at the root's
            // level) moves the root's stack up: slots below it are the
            // root's own from here, so a RET reading one is a jump.
            if self.stack.is_empty() && sp_after > self.root_sp {
                self.root_sp = sp_after;
            }
        }
    }
}

impl Routines {
    pub fn new() -> Routines {
        Routines::default()
    }

    /// The profile so far, named from `symbols`. Open frames count up to now.
    pub fn finish(mut self, symbols: Option<&Symbols>) -> Profile {
        self.shadow.open = self.stack.len() as u64;
        while !self.stack.is_empty() {
            self.pop();
        }
        self.end_root();
        let mut routines: Vec<RoutineCost> = self
            .costs
            .into_iter()
            .map(|(entry, c)| RoutineCost {
                entry,
                name: super::names::routine_name(symbols, entry),
                calls: c.calls,
                exclusive_t: c.exclusive_t,
                inclusive_t: c.inclusive_t,
            })
            .collect();
        routines.sort_by(|a, b| {
            (b.exclusive_t, a.entry.bank, a.entry.addr).cmp(&(
                a.exclusive_t,
                b.entry.bank,
                b.entry.addr,
            ))
        });
        Profile {
            routines,
            total_t: self.total_t,
            idle_t: self.idle_t,
            ticks: Ticks(self.ticks),
            shadow: self.shadow,
        }
    }

    fn set_root(&mut self, root: Entry, sp: u16) {
        self.root = Some(root);
        self.root_sp = sp;
        self.costs.entry(root).or_default();
    }

    /// Closes the root's books: it ran from the start of the profile to now.
    fn end_root(&mut self) {
        if let Some(root) = self.root {
            let cost = self.costs.entry(root).or_default();
            cost.inclusive_t += self.total_t;
            cost.exclusive_t += std::mem::take(&mut self.root_exclusive_t);
        }
    }

    fn push(&mut self, entry: Entry, ret: u16, sp: u16) {
        let cost = self.costs.entry(entry).or_default();
        cost.calls += 1;
        cost.active += 1;
        self.stack.push(Frame {
            entry,
            ret,
            sp,
            start_t: self.total_t,
            exclusive_t: 0,
        });
    }

    fn pop(&mut self) {
        let Some(frame) = self.stack.pop() else {
            return;
        };
        let cost = self.costs.entry(frame.entry).or_default();
        cost.active -= 1;
        cost.exclusive_t += frame.exclusive_t;
        if cost.active == 0 {
            cost.inclusive_t += self.total_t - frame.start_t;
        }
    }

    /// A taken RET that read the return-address slot at `sp` and went `to`,
    /// leaving SP at `sp_after`.
    fn ret(&mut self, sp: u16, to: Entry, sp_after: u16) {
        let Some(depth) = self.stack.iter().rposition(|f| f.sp == sp) else {
            if sp >= self.root_sp {
                self.shadow.unwound += self.stack.len() as u64;
                while !self.stack.is_empty() {
                    self.pop();
                }
                self.end_root();
                self.set_root(to, sp_after);
            } else {
                self.shadow.ret_jumps += 1;
            }
            return;
        };
        while self.stack.len() > depth + 1 {
            self.shadow.unwound += 1;
            self.pop();
        }
        if self.stack[depth].ret != to.addr {
            self.shadow.mismatched += 1;
        }
        self.pop();
    }
}

/// The length of a CALL or RST opcode, which pushes a return address.
fn call_len(opcode: u8) -> Option<u16> {
    match opcode {
        0xCD | 0xC4 | 0xCC | 0xD4 | 0xDC => Some(3),
        0xC7 | 0xCF | 0xD7 | 0xDF | 0xE7 | 0xEF | 0xF7 | 0xFF => Some(1),
        _ => None,
    }
}

fn is_return(opcode: u8) -> bool {
    matches!(opcode, 0xC9 | 0xD9 | 0xC0 | 0xC8 | 0xD0 | 0xD8)
}

impl Machine {
    /// Runs for `budget` with every step charged to a routine, ignoring
    /// breakpoints and watches. The machine moves, as a run does.
    pub fn profile(&mut self, budget: Budget) -> Result<Profile, CoreError> {
        let mut sink = Routines::new();
        let mut req = RunRequest::new(budget);
        req.sink = Some(&mut sink);
        let mut used = gbcli_core::Usage::default();
        loop {
            req.budget = budget.remaining_after(&used);
            let stop = self.core_mut().run(&mut req)?;
            used += stop.used;
            match stop.reason {
                StopReason::Frame | StopReason::Candidate(_) | StopReason::Step(_) => continue,
                _ => break,
            }
        }
        Ok(sink.finish(self.symbols()))
    }
}
