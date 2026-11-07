//! The raster timeline: the PPU's lines and the IO-register writes that land
//! on each, for checking mid-frame and mid-scanline effects. Design §11.
//!
//! Rows come from the core's line starts ([`Event::Scanline`]) and each
//! write's own line ([`Event::IoWrite`]), so a line the core reported no
//! start for (SameBoy skips line 153 and the first line after the LCD turns
//! on) still gets a row when something was written on it, with no start.

use gbcli_core::{
    Budget, Core, CoreError, Event, EventKinds, EventSink, Feature, Gap, IoReg, PpuMode,
    RunRequest, Support, Ticks,
};

use crate::machine::Machine;

/// One IO-register write on the timeline.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct RasterWrite {
    pub reg: IoReg,
    pub value: u8,
    pub at: Ticks,
    /// The PC of the instruction that wrote it.
    pub pc: u16,
    /// The dot within the line, where the core can place it.
    pub dot: Option<u16>,
}

/// A PPU mode change, where the core reports them.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ModeChange {
    pub mode: PpuMode,
    pub at: Ticks,
}

/// One line of one frame, or a stretch with the LCD off (`line: None`).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RasterLine {
    /// Counts from 0 at the timeline's first row and goes up each time the
    /// line number wraps, or the LCD comes on, so a partial first frame is
    /// frame 0.
    pub frame: u32,
    pub line: Option<u8>,
    /// When the line started, if the core reported its start.
    pub start: Option<Ticks>,
    pub modes: Vec<ModeChange>,
    pub writes: Vec<RasterWrite>,
}

impl RasterLine {
    /// Emulated ticks (2^23 Hz) from the line's start to `write`. Not a dot:
    /// it is measured from the core's own line-start event, which the core
    /// does not promise lands on dot 0.
    pub fn ticks_into_line(&self, write: &RasterWrite) -> Option<u64> {
        self.start.map(|start| write.at.0.saturating_sub(start.0))
    }
}

/// The lines a run passed through, and what the core couldn't report.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct RasterTimeline {
    pub lines: Vec<RasterLine>,
    /// One entry per missing feature ([`Feature::PpuDot`],
    /// [`Feature::PpuModeEvents`]), for the gap ledger.
    pub gaps: Vec<Gap>,
}

/// The sink that builds a [`RasterTimeline`]; public so any run that takes a
/// sink can record one.
#[derive(Debug, Default)]
pub struct Raster {
    modes: bool,
    timeline: RasterTimeline,
}

impl Raster {
    /// A sink asking `core` for what it can report; the rest become gaps.
    pub fn new(core: &dyn Core) -> Raster {
        let mut gaps = Vec::new();
        for feature in [Feature::PpuDot, Feature::PpuModeEvents] {
            if let Support::No { reason } = core.supports(feature) {
                gaps.push(Gap { feature, reason });
            }
        }
        let modes = !gaps.iter().any(|g| g.feature == Feature::PpuModeEvents);
        Raster {
            modes,
            timeline: RasterTimeline {
                lines: Vec::new(),
                gaps,
            },
        }
    }

    pub fn finish(self) -> RasterTimeline {
        self.timeline
    }

    /// A new row for `line`, numbered into the right frame.
    fn open(&mut self, line: Option<u8>, start: Option<Ticks>) -> &mut RasterLine {
        let lines = &mut self.timeline.lines;
        let frame = match lines.last() {
            None => 0,
            // A later line of the same lit frame.
            Some(last) if matches!((last.line, line), (Some(a), Some(b)) if b > a) => last.frame,
            Some(last) => last.frame + 1,
        };
        lines.push(RasterLine {
            frame,
            line,
            start,
            modes: Vec::new(),
            writes: Vec::new(),
        });
        lines.last_mut().expect("just pushed")
    }

    /// The row a write or mode change on `line` belongs to: the last row if
    /// it is that line, else a new row with no start.
    fn row_for(&mut self, line: Option<u8>) -> &mut RasterLine {
        if self.timeline.lines.last().is_some_and(|r| r.line == line) {
            return self.timeline.lines.last_mut().expect("checked");
        }
        self.open(line, None)
    }
}

impl EventSink for Raster {
    fn kinds(&self) -> EventKinds {
        EventKinds {
            exec: false,
            raster: true,
            ppu_modes: self.modes,
        }
    }

