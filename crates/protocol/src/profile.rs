//! The cycle profiler on the wire (design §11): the `measure` verb, and what
//! `run --measure`, `--profile` and `--frame-budget` report.

use serde::{Deserialize, Serialize};

use crate::keys::KeyEvent;
use crate::request::{Start, is_false};

/// `measure`: cycles from each arrival at `from` to the next arrival at `to`,
/// on a copy of `at` or the head. The head never moves.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct MeasureArgs {
    /// A named checkpoint or `~N` to measure from; the head when absent.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub at: Option<String>,
    /// The first address of each span (`measure FROM..TO`); `start` is where
    /// the copy begins.
    pub from: String,
    pub to: String,
    /// Begin the copy at the boot handoff or at power-on instead of the
    /// head, as `run`'s `from`; not with `at`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub start: Option<Start>,
    /// At least one; 1 when absent.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub iterations: Option<u32>,
    /// Fail with `predicate_false` (exit 1) when any iteration takes more
    /// T-cycles than this: an assertion for optimization loops.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max_t: Option<u64>,
    /// For the whole measurement, as a run's: 600 frames when none is given.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub frames: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub instructions: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ticks: Option<u64>,
    /// Key presses and releases at frame offsets from the measurement's
    /// start, as a run's `keys`: the copy runs under them.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub keys: Vec<KeyEvent>,
}

/// `run --measure FROM..TO`: one iteration, measured on a copy of the stop.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RunMeasure {
    pub from: String,
    pub to: String,
}

/// `run --profile`: cycles per routine over the run.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ProfileArgs {
    /// Only the routines with the most exclusive cycles; all when absent.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub top: Option<u32>,
}

/// A real number on the wire that is never NaN, so it compares as equal to
/// itself and the result types stay `Eq`.
#[derive(Clone, Copy, Debug, PartialEq, PartialOrd, Serialize, Deserialize)]
#[serde(transparent)]
pub struct Real(pub f64);

impl Eq for Real {}

/// The CPU clock a measurement ran at.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CpuSpeedName {
    Single,
    Double,
}

/// Minimum, maximum and mean over the iterations.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct CycleStats {
    pub min: u64,
    pub max: u64,
    pub mean: Real,
}

/// Minimum, maximum and mean share of a 70,224 T frame, as percentages.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PercentStats {
    pub min: Real,
    pub max: Real,
    pub mean: Real,
}

/// What a measurement found. T- and M-cycles are counted at `speed`; the
/// frame share is the same at either speed.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct MeasureMade {
    pub from: String,
    pub to: String,
    pub n: u64,
    pub speed: CpuSpeedName,
    pub t: CycleStats,
    pub m: CycleStats,
    pub frame_pct: PercentStats,
    /// The `max_t` asked for, when one was.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max_t: Option<u64>,
}

/// One routine's cost over a profile, in CPU T-cycles.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct RoutineRow {
    /// The `.sym` label at the entry, or `Label+$n`; absent without one.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    pub bank: u16,
    pub addr: u16,
    pub calls: u64,
    pub exclusive_t: u64,
    /// Of `exclusive_t`, the cycles the CPU spent halted or stopped in this
    /// routine's frame, a HALT reached by a tail jump (`jp WaitFrame`)
    /// included. 0 in a response from before the field.
    #[serde(default)]
    pub idle_t: u64,
    pub inclusive_t: u64,
}

/// What `run --profile` found: routines by exclusive cycles, most first.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProfileMade {
    pub routines: Vec<RoutineRow>,
    /// Routines left out by `top`.
    #[serde(default, skip_serializing_if = "is_zero")]
    pub omitted: u64,
    pub total_t: u64,
    /// Of `total_t`, the cycles the CPU spent halted or stopped.
    pub idle_t: u64,
    /// How often the shadow call stack had to follow the real one.
    pub unwound: u64,
    pub mismatched: u64,
    pub ret_jumps: u64,
    /// Calls still open when the run stopped.
    pub open: u64,
}

/// One frame of `run --frame-budget`, vblank to vblank.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct FrameBudgetRow {
    /// Which frame this is, counted as the stop's `frame` is: the vblanks
    /// since the run's time base when the frame began.
    pub frame: i64,
    /// CPU T-cycles running instructions and dispatching interrupts.
    pub active_t: u64,
    /// CPU T-cycles in HALT or STOP.
    pub idle_t: u64,
    /// The CPU never halted in this frame while the LCD was on: its wait for
    /// vblank came too late. False for an LCD-off frame, which shows nothing
    /// to lag; null for a partial frame, which the run saw only part of.
    pub lag: Option<bool>,
    /// The LCD was off for some of this frame: off at its start or its end,
    /// or turned off during it. A game does that on purpose, at startup and
    /// to change screens, so these frames are neither lag nor play.
    pub lcd_off: bool,
    /// A frame the run saw only part of: the one it started in, when it
    /// started mid-frame (from the start to the vblank), or the one it
    /// stopped in (from its last vblank to the stop).
    #[serde(default, skip_serializing_if = "is_false")]
    pub partial: bool,
}

