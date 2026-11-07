//! The emulator core trait gbcli's engine drives, designed for the ideal core
//! rather than SameBoy's shape (`project/2026-11-06-design.md` §2). A core's
//! limits show up as [`Support::No`] and [`CoreError::Unsupported`], never as
//! approximations.

mod apu;
mod error;
mod filter;
mod region;
mod run;
mod state;
mod stop;
mod time;

pub use apu::{ApuChannel, ApuChannelState, ApuState, ApuSync};
pub use error::{CoreError, Feature, Support};
pub use filter::{AddrFilter, BankSel, StoreFilter, canonical_addr};
pub use region::{
    FrameRef, FrameTag, Layer, LayerMap, Pixels, Region, RegionView, VblankKind, WrittenView,
};
pub use run::{Budget, BudgetKind, Event, EventSink, PpuMode, RunRequest, StepUnit, Usage};
pub use state::{
    BootRom, CoreConfig, CoreInfo, IoReg, JoypadBounce, Keys, Model, PpuStatus, Registers,
    Rendering, RtcMode, WriteTracking,
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
    /// A region's raw backing bytes and its current bank.
    fn region(&self, region: Region) -> Result<RegionView<'_>, CoreError>;
    /// A hardware register decoded as the CPU would read it, without a bus read;
    /// `Unsupported` where decoding would need one.
    fn io_register(&self, reg: IoReg) -> Result<u8, CoreError>;
    fn ppu(&self) -> PpuStatus;
    fn last_complete_frame(&self) -> Option<FrameRef<'_>>;
    /// Which layer drew each pixel of the last complete frame
    /// ([`Feature::PixelLayers`]). Capture is off until
    /// [`set_pixel_layers`](Core::set_pixel_layers) turns it on; the map is not
    /// part of the state blob, so a load clears it until the next frame.
    fn pixel_layers(&self) -> Result<LayerMap<'_>, CoreError>;
    /// Turns per-pixel layer capture on or off. Off by default. The default is
    /// for a core without it, so adding this method broke no backend.
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
    /// Never-written tracking for WRAM, HRAM or cart RAM (design §9).
    /// `InvalidState` when the core was built with tracking off.
    fn written(&self, region: Region) -> Result<WrittenView<'_>, CoreError>;

    // The three kinds of write, never conflated (design §4).

    /// As the CPU would write, with IO side effects.
    fn bus_write(&mut self, addr: u16, value: u8) -> Result<(), CoreError>;
    /// A real bus read, with its side effects (open bus, the OAM bug). Never
    /// used by inspection.
    fn bus_read(&mut self, addr: u16) -> Result<u8, CoreError>;
    /// Writes backing memory with no side effects. Refuses ROM: see `patch_rom`.
    fn poke(&mut self, region: Region, offset: u32, bytes: &[u8]) -> Result<(), CoreError>;
    /// Changes the ROM at a file offset. Refuses the header (0x100–0x14F).
    fn patch_rom(&mut self, offset: u32, bytes: &[u8]) -> Result<(), CoreError>;

    fn set_registers(&mut self, regs: &Registers);
    fn set_keys(&mut self, keys: Keys);

    /// The core's own state blob, including its emulated time; the engine wraps
    /// it in an envelope.
    fn save(&self) -> Vec<u8>;
    fn load(&mut self, blob: &[u8]) -> Result<(), CoreError>;
}
