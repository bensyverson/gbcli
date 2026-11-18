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
//! halting; such a frame shows nothing, so it is never lag. A frame that
//! never halted and is followed by a frame that turns the LCD off before it
//! halts is LCD-off too: it is the screen change's wait for VBlank, polling
//! LY with the LCD on so as to turn it off there (Ben, 2026-11-17). So a
//! frame's verdict waits on the next frame's start: [`FrameSplit::cut`]
//! returns each frame one frame late, and [`FrameSplit::finish`] settles the
//! last by what the run saw of the next.

use gbcli_core::{CoreError, CpuStep, Event, EventKinds, EventSink, RunRequest, StopReason, Ticks};

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
    /// The CPU never halted, with the LCD on throughout, and the next frame
    /// didn't turn the LCD off before halting.
    pub lag: bool,
    /// The LCD was off for some of the frame, or the frame never halted and
    /// the next turned the LCD off before halting.
    pub lcd_off: bool,
}

/// A frame's edge: the frame number there, the LCD on or off, how many
/// times the machine had turned it off by then, and the time.
#[derive(Clone, Copy, Debug)]
struct Edge {
    frame: u64,
    on: bool,
    offs: u64,
    at: Ticks,
}

impl Edge {
    fn of(m: &Machine) -> Edge {
        Edge {
            frame: frame_number(m.core()),
            on: m.core().ppu().lcd_on,
            offs: m.lcd_offs(),
            at: m.core().now(),
        }
    }
}

/// The sink behind [`Machine::frame_budget`]: it adds each step to the frame
/// in progress until [`FrameSplit::cut`] ends it. Usable on any run whose
/// request takes a sink, cut at each vblank and finished at the run's end.
/// Its LCD-off verdicts need the machine's LCD-off log ([`Machine::lcd_offs`],
/// [`Machine::first_lcd_off_in`]), which a drive (or `frame_budget`) keeps.
pub struct FrameSplit {
    frame: FrameBudget,
    start: Edge,
    /// Core ticks since the frame began, summed from its steps.
    elapsed: u64,
    /// When the frame first halted, if it has.
    first_idle: Option<Ticks>,
    /// The last frame ended, whose verdict waits on the frame after it.
    pending: Option<FrameBudget>,
}

impl FrameSplit {
    /// A split whose first frame starts where `m` is now.
    pub fn starting(m: &Machine) -> FrameSplit {
        FrameSplit {
            frame: FrameBudget::default(),
            start: Edge::of(m),
            elapsed: 0,
            first_idle: None,
            pending: None,
        }
    }

    /// Ends the frame in progress where `m` is now, and returns the frame
    /// before it, numbered and its LCD-off and lag decided; `None` at the
    /// first cut. The next frame starts here.
    pub fn cut(&mut self, m: &Machine) -> Option<FrameBudget> {
        let (ended, turned_off_first) = self.end(m);
        let settled = self.pending.take().map(|f| settle(f, turned_off_first));
        self.pending = Some(ended);
        settled
    }

    /// Ends the run where `m` is: the last frame [`cut`](Self::cut) ended,
    /// settled by what the run saw of the frame after it (nothing, when the
    /// run stopped at a vblank, so a busy last frame stays lag), and the
    /// frame in progress, its lag undecided (it is only part of a frame).
    pub fn finish(mut self, m: &Machine) -> (Option<FrameBudget>, FrameBudget) {
        let (rest, turned_off_first) = self.end(m);
        let last = self.pending.take().map(|f| settle(f, turned_off_first));
        (last, rest)
    }

    /// Ends the frame in progress, with its own verdict, and whether it
    /// turned the LCD off before it first halted.
    fn end(&mut self, m: &Machine) -> (FrameBudget, bool) {
        let end = Edge::of(m);
        let mut frame = std::mem::take(&mut self.frame);
        frame.frame = self.start.frame;
        let turned_off = end.offs > self.start.offs;
        frame.lcd_off = !self.start.on || !end.on || turned_off;
        frame.lag = frame.idle_t == 0 && !frame.lcd_off;
        let turned_off_first = turned_off
            && m.first_lcd_off_in(self.start.frame)
                .is_some_and(|off| self.first_idle.is_none_or(|idle| off < idle));
        self.start = end;
        self.elapsed = 0;
        self.first_idle = None;
        (frame, turned_off_first)
    }
}

/// `frame`'s verdict, given whether the frame after it turned the LCD off
/// before halting: a lag frame that leads into that was waiting for VBlank
/// to turn the LCD off, so it is the screen change's.
fn settle(mut frame: FrameBudget, next_turned_off_first: bool) -> FrameBudget {
    if frame.lag && next_turned_off_first {
        frame.lag = false;
        frame.lcd_off = true;
    }
    frame
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
                CpuStep::Idle => {
                    self.first_idle
                        .get_or_insert(Ticks(self.start.at.0 + self.elapsed));
                    self.frame.idle_t += t;
                }
                CpuStep::Instruction { .. } | CpuStep::Interrupt => self.frame.active_t += t,
            }
            self.elapsed += u64::from(ticks);
        }
    }
}

impl Machine {
    /// Runs `frames` frames and splits each into active and idle time. The
    /// first counts from where the machine is; each ends at the end of the
    /// step its vblank fell in. The machine moves, as a run does, and no
    /// further: the last frame is settled without seeing the next, so a busy
    /// last frame reads lag even if the next would turn the LCD off.
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
        for _ in 0..frames {
            let mut req = RunRequest::new(gbcli_core::Budget::frames(1));
            req.stop_at_frame = true;
            req.sink = Some(&mut split);
            let stop = self.core_mut().run(&mut req)?;
            if let Ok(offs) = self.core_mut().take_lcd_offs() {
                self.record_lcd_offs(&offs);
            }
            out.extend(split.cut(self));
            if matches!(stop.reason, StopReason::Locked { .. } | StopReason::Stopped) {
                break;
            }
        }
        out.extend(split.finish(self).0);
        Ok(out)
    }
}
