//! The A/B equivalence check: two machines (two builds of a ROM, or one ROM
//! with and without a patch) run from equivalent starting states under one
//! key timeline, compared at the end of every frame. The guardrail for
//! optimizing: a rewrite must leave the screen, the chosen RAM and the sound
//! register writes as they were. Design §11.

mod diff;
mod start;

use std::fmt;
use std::sync::atomic::AtomicBool;

use gbcli_core::{ApuWrite, Budget, CoreError};
use gbcli_protocol::{KeyEvent, Location, StepKind, StopReason, StopRecord};

use crate::addr::AddrError;
use crate::checkpoint::{Checkpoint, SaveMeta};
use crate::debug::{DebugError, RunOptions, Schedule};
use crate::machine::Machine;

pub use start::{StartField, StartMismatch};

/// Memory to compare: `len` bytes from `at`, in the §7 address grammar (a
/// symbol, `NN:AAAA`, a region form or a CPU address). Each side resolves
/// `at` through its own `.sym`, since an optimized build moves symbols.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RamRange {
    pub at: String,
    pub len: u16,
}

/// How the sound-register write logs are compared.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum AudioCompare {
    /// Not compared.
    Off,
    /// Each frame's writes as a sequence of (register, value, effect). When
    /// in the frame a write lands is ignored: an optimization moves every
    /// write, and a frame's writes are heard together.
    #[default]
    Writes,
    /// As `Writes`, and each write's emulated time must match too.
    Timed,
}

/// Whether the completed frame's pixels are compared.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum ScreenCompare {
    #[default]
    On,
    /// Not compared: for builds whose timing differs by design, where only
    /// timing-free state (RAM, the sound writes) can match.
    Off,
}

/// What to run and compare.
#[derive(Clone, Debug)]
pub struct EquivSpec<'a> {
    /// Frames to compare, each ended by a vblank.
    pub frames: u64,
    /// The input timeline both machines get, as `run --keys` applies it.
    pub keys: &'a [KeyEvent],
    pub screen: ScreenCompare,
    pub ram: &'a [RamRange],
    pub audio: AudioCompare,
    /// Set from elsewhere to stop the check: a user break or wall guard.
    pub interrupt: Option<&'a AtomicBool>,
}

impl EquivSpec<'_> {
    pub fn new(frames: u64) -> Self {
        EquivSpec {
            frames,
            keys: &[],
            screen: ScreenCompare::default(),
            ram: &[],
            audio: AudioCompare::default(),
            interrupt: None,
        }
    }
}

/// One of the two machines.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Side {
    A,
    B,
}

/// The check's answer.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Verdict {
    /// Nothing compared differed.
    Same {
        /// Frames compared.
        frames: u64,
        /// Set when both machines stopped the same way (HALT forever, an
        /// illegal opcode, STOP) before the last frame, so no more frames
        /// could be compared.
        ended: Option<StopReason>,
    },
    Diverged(Box<Divergence>),
}

/// The first frame at whose end something differed.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Divergence {
    /// Frames since the start, counting this one (1 is the first vblank).
    pub frame: u64,
    /// Emulated ticks (2^23 Hz) since the start, at side A's stop.
    pub ticks: u64,
    /// Everything that differed in this frame: the stops, then the pixels,
    /// then each RAM range, then the sound-register writes.
    pub differences: Vec<Difference>,
    /// Set when the divergent frame ended at a different time since the
    /// start on each side: their frames are offset, so the screens are drawn
    /// at different moments (a start-up of another length, or the LCD turned
    /// off and on at another time).
    pub frame_offset: Option<FrameOffset>,
    pub a: SideState,
    pub b: SideState,
}

/// How much sooner one side's divergent frame ended, counted from each
/// side's start.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct FrameOffset {
    /// The side whose frame ended first.
    pub earlier: Side,
    /// In 2^23 Hz ticks.
    pub ticks: u64,
    /// In CPU T-cycles at side A's speed, as the stop rows' `t` counts.
    pub t: u64,
}

