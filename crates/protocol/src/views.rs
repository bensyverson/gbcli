//! Read-only views of the machine for `show` (design §12).

use serde::{Deserialize, Serialize};

/// CPU registers with the pairs and flags spelled out (design §12 `show`).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct RegistersView {
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
    pub af: u16,
    pub bc: u16,
    pub de: u16,
    pub hl: u16,
    pub flags: FlagsView,
    pub ime: bool,
    pub halted: bool,
    pub double_speed: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct FlagsView {
    pub z: bool,
    pub n: bool,
    pub h: bool,
    pub c: bool,
}

impl RegistersView {
    pub fn of(r: &gbcli_core::Registers) -> Self {
        let pair = |hi: u8, lo: u8| u16::from_be_bytes([hi, lo]);
        RegistersView {
            a: r.a,
            f: r.f,
            b: r.b,
            c: r.c,
            d: r.d,
            e: r.e,
            h: r.h,
            l: r.l,
            sp: r.sp,
            pc: r.pc,
            af: pair(r.a, r.f),
            bc: pair(r.b, r.c),
            de: pair(r.d, r.e),
            hl: pair(r.h, r.l),
            flags: FlagsView {
                z: r.f & 0x80 != 0,
                n: r.f & 0x40 != 0,
                h: r.f & 0x20 != 0,
                c: r.f & 0x10 != 0,
            },
            ime: r.ime,
            halted: r.halted,
            double_speed: r.double_speed,
        }
    }
}

/// The PPU as the core reports it.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PpuView {
    pub ly: u8,
    pub mode: PpuModeView,
    pub lcd_on: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub dot: Option<u16>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PpuModeView {
    Hblank,
    Vblank,
    OamScan,
    Drawing,
}

impl PpuModeView {
    /// The wire spelling.
    pub fn name(self) -> &'static str {
        match self {
            PpuModeView::Hblank => "hblank",
            PpuModeView::Vblank => "vblank",
            PpuModeView::OamScan => "oam_scan",
            PpuModeView::Drawing => "drawing",
        }
    }
}

impl From<gbcli_core::PpuMode> for PpuModeView {
    fn from(mode: gbcli_core::PpuMode) -> Self {
        match mode {
            gbcli_core::PpuMode::HBlank => PpuModeView::Hblank,
            gbcli_core::PpuMode::VBlank => PpuModeView::Vblank,
            gbcli_core::PpuMode::OamScan => PpuModeView::OamScan,
            gbcli_core::PpuMode::Drawing => PpuModeView::Drawing,
        }
    }
}

impl PpuView {
    pub fn of(p: &gbcli_core::PpuStatus) -> Self {
        PpuView {
            ly: p.ly,
            mode: p.mode.into(),
            lcd_on: p.lcd_on,
            dot: p.dot,
        }
    }
}
