//! Direct access: region views, decoded registers, pokes and ROM patches.

use gbcli_core::{CoreError, Feature, IoReg, Region, RegionView};
use sameboy_sys as sb;

use crate::{SameBoy, unsupported_err};

/// ROM header bytes SameBoy parses only at load, so patches there are refused.
const HEADER: std::ops::Range<u32> = 0x100..0x150;

fn access(region: Region) -> sb::GB_direct_access_t {
    match region {
        Region::Rom => sb::GB_DIRECT_ACCESS_ROM,
        Region::Wram => sb::GB_DIRECT_ACCESS_RAM,
        Region::Sram => sb::GB_DIRECT_ACCESS_CART_RAM,
        Region::Vram => sb::GB_DIRECT_ACCESS_VRAM,
        Region::Hram => sb::GB_DIRECT_ACCESS_HRAM,
        Region::Oam => sb::GB_DIRECT_ACCESS_OAM,
        Region::Io => sb::GB_DIRECT_ACCESS_IO,
        Region::Ie => sb::GB_DIRECT_ACCESS_IE,
        Region::BootRom => sb::GB_DIRECT_ACCESS_BOOTROM,
        Region::BgPalette => sb::GB_DIRECT_ACCESS_BGP,
        Region::ObjPalette => sb::GB_DIRECT_ACCESS_OBP,
    }
}

/// The raw backing store of a region: pointer, length and current bank.
fn backing(core: &SameBoy, region: Region) -> Result<(*mut u8, usize, u16), CoreError> {
    let palette = matches!(region, Region::BgPalette | Region::ObjPalette);
    // SAFETY: a plain query on a live instance.
    if palette && !unsafe { sb::GB_is_cgb(core.raw()) } {
        return Err(CoreError::InvalidState {
            reason: "CGB palette RAM doesn't exist on this model".into(),
        });
    }
    let (mut size, mut bank) = (0usize, 0u16);
    // SAFETY: GB_get_direct_access only reads the instance and fills the outputs.
    let ptr = unsafe { sb::GB_get_direct_access(core.raw(), access(region), &mut size, &mut bank) };
    if ptr.is_null() {
        size = 0;
    }
    Ok((ptr.cast(), size, bank))
}

pub(crate) fn region(core: &SameBoy, region: Region) -> Result<RegionView<'_>, CoreError> {
    let (ptr, size, bank) = backing(core, region)?;
    let bytes = if size == 0 {
        &[][..]
    } else {
        // SAFETY: SameBoy owns `size` bytes at `ptr` for the instance's life; the
        // borrow of `core` keeps every mutation (which needs `&mut`) out.
        unsafe { std::slice::from_raw_parts(ptr, size) }
    };
    Ok(RegionView { bytes, bank })
}

pub(crate) fn io_register(core: &SameBoy, reg: IoReg) -> Result<u8, CoreError> {
    if reg.0 >= 0x80 {
        return Err(CoreError::OutOfRange {
            what: "IO register",
            value: 0xFF00 | u64::from(reg.0),
            limit: 0xFF7F,
        });
    }
    // SAFETY: a side-effect-free peek on a live instance.
    let value = unsafe { sb::gbcli_io_peek(core.raw(), reg.0) };
    u8::try_from(value).map_err(|_| unsupported_err(Feature::SideEffectFreeIo))
}

fn check_range(what: &'static str, offset: u32, len: usize, size: usize) -> Result<(), CoreError> {
    let end = u64::from(offset) + len as u64;
    if end > size as u64 {
        return Err(CoreError::OutOfRange {
            what,
            value: end.saturating_sub(1).max(u64::from(offset)),
            limit: (size as u64).saturating_sub(1),
        });
    }
    Ok(())
}

pub(crate) fn poke(
    core: &mut SameBoy,
    region: Region,
    offset: u32,
    bytes: &[u8],
) -> Result<(), CoreError> {
    if region == Region::Rom {
        return Err(CoreError::InvalidState {
            reason: "pokes can't change ROM; use a ROM patch (--patch), which checkpoints record"
                .into(),
        });
    }
    let (ptr, size, _) = backing(core, region)?;
    check_range("poke offset", offset, bytes.len(), size)?;
    // SAFETY: in bounds by the check above; `&mut core` excludes other borrows.
    unsafe { std::ptr::copy_nonoverlapping(bytes.as_ptr(), ptr.add(offset as usize), bytes.len()) };
    if let Some(tracking) = &mut core.hooks_mut().tracking {
        tracking.mark_poke(region, offset as usize, bytes.len());
    }
    Ok(())
}

pub(crate) fn patch_rom(core: &mut SameBoy, offset: u32, bytes: &[u8]) -> Result<(), CoreError> {
    // SAFETY: a plain query on a live instance.
    if unsafe { sb::gbcli_is_mmm01(core.raw()) } {
        return Err(unsupported_err(Feature::Mmm01RomPatch));
    }
    check_range("ROM offset", offset, bytes.len(), core.rom_len)?;
    let end = offset + bytes.len() as u32;
    if offset < HEADER.end && end > HEADER.start {
        return Err(CoreError::InvalidState {
            reason: "the header (0x100-0x14F) is read only when the ROM loads; rebuild the ROM \
                     instead"
                .into(),
        });
    }
    let (ptr, size, _) = backing(core, Region::Rom)?;
    check_range("ROM offset", offset, bytes.len(), size)?;
    // SAFETY: in bounds by the checks above.
    unsafe { std::ptr::copy_nonoverlapping(bytes.as_ptr(), ptr.add(offset as usize), bytes.len()) };
    Ok(())
}
