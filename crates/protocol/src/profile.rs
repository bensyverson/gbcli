//! The cycle profiler on the wire (design §11): the `measure` verb, and what
//! `run --measure`, `--profile` and `--frame-budget` report.

use serde::{Deserialize, Serialize};

use crate::keys::KeyEvent;
use crate::request::is_false;

/// `measure`: cycles from each arrival at `from` to the next arrival at `to`,
/// on a copy of `at` or the head. The head never moves.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct MeasureArgs {
    /// A named checkpoint or `~N` to measure from; the head when absent.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub at: Option<String>,
    pub from: String,
    pub to: String,
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
    /// CPU T-cycles running instructions and dispatching interrupts.
    pub active_t: u64,
    /// CPU T-cycles in HALT or STOP.
    pub idle_t: u64,
    /// The CPU never halted in this frame: its wait for vblank came too late.
    /// Null for a partial frame, which never reached its vblank.
    pub lag: Option<bool>,
    /// The frame the run stopped in, from its last vblank to the stop.
    #[serde(default, skip_serializing_if = "is_false")]
    pub partial: bool,
}

/// What `run --frame-budget` found, one row per frame the run completed (the
/// first counts from where the run started), then the frame it stopped in,
/// if it stopped mid-frame, marked partial: the rows add up to the run's
/// cycles.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct FrameBudgetMade {
    pub frames: Vec<FrameBudgetRow>,
    /// The lag frames among the whole ones.
    pub lag: u64,
}

fn is_zero(n: &u64) -> bool {
    *n == 0
}
