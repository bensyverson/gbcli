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
    /// A log of CPU writes to the sound registers and wave RAM
    /// (`Core::set_apu_write_log`).
    ApuWriteLog,
    /// The core's audio output as samples, with channels mutable
    /// (`Core::set_audio_capture`).
    AudioSamples,
    /// The write hook seeing OAM DMA and HDMA destination writes.
    WriteHookCoversDma,
    /// Store filters over cart RAM: every change to cart RAM is a CPU write
    /// the core places at its byte (no mapper-internal writes it can't see).
    CartRamWrites,
    /// The execution hook seeing interrupt dispatch.
    ExecHookCoversInterrupts,
    /// ROM patches on MMM01 carts, which rotate their ROM in memory.
    Mmm01RomPatch,
    /// Event sinks on a run, with [`Event::Exec`](crate::Event::Exec) for
    /// every CPU step (the profiler).
    EventSinks,
    /// Scanline and IO-write events on a run (the raster timeline).
    RasterEvents,
    /// PPU mode transitions as events on a run, each at the time it happened.
    PpuModeEvents,
    /// The live, partly drawn framebuffer (`Core::live_frame`).
    LiveFrame,
    /// The core's color mode and its RGB output for a CGB color
    /// (`Core::color_mode`, `Core::rgb555_color`).
    DisplayColors,
    /// What ends the CPU's HALT, telling the CGB speed switch's self-ending
    /// pause from a wait for an interrupt (`Core::halt_wake`).
    HaltWake,
    /// The cart's battery-backed RAM and clock as a `.sav`, both ways
    /// (`Core::battery_sav`, `Core::load_battery_sav`, `Core::advance_rtc`).
    BatterySave,
    /// A log of the LCD being turned off during a run, with the line it was
    /// on (`Core::set_lcd_off_log`).
    LcdOffLog,
    /// A flash-save cart: the CPU's command writes reach a modeled flash
    /// chip (`CoreConfig::flash`, `Core::flash_contents`).
    FlashCart,
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
            Feature::ApuWriteLog => "apu_write_log",
            Feature::LcdOffLog => "lcd_off_log",
            Feature::AudioSamples => "audio_samples",
            Feature::WriteHookCoversDma => "write_hook_covers_dma",
            Feature::CartRamWrites => "cart_ram_writes",
            Feature::ExecHookCoversInterrupts => "exec_hook_covers_interrupts",
            Feature::Mmm01RomPatch => "mmm01_rom_patch",
            Feature::EventSinks => "event_sinks",
            Feature::RasterEvents => "raster_events",
            Feature::PpuModeEvents => "ppu_mode_events",
            Feature::LiveFrame => "live_frame",
            Feature::DisplayColors => "display_colors",
            Feature::HaltWake => "halt_wake",
            Feature::BatterySave => "battery_save",
            Feature::FlashCart => "flash_cart",
        }
    }
}

/// Whether a core has a [`Feature`].
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Support {
    Yes,
    No { reason: &'static str },
}

/// A feature a core can't provide, and why: one gap-ledger entry.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Gap {
    pub feature: Feature,
    pub reason: &'static str,
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
