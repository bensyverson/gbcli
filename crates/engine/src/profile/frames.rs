//! The per-frame budget: how much of each frame the CPU worked and how much
//! it waited in HALT, and which frames lagged.
//!
//! **A lag frame** is a frame, from one vblank to the next, in which the CPU
//! never entered HALT or STOP: the game was busy for all of it, so its wait
//! for vblank didn't come before the vblank. This is the definition for games
//! that wait in HALT, as most homebrew does. A game that busy-waits (polling
//! LY or a flag) never halts, so every one of its frames reads as lag; for
//! those, `measure` from the top of the main loop to its wait is the tool.

use gbcli_core::{CoreError, CpuStep, Event, EventKinds, EventSink, RunRequest, StopReason};

use crate::machine::Machine;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct FrameBudget {
    /// CPU T-cycles spent running instructions and dispatching interrupts.
    pub active_t: u64,
    /// CPU T-cycles spent in HALT or STOP.
    pub idle_t: u64,
    pub lag: bool,
}

#[derive(Default)]
struct Split {
    frame: FrameBudget,
}

impl EventSink for Split {
    fn kinds(&self) -> EventKinds {
        EventKinds {
            exec: true,
            raster: false,
        }
    }

    fn event(&mut self, event: Event) {
        if let Event::Exec {
            step,
            ticks,
            double_speed,
            ..
        } = event
        {
            let t = u64::from(ticks) / if double_speed { 1 } else { 2 };
            match step {
                CpuStep::Idle => self.frame.idle_t += t,
                CpuStep::Instruction { .. } | CpuStep::Interrupt => self.frame.active_t += t,
            }
        }
    }
}

impl Machine {
    /// Runs `frames` frames and splits each into active and idle time. The
    /// first counts from where the machine is; each ends at the end of the
    /// step its vblank fell in. The machine moves, as a run does.
    pub fn frame_budget(&mut self, frames: u32) -> Result<Vec<FrameBudget>, CoreError> {
        let mut out = Vec::new();
        while out.len() < frames as usize {
            let mut split = Split::default();
            let mut req = RunRequest::new(gbcli_core::Budget::frames(1));
            req.stop_at_frame = true;
            req.sink = Some(&mut split);
            let stop = self.core_mut().run(&mut req)?;
            let mut frame = split.frame;
            frame.lag = frame.idle_t == 0;
            out.push(frame);
            if matches!(stop.reason, StopReason::Locked { .. } | StopReason::Stopped) {
                break;
            }
        }
        Ok(out)
    }
}
