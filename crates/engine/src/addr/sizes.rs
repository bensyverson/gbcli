//! How many bytes each region holds, which bounds a region offset.

use gbcli_core::{Core, CoreError, Region};

/// Whose sizes a [`RegionSizes`] holds, which an error names.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SizeBasis {
    /// Read from a running machine: its model and its cart.
    ThisMachine,
    /// The largest any model and cart has, for when there's no machine.
    AnyModel,
}

/// Bytes in each CPU-mapped region's whole backing store, every bank included.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct RegionSizes {
    pub rom: usize,
    pub wram: usize,
    /// 0 for a cart without RAM.
    pub sram: usize,
    pub vram: usize,
    pub hram: usize,
    pub oam: usize,
    pub io: usize,
    pub ie: usize,
    pub basis: SizeBasis,
}

impl RegionSizes {
    pub const ANY_MODEL: RegionSizes = RegionSizes {
        rom: 0x80_0000,
        wram: 0x8000,
        sram: 0x2_0000,
        vram: 0x4000,
        hram: 0x7F,
        oam: 0xA0,
        io: 0x80,
        ie: 1,
        basis: SizeBasis::AnyModel,
    };

    /// The sizes on `core`'s model and cart.
    pub fn of(core: &dyn Core) -> Result<RegionSizes, CoreError> {
        let len = |region| core.region(region).map(|view| view.bytes.len());
        Ok(RegionSizes {
            rom: len(Region::Rom)?,
            wram: len(Region::Wram)?,
            sram: len(Region::Sram)?,
            vram: len(Region::Vram)?,
            hram: len(Region::Hram)?,
            oam: len(Region::Oam)?,
            io: len(Region::Io)?,
            ie: len(Region::Ie)?,
            basis: SizeBasis::ThisMachine,
        })
    }

    /// The size of `region`, or `None` for a region with no CPU address.
    pub fn len(&self, region: Region) -> Option<usize> {
        Some(match region {
            Region::Rom => self.rom,
            Region::Wram => self.wram,
            Region::Sram => self.sram,
            Region::Vram => self.vram,
            Region::Hram => self.hram,
            Region::Oam => self.oam,
            Region::Io => self.io,
            Region::Ie => self.ie,
            Region::BootRom | Region::BgPalette | Region::ObjPalette => return None,
        })
    }
}
