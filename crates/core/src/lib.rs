//! The emulator core trait gbcli's engine drives, designed for the ideal core
//! rather than SameBoy's shape (`project/2026-11-06-design.md` §2). A core's
//! limits show up as [`Support::No`] and [`CoreError::Unsupported`], never as
//! approximations.

mod apu;
mod audio;
mod battery;
mod display;
mod error;
mod filter;
mod region;
mod run;
mod state;
mod stop;
mod time;

pub use apu::{ApuChannel, ApuChannelState, ApuState, ApuSync};
pub use audio::{
    ApuWrite, ApuWriteEffect, AudioCapture, AudioSamples, SOUND_REGISTERS, StereoSample, WAVE_RAM,
    is_sound_addr,
};
pub use battery::SavClock;
pub use display::{ColorMode, LiveFrame, PixelBuf};
pub use error::{CoreError, Feature, Gap, Support};
pub use filter::{AddrFilter, BankSel, StoreFilter, canonical_addr};
pub use region::{
    FrameRef, FrameTag, Layer, LayerMap, Pixels, Region, RegionView, VblankKind, WrittenView,
};
pub use run::{
    Budget, BudgetKind, CpuStep, Event, EventKinds, EventSink, FirstStep, InStop, PpuMode,
    RunRequest, StepUnit, Usage,
};
pub use state::{
    BootRom, CoreConfig, CoreInfo, HaltWake, IoReg, JoypadBounce, Keys, Model, PpuStatus,
    Registers, Rendering, RtcMode, WriteTracking,
};
pub use stop::{Access, Hit, Hits, Stop, StopReason};
pub use time::Ticks;

/// An emulator core. Construction is not on the trait: each backend has a
/// `new(CoreConfig)` that does its determinism setup.
pub trait Core {
    /// Backend name, version and model.
    fn info(&self) -> CoreInfo;
    fn supports(&self, feature: Feature) -> Support;

