//! The raster timeline on the wire (design §11): what `run --raster` reports.

use serde::{Deserialize, Serialize};

use crate::PpuModeView;

/// One IO-register write on a line.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct RasterWriteRow {
    /// The register's hardware.inc name (`rSCX`), or its
    /// address (`$FF4C`) when it has none.
    pub reg: String,
    pub value: u8,
    /// The PC of the instruction that wrote it.
    pub pc: u16,
    /// Emulated ticks (2^23 Hz; 912 per line) from the line's start; null
    /// when the core reported no start for the line. Not a dot.
    pub ticks_into_line: Option<u64>,
    /// The PPU's mode as the write landed, as a CPU read of STAT then would
    /// report it; null while the LCD is off, and where the core can't know
    /// it without running the PPU ahead (on SameBoy: late in a mode 3 it
    /// batched by an upper bound).
    #[serde(default)]
    pub mode: Option<PpuModeView>,
}

/// One line the run passed through that has writes on it, or a stretch with
/// the LCD off (`line` null).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct RasterLineRow {
    /// Counts from 0 at the run's first line; goes up where the line number
    /// wraps or the LCD turns on or off.
    pub frame: u32,
    /// The PPU's line (0-153), null with the LCD off.
    pub line: Option<u8>,
    /// Emulated ticks since power-on at the line's start; null when the core
    /// reported none (on SameBoy: line 153, and the first line after the LCD
    /// turns on).
    pub start: Option<u64>,
    pub writes: Vec<RasterWriteRow>,
}

/// What `run --raster` found. Only the lines with writes are listed; `lines`
/// counts every line the run passed through.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct RasterMade {
    /// Frames the timeline touched, counting a partial first and last.
    pub frames: u32,
    /// Lines the run passed through, with or without writes.
    pub lines: u64,
    pub rows: Vec<RasterLineRow>,
    /// What the core can't place, by feature name (`ppu_dot`,
    /// `ppu_mode_events`); each is also in the gap ledger.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub unsupported: Vec<String>,
}
