//! CPU, PPU and input state, and what a core is built from.

use crate::run::PpuMode;

/// CPU registers plus the flags that are part of CPU state.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Registers {
    pub a: u8,
    pub f: u8,
    pub b: u8,
    pub c: u8,
    pub d: u8,
    pub e: u8,
    pub h: u8,
    pub l: u8,
    pub sp: u16,
    pub pc: u16,
    pub ime: bool,
    pub halted: bool,
    /// Read-only: `set_registers` ignores it (a speed switch is a KEY1 write plus STOP).
    pub double_speed: bool,
}

/// Held joypad buttons, one bit each.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct Keys(pub u8);

impl Keys {
    pub const NONE: Keys = Keys(0);
    pub const RIGHT: Keys = Keys(1 << 0);
    pub const LEFT: Keys = Keys(1 << 1);
    pub const UP: Keys = Keys(1 << 2);
    pub const DOWN: Keys = Keys(1 << 3);
    pub const A: Keys = Keys(1 << 4);
    pub const B: Keys = Keys(1 << 5);
    pub const SELECT: Keys = Keys(1 << 6);
    pub const START: Keys = Keys(1 << 7);
}

impl std::ops::BitOr for Keys {
    type Output = Keys;
    fn bitor(self, rhs: Keys) -> Keys {
        Keys(self.0 | rhs.0)
    }
}

/// The PPU as the CPU sees it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PpuStatus {
    pub ly: u8,
    pub mode: PpuMode,
    /// The dot within the line, if the core can report it ([`Feature::PpuDot`](crate::Feature::PpuDot)).
    pub dot: Option<u16>,
    pub lcd_on: bool,
}

/// A hardware register in FF00–FF7F, by its low address byte.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct IoReg(pub u8);

impl IoReg {
    pub const JOYP: IoReg = IoReg(0x00);
    pub const SB: IoReg = IoReg(0x01);
    pub const SC: IoReg = IoReg(0x02);
    pub const DIV: IoReg = IoReg(0x04);
    pub const TIMA: IoReg = IoReg(0x05);
    pub const TMA: IoReg = IoReg(0x06);
    pub const TAC: IoReg = IoReg(0x07);
    pub const IF: IoReg = IoReg(0x0F);
    pub const NR10: IoReg = IoReg(0x10);
    pub const NR52: IoReg = IoReg(0x26);
    pub const LCDC: IoReg = IoReg(0x40);
    pub const STAT: IoReg = IoReg(0x41);
    pub const SCY: IoReg = IoReg(0x42);
    pub const SCX: IoReg = IoReg(0x43);
    pub const LY: IoReg = IoReg(0x44);
    pub const LYC: IoReg = IoReg(0x45);
    pub const DMA: IoReg = IoReg(0x46);
    pub const BGP: IoReg = IoReg(0x47);
    pub const OBP0: IoReg = IoReg(0x48);
    pub const OBP1: IoReg = IoReg(0x49);
    pub const WY: IoReg = IoReg(0x4A);
    pub const WX: IoReg = IoReg(0x4B);
    pub const KEY1: IoReg = IoReg(0x4D);
    pub const VBK: IoReg = IoReg(0x4F);
    pub const BANK: IoReg = IoReg(0x50);
    pub const SVBK: IoReg = IoReg(0x70);
}

/// Hardware models. SameBoy has only one DMG revision (B).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Model {
    DmgB,
    Mgb,
    Sgb,
    Sgb2,
    Cgb0,
    CgbA,
    CgbB,
    CgbC,
    CgbD,
    CgbE,
    Agb,
}

impl Model {
    /// Whether frames come out as raw DMG shades rather than RGB.
    pub fn has_dmg_shades(self) -> bool {
        matches!(self, Model::DmgB | Model::Mgb)
    }

    /// Whether this is a Super Game Boy model, whose frames include a border.
    pub fn is_sgb(self) -> bool {
        matches!(self, Model::Sgb | Model::Sgb2)
    }
}

/// Which boot ROM runs at power-on.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum BootRom {
    /// The backend's built-in open-source boot ROM for the model.
    BuiltIn,
    /// Caller-supplied bytes.
    Custom(Vec<u8>),
}

/// How the cartridge real-time clock advances.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RtcMode {
    /// From emulated cycles only; deterministic.
    Accurate,
}

/// Whether the PPU produces pixels.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Rendering {
    On,
    Off,
}

/// Joypad bounce emulation.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum JoypadBounce {
    Off,
    On,
}

/// Never-written tracking (design §9): one bit per byte of WRAM, HRAM and
/// cart RAM, set by every CPU write. Keeps the write hook armed.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum WriteTracking {
    On,
    Off,
}

/// What a core is built from. Determinism setup happens at construction, so a
/// core can't exist in a non-deterministic state (design §2).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CoreConfig {
    pub model: Model,
    pub rom: Vec<u8>,
    pub boot_rom: BootRom,
    /// Seeds the power-on RAM randomization.
    pub seed: u64,
    pub rtc: RtcMode,
    pub rendering: Rendering,
    pub joypad_bounce: JoypadBounce,
    pub write_tracking: WriteTracking,
}

impl CoreConfig {
    /// Defaults for everything but the model and ROM: built-in boot ROM, seed 0,
    /// accurate RTC, rendering on, bounce off, write tracking on.
    pub fn new(model: Model, rom: Vec<u8>) -> Self {
        CoreConfig {
            model,
            rom,
            boot_rom: BootRom::BuiltIn,
            seed: 0,
            rtc: RtcMode::Accurate,
            rendering: Rendering::On,
            joypad_bounce: JoypadBounce::Off,
            write_tracking: WriteTracking::On,
        }
    }
}

/// Which backend this is.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CoreInfo {
    pub backend: &'static str,
    /// The backend's version or commit.
    pub version: &'static str,
    pub model: Model,
}
