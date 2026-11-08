//! The A/B equivalence check on the wire (design §11): side A is the
//! session's ROM, side B another build or the same ROM patched, both run from
//! equivalent starts under one key timeline and compared frame by frame.

use gbcli_core::ApuWriteEffect;
use serde::{Deserialize, Serialize};

use crate::audio::ApuWriteEffectDef;
use crate::checkpoint::Location;
use crate::keys::KeyEvent;
use crate::request::{Start, WriteArg};
use crate::response::CheckpointMade;
use crate::stop::{StopReason, StopRecord};

/// `equiv`: run both sides `frames` frames and report the first difference.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct EquivArgs {
    /// Side B; its ROM is the session's when it names none.
    #[serde(default)]
    pub b: EquivSide,
    /// A checkpoint or `~N` restored into both sides; else `from`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub at: Option<String>,
    /// Each side's own boot handoff (the default), or power-on.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub from: Option<Start>,
    pub frames: u64,
    /// The input both sides get, as `run --keys` applies it.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub keys: Vec<KeyEvent>,
    /// RAM to compare after every frame; each side resolves `at` through its
    /// own `.sym`.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub ram: Vec<RamRangeArg>,
    /// How the sound-register writes are compared; `writes` when absent.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub audio: Option<AudioCompareName>,
}

/// Side B: another build of the ROM, and writes (patches) applied after its
/// start state loads.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct EquivSide {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub rom: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sym: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub writes: Vec<WriteArg>,
}

/// `len` bytes from `at`, in the address grammar.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RamRangeArg {
    pub at: String,
    pub len: u16,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AudioCompareName {
    Off,
    /// Each frame's writes in order, ignoring when in the frame they land.
    #[default]
    Writes,
    /// As `writes`, and each write's time must match too.
    Timed,
}

/// What the check found.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct EquivMade {
    /// Frames compared without a difference.
    pub frames: u64,
    /// Both sides stopped the same way before the last frame.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ended: Option<StopReason>,
    /// The first frame that differed; absent when the sides are the same.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub divergence: Option<Box<DivergenceMade>>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct DivergenceMade {
    /// Frames since the start, counting this one (1 is the first vblank).
    pub frame: u64,
    /// Emulated ticks (2^23 Hz) since the start, at side A's stop.
    pub ticks: u64,
    pub differences: Vec<DifferenceMade>,
    pub a: SideMade,
    pub b: SideMade,
}

/// One side where the check stopped, saved as a named checkpoint.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct SideMade {
    pub stop: StopRecord,
    pub checkpoint: CheckpointMade,
}

/// One thing that differed in the divergent frame.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum DifferenceMade {
    Stop {
        a: StopReason,
        b: StopReason,
    },
    /// The first differing pixel (shades 0-3 on DMG, 0x00RRGGBB in color)
    /// and how many differ.
    Pixels {
        x: u32,
        y: u32,
        a: u32,
        b: u32,
        count: u64,
    },
    /// A range's first differing byte, where each side has it, and how many
    /// bytes differ.
    Ram {
        range: String,
        offset: u16,
        a_at: Location,
        b_at: Location,
        a: u8,
        b: u8,
        count: u64,
        /// The sides on which that first byte has never been written since
        /// power-on, so it holds a random power-on value.
        #[serde(default, skip_serializing_if = "Vec::is_empty")]
        never_written: Vec<SideName>,
    },
    /// The frame's sound-register writes from the first that differs; absent
    /// where one side wrote fewer.
    Apu {
        index: u64,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        a: Option<ApuWriteMade>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        b: Option<ApuWriteMade>,
    },
}

/// One of the two sides: A is the session's ROM, B the other build.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SideName {
    A,
    B,
}

/// A sound-register write: the core's `ApuWrite`, which has no serde.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ApuWriteMade {
    /// Since power-on, in 2^23 Hz ticks.
    pub ticks: u64,
    pub pc: u16,
    pub addr: u16,
    pub value: u8,
    #[serde(with = "ApuWriteEffectDef")]
    pub effect: ApuWriteEffect,
}