/// What `run --frame-budget` found, one row per frame the run completed,
/// then the frame it stopped in, if it stopped mid-frame: the rows add up to
/// the run's cycles. The first counts from where the run started, and is
/// partial unless that was a frame's start (a vblank stop, the boot
/// handoff, power-on); the last, mid-frame one is always partial.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct FrameBudgetMade {
    pub frames: Vec<FrameBudgetRow>,
    /// The lag frames among the whole ones.
    pub lag: u64,
    /// The LCD-off frames among the whole ones, counted apart from lag.
    pub lcd_off: u64,
    /// The whole LCD-on frames' active T-cycles: fewest, most and mean.
    /// Absent when the run completed no such frame.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub active_t: Option<CycleStats>,
    /// The row index of the whole LCD-on frame with the most active cycles
    /// (the first, on a tie).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub busiest: Option<u64>,
}

/// A stretch of consecutive lag frames in a frame budget: the shape a
/// reader wants first, since a slow game lags in bursts (a screen being
/// drawn, a busy scene) rather than one frame at a time.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct LagRun {
    /// The row index of its first frame in `FrameBudgetMade::frames`.
    pub index: usize,
    /// Its first and last frames, counted as `FrameBudgetRow::frame` is.
    pub from: i64,
    pub to: i64,
    pub count: usize,
    /// The most active T-cycles of any of its frames.
    pub active_max: u64,
}

impl FrameBudgetMade {
    /// The lag frames grouped into runs of adjacent rows, in order. Any row
    /// that isn't lag (a frame that made its budget, an LCD-off frame, a
    /// partial one) ends a run.
    pub fn lag_runs(&self) -> Vec<LagRun> {
        let mut runs: Vec<LagRun> = Vec::new();
        let mut open = false;
        for (index, f) in self.frames.iter().enumerate() {
            if f.lag != Some(true) {
                open = false;
                continue;
            }
            match runs.last_mut() {
                Some(run) if open => {
                    run.to = f.frame;
                    run.count += 1;
                    run.active_max = run.active_max.max(f.active_t);
                }
                _ => runs.push(LagRun {
                    index,
                    from: f.frame,
                    to: f.frame,
                    count: 1,
                    active_max: f.active_t,
                }),
            }
            open = true;
        }
        runs
    }
}

fn is_zero(n: &u64) -> bool {
    *n == 0
}

#[cfg(test)]
mod tests {
    use super::{FrameBudgetMade, FrameBudgetRow, LagRun};

    fn frame(n: i64, active_t: u64, lag: Option<bool>) -> FrameBudgetRow {
        FrameBudgetRow {
            frame: n,
            active_t,
            idle_t: 70224u64.saturating_sub(active_t),
            lag,
            lcd_off: false,
            partial: lag.is_none(),
        }
    }

    fn budget(frames: Vec<FrameBudgetRow>) -> FrameBudgetMade {
        FrameBudgetMade {
            frames,
            lag: 0,
            lcd_off: 0,
            active_t: None,
            busiest: None,
        }
    }

    #[test]
    fn adjacent_lag_frames_make_one_run_with_their_busiest_frame() {
        let made = budget(vec![
            frame(10, 30000, Some(false)),
            frame(11, 70208, Some(true)),
            frame(12, 70232, Some(true)),
            frame(13, 70224, Some(true)),
            frame(14, 31000, Some(false)),
            frame(15, 70220, Some(true)),
        ]);
        assert_eq!(
            made.lag_runs(),
            vec![
                LagRun {
                    index: 1,
                    from: 11,
                    to: 13,
                    count: 3,
                    active_max: 70232
                },
                LagRun {
                    index: 5,
                    from: 15,
                    to: 15,
                    count: 1,
                    active_max: 70220
                },
            ]
        );
    }

    #[test]
    fn an_lcd_off_or_partial_frame_ends_a_run() {
        let off = FrameBudgetRow {
            lcd_off: true,
            ..frame(2, 70224, Some(false))
        };
        let made = budget(vec![
            frame(0, 1000, None),
            frame(1, 70224, Some(true)),
            off,
            frame(3, 70224, Some(true)),
            frame(4, 900, None),
        ]);
        let runs: Vec<(i64, i64)> = made.lag_runs().iter().map(|r| (r.from, r.to)).collect();
        assert_eq!(runs, vec![(1, 1), (3, 3)]);
        assert_eq!(budget(vec![frame(0, 1000, None)]).lag_runs(), vec![]);
    }
}