/// One machine where the check stopped, with a checkpoint to restore it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SideState {
    pub stop: StopRecord,
    pub checkpoint: Checkpoint,
}

/// One thing that differed in the divergent frame.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Difference {
    /// The frame ended differently: one machine stopped otherwise.
    Stop { a: StopReason, b: StopReason },
    /// The completed frame's pixels: the first differing one in row-major
    /// order (shades 0-3 on DMG, 0x00RRGGBB on color models), and how many
    /// differ.
    Pixels {
        x: u32,
        y: u32,
        a: u32,
        b: u32,
        count: usize,
    },
    /// A RAM range: its first differing byte, where each side has it, and
    /// how many bytes of the range differ.
    Ram {
        range: String,
        offset: u16,
        a_at: Location,
        b_at: Location,
        a: u8,
        b: u8,
        count: usize,
        /// The sides on which that first byte has never been written since
        /// power-on, so it holds a random power-on value: the program hasn't
        /// initialized it yet, which is no fault of the rewrite.
        never_written: Vec<Side>,
    },
    /// The frame's sound-register writes, from the first that differs; `None`
    /// where one side wrote fewer.
    Apu {
        index: usize,
        a: Option<ApuWrite>,
        b: Option<ApuWrite>,
    },
}

/// Why the check couldn't run to an answer.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum EquivError {
    /// The screen, RAM and sound compares are all off.
    NothingCompared,
    /// The two machines don't start in equivalent states.
    Start(StartMismatch),
    /// A RAM range doesn't resolve, or can't be read, on one side.
    Range {
        side: Side,
        at: String,
        error: AddrError,
    },
    Run {
        side: Side,
        error: DebugError,
    },
    Core {
        side: Side,
        error: CoreError,
    },
    /// The interrupt flag was set; `frames` were compared without a difference.
    Interrupted {
        frames: u64,
    },
}

impl fmt::Display for EquivError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            EquivError::NothingCompared => {
                write!(
                    f,
                    "nothing to compare: the screen, RAM and sound are all off"
                )
            }
            EquivError::Start(m) => write!(f, "{m}"),
            EquivError::Range { side, at, error } => write!(f, "{side:?}: range {at}: {error}"),
            EquivError::Run { side, error } => write!(f, "{side:?}: {error}"),
            EquivError::Core { side, error } => write!(f, "{side:?}: {error}"),
            EquivError::Interrupted { frames } => {
                write!(f, "interrupted after {frames} equivalent frames")
            }
        }
    }
}

impl std::error::Error for EquivError {}

/// Runs `a` and `b` side by side and reports the first frame where they
/// differ. Works on copies, so neither machine runs (copying saves each, which
/// takes `&mut`); the copies ignore both machines' breakpoints and watches.
pub fn compare(
    a: &mut Machine,
    b: &mut Machine,
    spec: &EquivSpec<'_>,
) -> Result<Verdict, EquivError> {
    if spec.screen == ScreenCompare::Off && spec.ram.is_empty() && spec.audio == AudioCompare::Off {
        return Err(EquivError::NothingCompared);
    }
    start::check(a, b).map_err(EquivError::Start)?;
    let ranges = spec
        .ram
        .iter()
        .map(|r| diff::resolve(r, a, b))
        .collect::<Result<Vec<_>, _>>()?;
    let mut a = Run::new(Side::A, a, spec)?;
    let mut b = Run::new(Side::B, b, spec)?;
    for frame in 1..=spec.frames {
        let (stop_a, stop_b) = (a.frame(spec)?, b.frame(spec)?);
        if [&stop_a, &stop_b]
            .iter()
            .any(|s| s.reason == StopReason::UserBreak)
        {
            return Err(EquivError::Interrupted { frames: frame - 1 });
        }
        let mut differences = Vec::new();
        if stop_a.reason != stop_b.reason {
            differences.push(Difference::Stop {
                a: stop_a.reason.clone(),
                b: stop_b.reason.clone(),
            });
        }
        if spec.screen == ScreenCompare::On {
            differences.extend(diff::pixels(a.m.core(), b.m.core()));
        }
        for range in &ranges {
            differences.extend(diff::ram(range, a.m.core(), b.m.core())?);
        }
        if spec.audio != AudioCompare::Off {
            let (wa, wb) = (a.writes()?, b.writes()?);
            differences.extend(diff::apu(&wa, &wb, spec.audio, (a.start, b.start)));
        }
        if !differences.is_empty() {
            return Ok(Verdict::Diverged(Box::new(Divergence {
                frame,
                ticks: a.elapsed(),
                frame_offset: frame_offset(&a, &b),
                differences,
                a: a.state(stop_a),
                b: b.state(stop_b),
            })));
        }
        if stop_a.reason != FRAME_DONE {
            return Ok(Verdict::Same {
                frames: frame - 1,
                ended: Some(stop_a.reason),
            });
        }
        a.keys.apply_due(&mut a.m, frame);
        b.keys.apply_due(&mut b.m, frame);
    }
    Ok(Verdict::Same {
        frames: spec.frames,
        ended: None,
    })
}

