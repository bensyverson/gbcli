//! Errors and the capability table.

use std::fmt;

/// A capability a core may or may not have. `gbcli doctor --core` prints the table.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Feature {
    /// Stepping one PPU dot.
    DotStep,
    /// Stepping one M-cycle.
    MCycleStep,
    /// Reading the PPU's dot position within the line.
    PpuDot,
    /// Which layer (background, window, object) drew each pixel.
    PixelLayers,
    /// Decoding every hardware register without a bus read.
    SideEffectFreeIo,
    /// The unusable OAM area FEA0–FEFF.
    ExtraOam,
    /// APU internals beyond the registers and the channel state.
    ApuInternals,
    /// Per-channel volume, amplitude and period, and the wave table (`Core::apu`).
    ApuChannels,
    /// The write hook seeing OAM DMA and HDMA destination writes.
    WriteHookCoversDma,
    /// The execution hook seeing interrupt dispatch.
    ExecHookCoversInterrupts,
    /// ROM patches on MMM01 carts, which rotate their ROM in memory.
    Mmm01RomPatch,
    /// Event sinks (taps and the profiler) on a run.
    EventSinks,
}

impl Feature {
    /// The name the gap ledger and `gbcli doctor --core` spell.
    pub fn name(self) -> &'static str {
        match self {
            Feature::DotStep => "dot_step",
            Feature::MCycleStep => "mcycle_step",
            Feature::PpuDot => "ppu_dot",
            Feature::PixelLayers => "pixel_layers",
            Feature::SideEffectFreeIo => "side_effect_free_io",
            Feature::ExtraOam => "extra_oam",
            Feature::ApuInternals => "apu_internals",
            Feature::ApuChannels => "apu_channels",
            Feature::WriteHookCoversDma => "write_hook_covers_dma",
            Feature::ExecHookCoversInterrupts => "exec_hook_covers_interrupts",
            Feature::Mmm01RomPatch => "mmm01_rom_patch",
            Feature::EventSinks => "event_sinks",
        }
    }
}

/// Whether a core has a [`Feature`].
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Support {
    Yes,
    No { reason: &'static str },
}

/// Everything a core call can fail with.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum CoreError {
    /// The core can't do this; the engine records it in the gap ledger.
    Unsupported {
        feature: Feature,
        reason: &'static str,
    },
    /// An address, offset or length outside what exists.
    OutOfRange {
        what: &'static str,
        value: u64,
        limit: u64,
    },
    /// The request is refused in the current state; `reason` names the remedy where there is one.
    InvalidState { reason: String },
    /// The backend failed in a way the trait doesn't model.
    Backend { message: String },
}

impl fmt::Display for CoreError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            CoreError::Unsupported { feature, reason } => {
                write!(f, "unsupported ({feature:?}): {reason}")
            }
            CoreError::OutOfRange { what, value, limit } => {
                write!(f, "{what} {value:#x} out of range (limit {limit:#x})")
            }
            CoreError::InvalidState { reason } => write!(f, "refused: {reason}"),
            CoreError::Backend { message } => write!(f, "backend error: {message}"),
        }
    }
}

impl std::error::Error for CoreError {}