    fn event(&mut self, event: Event) {
        match event {
            Event::Scanline { ly, at } => {
                self.open(Some(ly), Some(at));
            }
            Event::IoWrite {
                reg,
                value,
                at,
                pc,
                line,
                dot,
            } => self.row_for(line).writes.push(RasterWrite {
                reg: IoReg(reg),
                value,
                at,
                pc,
                dot,
            }),
            Event::PpuMode { mode, ly, at } => {
                self.row_for(Some(ly)).modes.push(ModeChange { mode, at })
            }
            Event::Exec { .. } => {}
        }
    }
}

impl Machine {
    /// Runs `frames` frames recording the raster timeline. The machine
    /// moves, as a run does.
    pub fn raster(&mut self, frames: u32) -> Result<RasterTimeline, CoreError> {
        let mut sink = Raster::new(self.core());
        let mut req = RunRequest::new(Budget::frames(u64::from(frames)));
        req.sink = Some(&mut sink);
        self.core_mut().run(&mut req)?;
        Ok(sink.finish())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sink() -> Raster {
        Raster {
            modes: true,
            timeline: RasterTimeline::default(),
        }
    }

    fn start(ly: u8, at: u64) -> Event {
        Event::Scanline { ly, at: Ticks(at) }
    }

    fn write(line: Option<u8>, at: u64) -> Event {
        Event::IoWrite {
            reg: IoReg::SCX.0,
            value: 1,
            at: Ticks(at),
            pc: 0x150,
            line,
            dot: None,
        }
    }

    fn rows(events: &[Event]) -> Vec<(u32, Option<u8>, Option<u64>, usize)> {
        let mut s = sink();
        for &e in events {
            s.event(e);
        }
        s.finish()
            .lines
            .iter()
            .map(|r| (r.frame, r.line, r.start.map(|t| t.0), r.writes.len()))
            .collect()
    }

    #[test]
    fn a_write_joins_the_line_that_started_before_it() {
        let got = rows(&[start(5, 100), write(Some(5), 150), write(Some(5), 160)]);
        assert_eq!(got, vec![(0, Some(5), Some(100), 2)]);
    }

    #[test]
    fn a_write_on_a_line_with_no_start_gets_a_row_without_one() {
        let got = rows(&[start(152, 0), write(Some(153), 1000), start(0, 1824)]);
        assert_eq!(
            got,
            vec![
                (0, Some(152), Some(0), 0),
                (0, Some(153), None, 1),
                (1, Some(0), Some(1824), 0),
            ]
        );
    }

    #[test]
    fn writes_with_the_lcd_off_share_one_row() {
        let got = rows(&[write(None, 10), write(None, 20), start(1, 500)]);
        assert_eq!(got, vec![(0, None, None, 2), (1, Some(1), Some(500), 0)]);
    }

    #[test]
    fn a_frame_ends_when_the_line_number_wraps() {
        let got = rows(&[
            start(151, 0),
            start(152, 912),
            start(0, 2736),
            start(1, 3648),
        ]);
        let frames: Vec<u32> = got.iter().map(|r| r.0).collect();
        assert_eq!(frames, vec![0, 0, 1, 1]);
    }

    #[test]
    fn turning_the_lcd_off_mid_frame_ends_the_frame() {
        let got = rows(&[start(70, 0), write(None, 100), start(1, 900)]);
        let frames: Vec<u32> = got.iter().map(|r| r.0).collect();
        assert_eq!(frames, vec![0, 1, 2]);
    }

    #[test]
    fn mode_changes_join_their_line() {
        let mut s = sink();
        s.event(start(3, 0));
        s.event(Event::PpuMode {
            mode: PpuMode::Drawing,
            ly: 3,
            at: Ticks(160),
        });
        let t = s.finish();
        assert_eq!(
            t.lines[0].modes,
            vec![ModeChange {
                mode: PpuMode::Drawing,
                at: Ticks(160)
            }]
        );
    }

    #[test]
    fn ticks_into_line_count_from_the_start() {
        let mut s = sink();
        s.event(start(3, 1000));
        s.event(write(Some(3), 1300));
        let t = s.finish();
        assert_eq!(t.lines[0].ticks_into_line(&t.lines[0].writes[0]), Some(300));
    }
}
