//! CPU addresses read through direct access only: never a bus read, so
//! nothing a read does can change the machine (design §2.3, §8).

use gbcli_core::{Core, CoreError, Feature, IoReg, Region, canonical_addr};

use crate::sym::Area;

/// Where a CPU address lives, in the currently mapped banks.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Target {
    /// A byte of a region's backing store.
    Backing { region: Region, offset: usize },
    /// A hardware register, read decoded.
    Io(IoReg),
}

/// The bank mapped at `addr`: the ROM, VRAM, cart RAM or WRAM bank for the
/// switchable areas, 0 elsewhere.
pub fn bank_at(core: &dyn Core, addr: u16) -> u16 {
    let bank = |region| core.region(region).map_or(0, |v| v.bank);
    match Area::of(canonical_addr(addr)) {
        Area::RomX => bank(Region::Rom),
        Area::Vram => bank(Region::Vram),
        Area::Sram => bank(Region::Sram),
        Area::WramX => bank(Region::Wram),
        _ => 0,
    }
}

/// Resolves a CPU address to its backing byte or register.
pub fn target(core: &dyn Core, addr: u16) -> Result<Target, CoreError> {
    banked_target(bank_at(core, addr), addr)
}

/// Resolves an address in a given bank, whatever is mapped now. The bank
/// matters only in the switchable areas (ROMX, VRAM, cart RAM, WRAMX).
pub fn banked_target(bank: u16, addr: u16) -> Result<Target, CoreError> {
    let addr = canonical_addr(addr);
    let a = usize::from(addr);
    let banked = |region, base: usize, size: usize| Target::Backing {
        region,
        offset: usize::from(bank) * size + (a - base),
    };
    let flat = |region, base: usize| Target::Backing {
        region,
        offset: a - base,
    };
    Ok(match Area::of(addr) {
        Area::Rom0 => flat(Region::Rom, 0),
        Area::RomX => banked(Region::Rom, 0x4000, 0x4000),
        Area::Vram => banked(Region::Vram, 0x8000, 0x2000),
        Area::Sram => banked(Region::Sram, 0xA000, 0x2000),
        Area::Wram0 => flat(Region::Wram, 0xC000),
        Area::WramX => Target::Backing {
            region: Region::Wram,
            offset: usize::from(bank.max(1)) * 0x1000 + (a - 0xD000),
        },
        Area::Oam => flat(Region::Oam, 0xFE00),
        Area::Io => Target::Io(IoReg(addr as u8)),
        Area::Hram => flat(Region::Hram, 0xFF80),
        Area::Ie => flat(Region::Ie, 0xFFFF),
        Area::Unusable => {
            return Err(CoreError::Unsupported {
                feature: Feature::ExtraOam,
                reason: "FEA0-FEFF has no backing store to read without a bus read",
            });
        }
        Area::Echo => unreachable!("canonical_addr maps echo RAM to WRAM"),
    })
}

/// Reads a byte from a region's backing store.
pub fn read_backing(core: &dyn Core, region: Region, offset: usize) -> Result<u8, CoreError> {
    let view = core.region(region)?;
    view.bytes
        .get(offset)
        .copied()
        .ok_or(CoreError::OutOfRange {
            what: "region offset",
            value: offset as u64,
            limit: view.bytes.len() as u64,
        })
}

/// Reads the byte at a CPU address without side effects.
pub fn peek(core: &dyn Core, addr: u16) -> Result<u8, CoreError> {
    match target(core, addr)? {
        Target::Backing { region, offset } => read_backing(core, region, offset),
        Target::Io(reg) => core.io_register(reg),
    }
}

/// Whether the byte at `addr` has never been written since power-on.
/// `false` where the core doesn't track writes (ROM, VRAM, IO, tracking off).
pub fn never_written(core: &dyn Core, addr: u16) -> bool {
    let Ok(Target::Backing { region, offset }) = target(core, addr) else {
        return false;
    };
    if !matches!(region, Region::Wram | Region::Hram | Region::Sram) {
        return false;
    }
    core.written(region)
        .is_ok_and(|w| offset < w.len && w.never_written(offset))
}
