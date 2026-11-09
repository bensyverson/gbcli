//! The per-frame budget: how much of each frame the CPU worked and how much
//! it waited in HALT, and which frames lagged.
//!
//! **A lag frame** is a frame, from one vblank to the next, in which the CPU
//! never entered HALT or STOP: the game was busy for all of it, so its wait
//! for vblank didn't come before the vblank. This is the definition for games
//! that wait in HALT, as most homebrew does. A game that busy-waits (polling
//! LY or a flag) never halts, so every one of its frames reads as lag; for
//! those, `measure` from the top of the main loop to its wait is the tool.
//!
//! **An LCD-off frame** is one the LCD was off for some of: off at its start
//! or its end, or turned off during it. Games turn the LCD off on purpose, at
//! startup and to change screens, and work through those frames without
//! halting; such a frame shows nothing, so it is never lag.

use gbcli_core::{CoreError, CpuStep, Event, EventKinds, EventSink, RunRequest, StopReason};

use crate::cond::eval::frame_number;
use crate::machine::Machine;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct FrameBudget {
    /// Which frame it is: the vblanks since power-on when it began, as a
    /// stop record counts them before taking its time base off.
    pub frame: u64,
    /// CPU T-cycles spent running instructions and dispatching interrupts.
    pub active_t: u64,
    /// CPU T-cycles spent in HALT or STOP.
    pub idle_t: u64,
    /// The CPU never halted, with the LCD on throughout.
    pub lag: bool,
    /// The LCD was off for some of the frame.
    pub lcd_off: bool,
}

/// A frame's edge: the frame number there, the LCD on or off, and how many
/// times the machine had turned it off by then.
#[derive(Clone, Copy, Debug)]
struct Edge {
    frame: u64,
    on: bool,
    offs: u64,
}

impl Edge {
    fn of(m: &Machine) -> Edge {
        Edge {
            frame: frame_number(m.core()),
            on: m.core().ppu().lcd_on,
            offs: m.lcd_offs(),
        }
    }
}

/// The sink behind [`Machine::frame_budget`]: it adds each step to the frame
/// in progress until [`FrameSplit::cut`] ends it. Usable on any run whose
/// request takes a sink, cut at each vblank. Its LCD-off verdict needs the
/// machine's LCD-off count, which a drive (or `frame_budget`) keeps.
pub struct FrameSplit {
    frame: FrameBudget,
    start: Edge,
}

impl FrameSplit {
    /// A split whose first frame starts where `m` is now.
    pub fn starting(m: &Machine) -> FrameSplit {
        FrameSplit {
            frame: FrameBudget::default(),
            start: Edge::of(m),
        }
    }

    /// Ends the frame in progress where `m` is now and returns it, numbered,
    /// LCD-off and lag decided; the next frame starts here.
    pub fn cut(&mut self, m: &Machine) -> FrameBudget {
        let end = Edge::of(m);
        let mut frame = std::mem::take(&mut self.frame);
        frame.frame = self.start.frame;
        frame.lcd_off = !self.start.on || !end.on || end.offs > self.start.offs;
        frame.lag = frame.idle_t == 0 && !frame.lcd_off;
        self.start = end;
        frame
    }
}

impl EventSink for FrameSplit {
    fn kinds(&self) -> EventKinds {
        EventKinds {
            exec: true,
            raster: false,
            ppu_modes: false,
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
        // The LCD-off log feeds the LCD-off verdict; a core without it
        // reports only the LCD's state at each vblank.
        let _ = self.core_mut().set_lcd_off_log(true);
        let out = self.split_frames(frames);
        let _ = self.core_mut().set_lcd_off_log(false);
        out
    }

    fn split_frames(&mut self, frames: u32) -> Result<Vec<FrameBudget>, CoreError> {
        let mut out = Vec::new();
        let mut split = FrameSplit::starting(self);
        while out.len() < frames as usize {
            let mut req = RunRequest::new(gbcli_core::Budget::frames(1));
            req.stop_at_frame = true;
            req.sink = Some(&mut split);
            let stop = self.core_mut().run(&mut req)?;
            if let Ok(offs) = self.core_mut().take_lcd_offs() {
                self.count_lcd_offs(offs.len());
            }
            out.push(split.cut(self));
            if matches!(stop.reason, StopReason::Locked { .. } | StopReason::Stopped) {
                break;
            }
        }
        Ok(out)
    }
}
