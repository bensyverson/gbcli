//! The must-have mappers through the core interface (leaf OU9rOL): ROM-only,
//! MBC5 (bank 0 selectable, the 9-bit bank, 128 KiB of RAM, the rumble bit),
//! MBC3 with its clock deterministic, and MBC1 beside them. Each cart is
//! synthetic (`common::cart`): every bank holds its own number, so a read at
//! $4000 names the bank mapped there. Mooneye's MBC1, MBC1M and MBC5 ROMs and
//! docboy's MBC3 ROMs run in the corpus baseline; see
//! `project/2026-11-06-sameboy-baseline.md`.

mod common;

use common::cart::*;
use common::run;
use gbcli_core::{Budget, Core, Region};

#[test]
fn rom_only_maps_bank_1_and_ignores_bank_writes() {
    let mut core = core(cart(ROM_ONLY, 2, NO_RAM));
    assert_eq!(tag_at(&mut core, 0x4000), 1);
    poke_bus(&mut core, 0x2000, 0);
    assert_eq!(tag_at(&mut core, 0x4000), 1);
    assert!(core.region(Region::Sram).unwrap().bytes.is_empty());
}

#[test]
fn mbc1_maps_bank_1_for_a_write_of_0() {
    let mut core = core(cart(MBC1, 8, NO_RAM));
    poke_bus(&mut core, 0x2000, 3);
    assert_eq!(tag_at(&mut core, 0x4000), 3);
    poke_bus(&mut core, 0x2000, 0);
    assert_eq!(tag_at(&mut core, 0x4000), 1);
}

#[test]
fn mbc5_maps_bank_0_at_4000_when_asked() {
    let mut core = core(cart(MBC5, 8, NO_RAM));
    poke_bus(&mut core, 0x2000, 0);
    assert_eq!(tag_at(&mut core, 0x4000), 0);
    assert_eq!(core.region(Region::Rom).unwrap().bank, 0);
}

#[test]
fn mbc5_selects_all_512_banks_with_the_ninth_bit() {
    let mut core = core(cart(MBC5, 512, NO_RAM));
    for bank in [0x001u16, 0x0FF, 0x100, 0x123, 0x1FF] {
        poke_bus(&mut core, 0x2000, bank as u8);
        poke_bus(&mut core, 0x3000, (bank >> 8) as u8);
        assert_eq!(tag_at(&mut core, 0x4000), bank, "bank {bank:#x}");
        assert_eq!(core.region(Region::Rom).unwrap().bank, bank);
    }
}

#[test]
fn mbc5_has_sixteen_8k_ram_banks_at_128k() {
    let mut core = core(cart(MBC5_RAM, 8, RAM_128K));
    poke_bus(&mut core, 0x0000, 0x0A);
    for bank in 0..16u8 {
        poke_bus(&mut core, 0x4000, bank);
        poke_bus(&mut core, 0xA000, 0x40 + bank);
    }
    let sram = core.region(Region::Sram).unwrap().bytes;
    assert_eq!(sram.len(), 128 * 1024);
    for bank in 0..16u8 {
        assert_eq!(sram[usize::from(bank) * 0x2000], 0x40 + bank, "bank {bank}");
    }
}

#[test]
fn the_rumble_bit_drives_the_motor_not_a_ram_bank() {
    // Bit 3 of $4000 selects RAM bank 9 on a plain MBC5, and only the motor
    // on a rumble cart, where it leaves bank 1.
    for (kind, bank) in [(MBC5_RAM, 9usize), (MBC5_RUMBLE_RAM, 1)] {
        let mut core = core(cart(kind, 8, RAM_128K));
        poke_bus(&mut core, 0x0000, 0x0A);
        poke_bus(&mut core, 0x4000, 0x09);
        poke_bus(&mut core, 0xA000, 0x5A);
        let sram = core.region(Region::Sram).unwrap().bytes;
        assert_eq!(sram[bank * 0x2000], 0x5A, "cart type {kind:#04x}");
        assert_eq!(sram.iter().filter(|&&b| b == 0x5A).count(), 1);
    }
}

#[test]
fn mbc3_maps_bank_1_for_0_and_reaches_bank_127() {
    let mut core = core(cart(MBC3, 128, NO_RAM));
    poke_bus(&mut core, 0x2000, 0x7F);
    assert_eq!(tag_at(&mut core, 0x4000), 0x7F);
    poke_bus(&mut core, 0x2000, 0);
    assert_eq!(tag_at(&mut core, 0x4000), 1);
}

#[test]
fn the_mbc3_clock_counts_emulated_seconds_the_same_on_every_run() {
    let clock_after = |frames| {
        let mut core = core(cart(MBC3_TIMER_RAM_BATTERY, 4, RAM_32K));
        run(&mut core, Budget::frames(frames));
        read_rtc(&mut core)
    };
    // 60 frames are just under a second of emulated time.
    let first = clock_after(130);
    assert_eq!(first, [2, 0, 0, 0, 0]);
    assert_eq!(clock_after(130), first);
}

#[test]
fn the_mbc3_clock_carries_into_minutes_hours_and_days() {
    let mut core = core(cart(MBC3_TIMER_RAM_BATTERY, 4, RAM_32K));
    set_rtc(&mut core, [59, 59, 23, 0xFF, 0x00], false);
    run(&mut core, Budget::frames(65));
    assert_eq!(read_rtc(&mut core), [0, 0, 0, 0x00, 0x01]);
}

// The two MBC3 clock gaps behind docboy's failing rows in the corpus
// baseline, pinned as SameBoy behaves so a change to either shows here.

#[test]
fn sameboy_latches_the_mbc3_clock_on_any_write_not_only_0_then_1() {
    // Hardware latches when 1 is written after 0 (docboy's mbc3_rtc_latch_*
    // rows: 0 then 2, 1 then 0, 1 then 1 must not latch).
    let mut core = core(cart(MBC3_TIMER_RAM_BATTERY, 4, RAM_32K));
    assert_eq!(read_rtc(&mut core)[0], 0);
    run(&mut core, Budget::frames(130));
    poke_bus(&mut core, 0x6000, 0x01); // 1 after 1
    poke_bus(&mut core, 0x4000, 0x08);
    assert_eq!(core.bus_read(0xA000).unwrap(), 2, "latched anyway");
}

#[test]
fn sameboy_counts_an_out_of_range_second_on_past_its_six_bits() {
    // Hardware wraps 63 to 0 without carrying, then carries at 60 as usual
    // (docboy's mbc3_rtc_write_out_of_range_tick): 61 seconds after 63 it
    // reads 0:01:01. SameBoy counts the register up from 63 and only masks
    // it on read, so it reads 0:00:60.
    let mut core = core(cart(MBC3_TIMER_RAM_BATTERY, 4, RAM_32K));
    set_rtc(&mut core, [63, 0, 0, 0, 0], false);
    core.advance_rtc(61).unwrap();
    assert_eq!(&read_rtc(&mut core)[..2], &[60, 0]);
}

#[test]
fn a_halted_mbc3_clock_stays_put() {
    let mut core = core(cart(MBC3_TIMER_RAM_BATTERY, 4, RAM_32K));
    set_rtc(&mut core, [10, 0, 0, 0, 0], true);
    run(&mut core, Budget::frames(130));
    assert_eq!(read_rtc(&mut core), [10, 0, 0, 0, 0x40]);
}