/// How far apart the two sides' frames ended, counted from each side's start;
/// `None` when they ended together.
fn frame_offset(a: &Run, b: &Run) -> Option<FrameOffset> {
    let (ta, tb) = (a.elapsed(), b.elapsed());
    let (earlier, ticks) = match ta.cmp(&tb) {
        std::cmp::Ordering::Equal => return None,
        std::cmp::Ordering::Less => (Side::A, tb - ta),
        std::cmp::Ordering::Greater => (Side::B, ta - tb),
    };
    let per_t = if a.m.core().registers().double_speed {
        1
    } else {
        2
    };
    Some(FrameOffset {
        earlier,
        ticks,
        t: ticks / per_t,
    })
}

/// How every compared frame ends.
const FRAME_DONE: StopReason = StopReason::StepDone {
    unit: StepKind::Frame,
};

/// One side under way: its own copy of the machine and key schedule.
struct Run {
    side: Side,
    m: Machine,
    keys: Schedule,
    start: u64,
}

impl Run {
    fn new(side: Side, m: &mut Machine, spec: &EquivSpec<'_>) -> Result<Run, EquivError> {
        let core_error = |error| EquivError::Core { side, error };
        let mut m = m.try_clone().map_err(core_error)?;
        let debugger = m.debugger_config_mut();
        debugger.breakpoints.clear();
        debugger.watches.clear();
        if spec.audio != AudioCompare::Off {
            m.core_mut().set_apu_write_log(true).map_err(core_error)?;
        }
        let mut keys = Schedule::new(spec.keys);
        keys.apply_due(&mut m, 0);
        Ok(Run {
            side,
            start: m.core().now().0,
            m,
            keys,
        })
    }

    /// Runs to the end of the next frame. The budget is two frames, so a frame
    /// that ends is never mistaken for one that ran out.
    fn frame(&mut self, spec: &EquivSpec<'_>) -> Result<StopRecord, EquivError> {
        let mut opts = RunOptions::new(Budget::frames(2));
        opts.interrupt = spec.interrupt;
        let outcome = self
            .m
            .step(StepKind::Frame, &opts)
            .map_err(|error| EquivError::Run {
                side: self.side,
                error,
            })?;
        Ok(outcome.stop)
    }

    /// Ticks since this side's start.
    fn elapsed(&self) -> u64 {
        self.m.core().now().0 - self.start
    }

    fn writes(&mut self) -> Result<Vec<ApuWrite>, EquivError> {
        self.m
            .core_mut()
            .take_apu_writes()
            .map_err(|error| EquivError::Core {
                side: self.side,
                error,
            })
    }

    fn state(&mut self, stop: StopRecord) -> SideState {
        SideState {
            stop,
            checkpoint: self.m.capture(SaveMeta {
                created_by: Some("equiv".into()),
                ..SaveMeta::default()
            }),
        }
    }
}
