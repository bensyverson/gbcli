//! Region reads by name, bank and offset.

mod common;

use common::*;
use gbcli_core::{Core, CoreError, Region};
use gbcli_engine::inspect::regions::{
    ALL_REGIONS, bank_size, parse_region, read, region_info, region_name,
};

/// The CPU window a banked region's switchable bank appears in.
fn window(region: Region) -> u16 {
    match region {
        Region::Rom => 0x4000,
        Region::Sram => 0xA000,
        Region::Wram => 0xD000,
        Region::Vram => 0x8000,
        _ => unreachable!(),
    }
}

/// Maps `bank` of `region` into its CPU window with bus writes.
fn map_bank(core: &mut dyn Core, region: Region, bank: u16) {
    let (addr, value) = match region {
        Region::Rom => (0x2000, bank as u8),  // MBC5 ROM bank, low bits
        Region::Sram => (0x4000, bank as u8), // MBC5 RAM bank
        Region::Wram => (0xFF70, bank as u8), // SVBK
        Region::Vram => (0xFF4F, bank as u8), // VBK
        _ => unreachable!(),
    };
    core.bus_write(addr, value).unwrap();
}

#[test]
fn every_bank_of_rom_cart_ram_wram_and_vram_reads_what_the_cpu_sees_when_mapped() {
    let mut core = booted_cgb(BANKED);
    core.bus_write(0xFF40, 0x00).unwrap(); // LCD off, so VRAM is never blocked
    core.bus_write(0x0000, 0x0A).unwrap(); // enable cart RAM
    for region in [Region::Rom, Region::Sram, Region::Wram, Region::Vram] {
        let info = region_info(&core, region).unwrap();
        let size = bank_size(region).unwrap();
        let expected_banks = match region {
            Region::Rom | Region::Sram => 4,
            Region::Wram => 8,
            _ => 2,
        };
        assert_eq!(info.banks, expected_banks, "{region:?}");
        // WRAM bank 0 has its own window; SVBK 0 maps bank 1.
        let first = if region == Region::Wram { 1 } else { 0 };
        for bank in first..info.banks {
            // Distinct contents per bank, where pokes may write.
            if region != Region::Rom {
                let pattern: Vec<u8> = (0..size)
                    .map(|i| (i as u8) ^ (bank as u8).wrapping_mul(37))
                    .collect();
                core.poke(region, u32::from(bank) * size as u32, &pattern)
                    .unwrap();
            }
            map_bank(&mut core, region, bank);
            let got = read(&core, region, Some(bank), 0, size).unwrap();
            assert_eq!(got.bank, Some(bank));
            assert_eq!(got.offset, u32::from(bank) * size as u32);
            let base = window(region);
            let cpu: Vec<u8> = on_a_copy(&mut core, |core| {
                (0..size as u16)
                    .map(|i| core.bus_read(base + i).unwrap())
                    .collect()
            });
            assert!(
                got.bytes == cpu,
                "{region:?} bank {bank} differs from the CPU's view"
            );
        }
    }
    // WRAM bank 0 at C000.
    let got = read(&core, Region::Wram, Some(0), 0, 0x1000).unwrap();
    let cpu: Vec<u8> = on_a_copy(&mut core, |core| {
        (0..0x1000u16)
            .map(|i| core.bus_read(0xC000 + i).unwrap())
            .collect()
    });
    assert!(got.bytes == cpu, "WRAM bank 0 differs from the CPU's view");
}

#[test]
fn an_offset_without_a_bank_addresses_the_whole_store() {
    let core = booted_cgb(BANKED);
    let whole = core.region(Region::Vram).unwrap().bytes.to_vec();
    let got = read(&core, Region::Vram, None, 0x2010, 4).unwrap();
    assert_eq!(got.bytes, whole[0x2010..0x2014]);
    assert_eq!(got.bank, Some(1), "the bank holding the offset");
    let hram = read(&core, Region::Hram, None, 2, 3).unwrap();
    assert_eq!(hram.bank, None, "HRAM isn't banked");
}

#[test]
fn reads_past_a_bank_or_region_are_out_of_range() {
    let core = quick_dmg(BANKED);
    let over_bank = read(&core, Region::Rom, Some(1), 0x3FFF, 2).unwrap_err();
    assert!(
        matches!(over_bank, CoreError::OutOfRange { .. }),
        "{over_bank:?}"
    );
    let no_bank = read(&core, Region::Rom, Some(4), 0, 1).unwrap_err();
    assert!(
        matches!(no_bank, CoreError::OutOfRange { .. }),
        "{no_bank:?}"
    );
    let over_region = read(&core, Region::Oam, None, 0x9F, 2).unwrap_err();
    assert!(
        matches!(over_region, CoreError::OutOfRange { .. }),
        "{over_region:?}"
    );
    let unbanked = read(&core, Region::Hram, Some(1), 0, 1).unwrap_err();
    assert!(
        matches!(unbanked, CoreError::OutOfRange { .. }),
        "{unbanked:?}"
    );
}

#[test]
fn region_info_reports_the_mapped_bank() {
    let mut core = quick_dmg(BANKED);
    core.bus_write(0x2000, 3).unwrap();
    let info = region_info(&core, Region::Rom).unwrap();
    assert_eq!(
        (info.len, info.banks, info.mapped_bank),
        (0x10000, 4, Some(3))
    );
    let hram = region_info(&core, Region::Hram).unwrap();
    assert_eq!((hram.len, hram.banks, hram.mapped_bank), (0x7F, 1, None));
}

#[test]
fn region_names_round_trip_and_follow_the_address_grammar() {
    for region in ALL_REGIONS {
        assert_eq!(parse_region(region_name(region)), Some(region));
    }
    for name in ["rom", "wram", "sram", "vram", "oam", "io", "hram", "ie"] {
        assert!(parse_region(name).is_some(), "{name}");
    }
    assert_eq!(parse_region("VRAM"), Some(Region::Vram));
    assert_eq!(parse_region("nope"), None);
}

#[test]
fn region_sizes_come_from_the_model_and_the_cart() {
    use gbcli_engine::addr::{RegionSizes, SizeBasis};
    let dmg = RegionSizes::of(&quick_dmg(BUSY)).unwrap();
    assert_eq!(
        (dmg.wram, dmg.vram, dmg.hram, dmg.oam, dmg.io, dmg.ie),
        (0x2000, 0x2000, 0x7F, 0xA0, 0x80, 1)
    );
    assert_eq!((dmg.rom, dmg.sram), (BUSY.len(), 0));
    assert_eq!(dmg.basis, SizeBasis::ThisMachine);
    let cgb = RegionSizes::of(&booted_cgb(BANKED)).unwrap();
    assert_eq!(
        (cgb.rom, cgb.sram, cgb.wram, cgb.vram),
        (0x10000, 0x8000, 0x8000, 0x4000)
    );
}
