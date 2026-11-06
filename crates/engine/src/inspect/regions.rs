//! Regions by name, bank and offset (design §7, "Region + offset").

use gbcli_core::{Core, CoreError, Region};

pub const ALL_REGIONS: [Region; 11] = [
    Region::Rom,
    Region::Wram,
    Region::Sram,
    Region::Vram,
    Region::Hram,
    Region::Oam,
    Region::Io,
    Region::Ie,
    Region::BootRom,
    Region::BgPalette,
    Region::ObjPalette,
];

/// The region's name in the address grammar (`wram:0x1A0`).
pub fn region_name(region: Region) -> &'static str {
    match region {
        Region::Rom => "rom",
        Region::Wram => "wram",
        Region::Sram => "sram",
        Region::Vram => "vram",
        Region::Hram => "hram",
        Region::Oam => "oam",
        Region::Io => "io",
        Region::Ie => "ie",
        Region::BootRom => "bootrom",
        Region::BgPalette => "bgpal",
        Region::ObjPalette => "objpal",
    }
}

/// The region a name denotes, ignoring case.
pub fn parse_region(name: &str) -> Option<Region> {
    ALL_REGIONS
        .into_iter()
        .find(|&r| region_name(r).eq_ignore_ascii_case(name))
}

/// Bytes per bank, for the regions the CPU sees through a switchable window.
pub fn bank_size(region: Region) -> Option<usize> {
    match region {
        Region::Rom => Some(0x4000),
        Region::Sram | Region::Vram => Some(0x2000),
        Region::Wram => Some(0x1000),
        _ => None,
    }
}

/// A region's size, banking and the bank mapped now.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct RegionInfo {
    pub region: Region,
    /// Bytes in the whole backing store, every bank included.
    pub len: usize,
    /// 1 for an unbanked region.
    pub banks: u16,
    /// The bank in the switchable window (ROMX, A000, D000, VRAM); `None`
    /// for an unbanked region.
    pub mapped_bank: Option<u16>,
}

pub fn region_info(core: &dyn Core, region: Region) -> Result<RegionInfo, CoreError> {
    let view = core.region(region)?;
    let len = view.bytes.len();
    Ok(match bank_size(region) {
        Some(size) => RegionInfo {
            region,
            len,
            banks: len.div_ceil(size) as u16,
            mapped_bank: Some(view.bank),
        },
        None => RegionInfo {
            region,
            len,
            banks: 1,
            mapped_bank: None,
        },
    })
}

/// Bytes read from a region's backing store.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MemoryRead {
    pub region: Region,
    /// Offset into the whole backing store.
    pub offset: u32,
    /// The bank holding `offset`, for a banked region.
    pub bank: Option<u16>,
    pub bytes: Vec<u8>,
}

/// Reads `len` bytes at `offset`: into the whole store when `bank` is `None`,
/// or into that bank (staying inside it) when it is given.
pub fn read(
    core: &dyn Core,
    region: Region,
    bank: Option<u16>,
    offset: u32,
    len: usize,
) -> Result<MemoryRead, CoreError> {
    let bytes = core.region(region)?.bytes;
    let size = bank_size(region);
    let start = match (bank, size) {
        (None, _) => u64::from(offset),
        (Some(0), None) => u64::from(offset),
        (Some(b), None) => return Err(out_of_range("bank of an unbanked region", b.into(), 0)),
        (Some(b), Some(size)) => {
            let banks = bytes.len().div_ceil(size) as u64;
            if u64::from(b) >= banks {
                return Err(out_of_range("bank", b.into(), banks.saturating_sub(1)));
            }
            within("offset in bank", offset, len, size)?;
            u64::from(b) * size as u64 + u64::from(offset)
        }
    };
    let end = start + len as u64;
    if end > bytes.len() as u64 {
        let last = end.saturating_sub(1).max(start);
        return Err(out_of_range(
            "offset",
            last,
            bytes.len().saturating_sub(1) as u64,
        ));
    }
    Ok(MemoryRead {
        region,
        offset: start as u32,
        bank: size.map(|size| (start / size as u64) as u16),
        bytes: bytes[start as usize..end as usize].to_vec(),
    })
}

fn within(what: &'static str, offset: u32, len: usize, size: usize) -> Result<(), CoreError> {
    let end = u64::from(offset) + len as u64;
    if end > size as u64 {
        let last = end.saturating_sub(1).max(u64::from(offset));
        return Err(out_of_range(what, last, size as u64 - 1));
    }
    Ok(())
}

fn out_of_range(what: &'static str, value: u64, limit: u64) -> CoreError {
    CoreError::OutOfRange { what, value, limit }
}