    /// Runs until a budget limit, a filter hit or another stop. Always returns;
    /// nothing blocks waiting for input.
    fn run(&mut self, req: &mut RunRequest<'_>) -> Result<Stop, CoreError>;
    fn step(&mut self, unit: StepUnit) -> Result<Stop, CoreError>;

    /// Emulated time since power-on.
    fn now(&self) -> Ticks;

    // Side-effect-free inspection: never advances, syncs or perturbs anything.

    fn registers(&self) -> Registers;
    /// Whether the CPU is in STOP, which only a key in a selected joypad row
    /// ends. PC already points past STOP, as it does in HALT.
    fn in_stop(&self) -> bool;
    /// What ends the CPU's HALT, or `None` outside HALT (and in STOP)
    /// ([`Feature::HaltWake`]). The default is for a core without it, so
    /// adding this method broke no backend.
    fn halt_wake(&self) -> Result<Option<HaltWake>, CoreError> {
        Err(CoreError::Unsupported {
            feature: Feature::HaltWake,
            reason: "this core doesn't say what ends its HALT",
        })
    }
    /// A region's raw backing bytes and its current bank. For ROM, the bank
    /// is the one mapped at 4000-7FFF; see [`rom0_bank`](Core::rom0_bank).
    fn region(&self, region: Region) -> Result<RegionView<'_>, CoreError>;
    /// The ROM bank mapped at 0000-3FFF once the boot ROM is unmapped: 0,
    /// except on mappers that switch it (MBC1 in mode 1 on a large cart,
    /// MMM01).
    fn rom0_bank(&self) -> u16;
    /// A hardware register decoded as the CPU would read it, without a bus read;
    /// `Unsupported` where decoding would need one.
    fn io_register(&self, reg: IoReg) -> Result<u8, CoreError>;
    fn ppu(&self) -> PpuStatus;
    fn last_complete_frame(&self) -> Option<FrameRef<'_>>;
    /// The framebuffer as it is now, partly drawn ([`Feature::LiveFrame`]).
    /// The default is for a core without it, so adding this method broke no
    /// backend.
    fn live_frame(&self) -> Result<LiveFrame, CoreError> {
        Err(CoreError::Unsupported {
            feature: Feature::LiveFrame,
            reason: "this core doesn't expose its live framebuffer",
        })
    }
    /// How the PPU colors pixels now ([`Feature::DisplayColors`]).
    fn color_mode(&self) -> Result<ColorMode, CoreError> {
        Err(CoreError::Unsupported {
            feature: Feature::DisplayColors,
            reason: "this core doesn't report its color mode",
        })
    }
    /// The 0x00RRGGBB a frame of this core holds for a CGB RGB555 color, as
    /// its renderer converts it (color correction included)
    /// ([`Feature::DisplayColors`]). `Unsupported` on a core whose frames are
    /// shades.
    fn rgb555_color(&self, _color: u16) -> Result<u32, CoreError> {
        Err(CoreError::Unsupported {
            feature: Feature::DisplayColors,
            reason: "this core doesn't convert CGB colors",
        })
    }
    /// Which layer drew each pixel of the last complete frame
    /// ([`Feature::PixelLayers`]). Capture is off until
    /// [`set_pixel_layers`](Core::set_pixel_layers) turns it on. Only a frame
    /// captured whole gets a map: the map is not part of the state blob, so a
    /// load clears it, and a frame already under way when capture was turned
    /// on or a state was loaded has none (a frame loaded or enabled during
    /// vblank, before any of it is drawn, does).
    fn pixel_layers(&self) -> Result<LayerMap<'_>, CoreError>;
    /// Turns per-pixel layer capture on or off. Off by default; turning it on
    /// again keeps the current map. The default is for a core without it, so
    /// adding this method broke no backend.
    fn set_pixel_layers(&mut self, _on: bool) -> Result<(), CoreError> {
        Err(CoreError::Unsupported {
            feature: Feature::PixelLayers,
            reason: "this core doesn't capture pixel layers",
        })
    }
    /// The sound channels' state ([`Feature::ApuChannels`]). The default is for
    /// a core without it, so adding this method broke no backend.
    fn apu(&self) -> Result<ApuState, CoreError> {
        Err(CoreError::Unsupported {
            feature: Feature::ApuChannels,
            reason: "this core doesn't report APU channel state",
        })
    }
    /// Turns the sound-register write log on or off ([`Feature::ApuWriteLog`]).
    /// While on, every CPU write to $FF10-$FF26 or wave RAM is recorded with
    /// its time, the writing instruction and its [`ApuWriteEffect`]. Off by
    /// default; turning it off drops what wasn't taken. The default is for a
    /// core without it, so adding this method broke no backend.
    fn set_apu_write_log(&mut self, _on: bool) -> Result<(), CoreError> {
        Err(CoreError::Unsupported {
            feature: Feature::ApuWriteLog,
            reason: "this core doesn't log sound-register writes",
        })
    }
    /// The writes logged since the log was turned on or last taken, oldest
    /// first. A load drops them: they belong to the timeline it replaced.
    fn take_apu_writes(&mut self) -> Result<Vec<ApuWrite>, CoreError> {
        Err(CoreError::Unsupported {
            feature: Feature::ApuWriteLog,
            reason: "this core doesn't log sound-register writes",
        })
    }
    /// Starts rendering audio samples, at [`AudioCapture::SAMPLE_RATE`] with
    /// the given channels muted, or stops with `None` ([`Feature::AudioSamples`]).
    /// Starting (or changing the mutes) restarts the renderer from silence, as
    /// a fresh core has it, so the same state and the same run give the same
    /// samples whatever came before. Rendering never changes what the APU does.
    fn set_audio_capture(&mut self, _capture: Option<AudioCapture>) -> Result<(), CoreError> {
        Err(CoreError::Unsupported {
            feature: Feature::AudioSamples,
            reason: "this core doesn't render audio samples",
        })
    }
    /// The samples rendered since capture started or they were last taken. A
    /// load drops them and restarts the renderer from silence.
    fn take_audio(&mut self) -> Result<AudioSamples, CoreError> {
        Err(CoreError::Unsupported {
            feature: Feature::AudioSamples,
            reason: "this core doesn't render audio samples",
        })
    }
    /// The cart's battery-backed state as a `.sav`: the cart RAM, then a
    /// clock footer on a clocked cart (MBC3: BGB's and VBA's 48 bytes, the
    /// real and latched clocks and a host-time stamp). `None` when the cart
    /// has no battery ([`Feature::BatterySave`]). The default is for a core
    /// without it, so adding this method broke no backend.
    fn battery_sav(&mut self) -> Result<Option<Vec<u8>>, CoreError> {
        Err(CoreError::Unsupported {
            feature: Feature::BatterySave,
            reason: "this core doesn't export battery saves",
        })
    }
    /// Loads a `.sav` in [`battery_sav`](Core::battery_sav)'s format into the
    /// cart's RAM and clock. The footer's stamp is never compared with host
    /// time, so no time passes on import; [`advance_rtc`](Core::advance_rtc)
    /// does that explicitly. A size that fits no layout of this cart is
    /// refused, naming the sizes that do.
    fn load_battery_sav(&mut self, _sav: &[u8]) -> Result<SavClock, CoreError> {
        Err(CoreError::Unsupported {
            feature: Feature::BatterySave,
            reason: "this core doesn't import battery saves",
        })
    }
    /// Moves the cart's clock on by whole seconds at once, as if the machine
    /// had been off that long; a halted clock stays put, as on hardware.
    /// `InvalidState` on a cart without a clock.
    fn advance_rtc(&mut self, _seconds: u64) -> Result<(), CoreError> {
        Err(CoreError::Unsupported {
            feature: Feature::BatterySave,
            reason: "this core doesn't advance the cart clock",
        })
    }
    /// Never-written tracking for WRAM, HRAM or cart RAM (design §9).
    /// `InvalidState` when the core was built with tracking off.
    fn written(&self, region: Region) -> Result<WrittenView<'_>, CoreError>;

    // The three kinds of write, never conflated (design §4).

    /// As the CPU would write, with IO side effects.
    fn bus_write(&mut self, addr: u16, value: u8) -> Result<(), CoreError>;
    /// A real bus read, with its side effects (open bus, the OAM bug). Never
    /// used by inspection.
    fn bus_read(&mut self, addr: u16) -> Result<u8, CoreError>;
    /// Writes backing memory with no side effects. Refuses ROM (see
    /// `patch_rom`) and the boot ROM.
    fn poke(&mut self, region: Region, offset: u32, bytes: &[u8]) -> Result<(), CoreError>;
    /// Changes the ROM at a file offset. Refuses the header (0x100–0x14F).
    fn patch_rom(&mut self, offset: u32, bytes: &[u8]) -> Result<(), CoreError>;

    fn set_registers(&mut self, regs: &Registers);
    fn set_keys(&mut self, keys: Keys);

    /// The core's own state blob, including its emulated time; the engine wraps
    /// it in an envelope.
    ///
    /// It takes `&mut self` because saving may catch batched work up (SameBoy
    /// renders any samples its APU owes), which reaches state that a shared
    /// borrow could be reading. So no borrow into the core outlives a save:
    ///
    /// ```compile_fail,E0502
    /// fn f(core: &mut dyn gbcli_core::Core) {
    ///     let frame = core.last_complete_frame();
    ///     core.save();
    ///     let _number = frame.map(|f| f.tag.number);
    /// }
    /// ```
    ///
    /// while one that ends first is fine:
    ///
    /// ```
    /// fn f(core: &mut dyn gbcli_core::Core) {
    ///     let number = core.last_complete_frame().map(|f| f.tag.number);
    ///     core.save();
    ///     drop(number);
    /// }
    /// ```
    fn save(&mut self) -> Vec<u8>;
    fn load(&mut self, blob: &[u8]) -> Result<(), CoreError>;
}
