//! Side-effect-free inspection over `&dyn Core`: CPU registers, regions by
//! name, bank and offset, decoded hardware registers, tile, tilemap, OAM and
//! palette data, and APU channel state.
//!
//! Everything here reads through the core's inspection methods only, never
//! `bus_read` (design §2.3), so it can't advance or perturb emulation.

pub mod hwreg;
mod hwreg_table;
pub mod regions;
pub mod video;

use gbcli_core::{ApuState, Core, CoreError, Registers};

/// The CPU flags in F.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Flags {
    pub z: bool,
    pub n: bool,
    pub h: bool,
    pub c: bool,
}

/// CPU registers with the pairs and flags spelled out.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct CpuView {
    /// Includes IME, halted and double speed.
    pub registers: Registers,
    pub af: u16,
    pub bc: u16,
    pub de: u16,
    pub hl: u16,
    pub flags: Flags,
}

pub fn cpu(core: &dyn Core) -> CpuView {
    let r = core.registers();
    let pair = |hi: u8, lo: u8| u16::from_be_bytes([hi, lo]);
    CpuView {
        registers: r,
        af: pair(r.a, r.f),
        bc: pair(r.b, r.c),
        de: pair(r.d, r.e),
        hl: pair(r.h, r.l),
        flags: Flags {
            z: r.f & 0x80 != 0,
            n: r.f & 0x40 != 0,
            h: r.f & 0x20 != 0,
            c: r.f & 0x10 != 0,
        },
    }
}

/// The sound channels and wave table, as the core reports them; note
/// [`ApuState::sync`], which says whether they are as of now.
pub fn apu_state(core: &dyn Core) -> Result<ApuState, CoreError> {
    core.apu()
}
