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

/// The region whose bank is switched at `addr`: ROM0 (on mappers that switch
/// it, and where the boot ROM overlays it until FF50 is written), ROMX, VRAM,
/// cart RAM and WRAMX. Everywhere else the mapping is fixed.
fn switchable(addr: u16) -> Option<Region> {
    match Area::of(canonical_addr(addr)) {
        Area::Rom0 | Area::RomX => Some(Region::Rom),
        Area::Vram => Some(Region::Vram),
        Area::Sram => Some(Region::Sram),
        Area::WramX => Some(Region::Wram),
        _ => None,
    }
}

/// The bank mapped at `addr`: the ROM0, ROMX, VRAM, cart RAM or WRAM bank for
/// the switchable areas, 0 elsewhere.
pub fn bank_at(core: &dyn Core, addr: u16) -> u16 {
    if Area::of(canonical_addr(addr)) == Area::Rom0 {
        return core.rom0_bank();
    }
    switchable(addr).map_or(0, |region| core.region(region).map_or(0, |v| v.bank))
}

/// The cart bank mapped at `addr`, as [`bank_at`], or `None` where the boot
/// ROM overlays it: the address then reads no cart byte, so no `.sym`
/// symbol names it.
pub fn cart_bank_at(core: &dyn Core, addr: u16) -> Option<u16> {
    match boot_rom_target(core, addr) {
        Some(_) => None,
        None => Some(bank_at(core, addr)),
    }
}

/// The boot ROM byte the CPU reads at `addr`, while the boot ROM is mapped
/// (rBANK bit 0 clear): 0000-00FF, and 0200-08FF where the boot ROM is that
/// long (CGB); 0100-01FF is always the cart header. Read through
/// `io_register` and the boot ROM's own size, so no model table is needed.
fn boot_rom_target(core: &dyn Core, addr: u16) -> Option<Target> {
    let offset = usize::from(addr);
    if (0x100..0x200).contains(&offset) || offset >= 0x900 {
        return None;
    }
    let mapped = core.io_register(IoReg::BANK).is_ok_and(|v| v & 1 == 0);
    let len = core.region(Region::BootRom).map_or(0, |v| v.bytes.len());
    (mapped && offset < len).then_some(Target::Backing {
        region: Region::BootRom,
        offset,
    })
}

/// The sizes cart memory aliases through: a mapper drives only as many
/// address lines as the memory has, so a bank or address past the end
/// reaches the byte its low bits name. Zero means no aliasing.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct CartSizes {
    pub rom: usize,
    pub sram: usize,
}

impl CartSizes {
    /// The sizes of `core`'s ROM and cart RAM.
    pub fn of(core: &dyn Core) -> Self {
        let len = |region| core.region(region).map_or(0, |v| v.bytes.len());
        CartSizes {
            rom: len(Region::Rom),
            sram: len(Region::Sram),
        }
    }

    /// `target` with a ROM or cart RAM offset aliased through the size.
    /// Other targets, and sizes that aren't a power of two, are unchanged.
    pub fn alias(self, target: Target) -> Target {
        let Target::Backing { region, offset } = target else {
            return target;
        };
        let size = match region {
            Region::Rom => self.rom,
            Region::Sram => self.sram,
            _ => return target,
        };
        if !size.is_power_of_two() {
            return target;
        }
        Target::Backing {
            region,
            offset: offset & (size - 1),
        }
    }
}

/// The bank a location names, as `core`'s hardware reaches it: a ROM or cart
/// RAM bank past the memory's size is the bank it aliases to (the bank of
/// [`target_in_bank`]'s byte). Other areas are unchanged.
pub fn effective_bank(core: &dyn Core, bank: u16, addr: u16) -> u16 {
    let size = match Area::of(canonical_addr(addr)) {
        Area::Rom0 | Area::RomX => 0x4000,
        Area::Sram => 0x2000,
        _ => return bank,
    };
    match target_in_bank(core, bank, addr) {
        Ok(Target::Backing { offset, .. }) => (offset / size) as u16,
        _ => bank,
    }
}

/// Resolves a CPU address to its backing byte or register: the boot ROM
/// where it is mapped over ROM0, else the mapped banks.
pub fn target(core: &dyn Core, addr: u16) -> Result<Target, CoreError> {
    if let Some(boot) = boot_rom_target(core, addr) {
        return Ok(boot);
    }
    match fixed_target(addr) {
        Some(target) => target,
        None => target_in_bank(core, bank_at(core, addr), addr),
    }
}

/// Resolves an address in a given bank as `core`'s hardware would with that
/// bank selected: [`banked_target`], aliased through the cart's sizes.
pub fn target_in_bank(core: &dyn Core, bank: u16, addr: u16) -> Result<Target, CoreError> {
    Ok(CartSizes::of(core).alias(banked_target(bank, addr)?))
}

/// What a CPU address resolves to whatever is mapped, or `None` in a
/// switchable area, where it depends on the bank. A predicate's cadence is
/// derived from this, the evaluator's own mapping.
pub fn fixed_target(addr: u16) -> Option<Result<Target, CoreError>> {
    match switchable(addr) {
        Some(_) => None,
        None => Some(banked_target(0, addr)),
    }
}

/// Resolves an address in a given bank, whatever is mapped now. The bank
/// matters only in the switchable areas (ROM0, ROMX, VRAM, cart RAM, WRAMX).
/// Offsets are not aliased: see [`target_in_bank`].
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
        Area::Rom0 => banked(Region::Rom, 0, 0x4000),
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_fixed_target_is_the_same_in_every_bank_and_a_switchable_one_is_not() {
        for addr in 0..=u16::MAX {
            match fixed_target(addr) {
                Some(fixed) => {
                    for bank in [0, 1, 2, 7, 0x1FF] {
                        assert_eq!(banked_target(bank, addr), fixed, "{addr:#06x}");
                    }
                }
                None => assert_ne!(
                    banked_target(2, addr),
                    banked_target(3, addr),
                    "{addr:#06x}"
                ),
            }
        }
    }
}
