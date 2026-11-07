//! Battery saves through the core interface (design §6, the 2026-11-07
//! battery-save ruling): export in SameBoy's `.sav` format, import without
//! host time, and the clock advanced only on request.

mod common;

use common::cart::*;
use common::run;
use gbcli_core::{Budget, Core, CoreError, Region, SavClock};
use gbcli_sameboy::SameBoy;

const RAM_32K_BYTES: usize = 32 * 1024;
/// BGB's and VBA's MBC3 footer: real and latched clocks, a 64-bit stamp.
const FOOTER: usize = 48;

/// An MBC3 cart with a clock and 32 KiB of battery RAM, its RAM written with
/// a pattern and its clock set to 1d 02:03:04 and running.
fn clocked() -> SameBoy {
    let mut core = core(cart(MBC3_TIMER_RAM_BATTERY, 4, RAM_32K));
    let pattern: Vec<u8> = (0..RAM_32K_BYTES).map(|i| (i * 7 + 3) as u8).collect();
    core.poke(Region::Sram, 0, &pattern).unwrap();
    set_rtc(&mut core, [4, 3, 2, 1, 0], false);
    core
}

/// A footer with every clock register named and the stamp given.
fn footer(real: [u8; 5], latched: [u8; 5], stamp: u64) -> Vec<u8> {
    let mut out = Vec::new();
    for reg in real.iter().chain(latched.iter()) {
        out.extend_from_slice(&u32::from(*reg).to_le_bytes());
    }
    out.extend_from_slice(&stamp.to_le_bytes());
    out
}

#[test]
fn an_export_is_cart_ram_then_the_mbc3_clock_footer() {
    let mut core = clocked();
    let ram = core.region(Region::Sram).unwrap().bytes.to_vec();
    let sav = core.battery_sav().unwrap().expect("a battery cart");
    assert_eq!(sav.len(), RAM_32K_BYTES + FOOTER);
    assert_eq!(&sav[..RAM_32K_BYTES], ram.as_slice());
    let real: Vec<u8> = (0..5).map(|i| sav[RAM_32K_BYTES + 4 * i]).collect();
    assert_eq!(real, [4, 3, 2, 1, 0]);
    let stamp = u64::from_le_bytes(sav[sav.len() - 8..].try_into().unwrap());
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_secs();
    assert!(stamp.abs_diff(now) < 60, "the stamp is host time: {stamp}");
}

#[test]
fn a_cart_without_a_battery_has_no_sav() {
    let mut core = core(cart(MBC5_RAM, 8, RAM_32K));
    assert_eq!(core.battery_sav().unwrap(), None);
    let refused = core.load_battery_sav(&[0; RAM_32K_BYTES]).unwrap_err();
    assert!(
        matches!(&refused, CoreError::InvalidState { reason } if reason.contains("no battery")),
        "{refused:?}"
    );
}

#[test]
fn a_ram_only_sav_round_trips_on_a_cart_without_a_clock() {
    let mut a = core(cart(MBC5_RAM_BATTERY, 8, RAM_128K));
    a.poke(Region::Sram, 0x1_2345, &[1, 2, 3]).unwrap();
    let sav = a.battery_sav().unwrap().unwrap();
    assert_eq!(sav.len(), 128 * 1024);
    let mut b = core(cart(MBC5_RAM_BATTERY, 8, RAM_128K));
    assert_eq!(b.load_battery_sav(&sav).unwrap(), SavClock::NotClocked);
    assert_eq!(
        b.region(Region::Sram).unwrap().bytes,
        a.region(Region::Sram).unwrap().bytes
    );
}

#[test]
fn export_then_import_round_trips_ram_and_both_clocks() {
    let mut a = clocked();
    run(&mut a, Budget::frames(130));
    let clock = read_rtc(&mut a);
    assert_eq!(clock, [6, 3, 2, 1, 0]);
    let sav = a.battery_sav().unwrap().unwrap();

    let mut b = core(cart(MBC3_TIMER_RAM_BATTERY, 4, RAM_32K));
    assert_eq!(b.load_battery_sav(&sav).unwrap(), SavClock::Restored);
    assert_eq!(
        b.region(Region::Sram).unwrap().bytes,
        a.region(Region::Sram).unwrap().bytes
    );
    let again = b.battery_sav().unwrap().unwrap();
    // Everything but the stamp, which is host time at each export.
    assert_eq!(again[..again.len() - 8], sav[..sav.len() - 8]);
    assert_eq!(read_rtc(&mut b), clock);
}

#[test]
fn import_never_advances_the_clock_by_host_time() {
    // SameBoy's own loader would add the decades since this stamp.
    let mut sav = vec![0u8; RAM_32K_BYTES];
    sav.extend(footer([1, 2, 3, 4, 0], [9, 9, 9, 9, 0], 946_684_800));
    let mut core = core(cart(MBC3_TIMER_RAM_BATTERY, 4, RAM_32K));
    assert_eq!(core.load_battery_sav(&sav).unwrap(), SavClock::Restored);
    // The latched copy is the file's until the game latches again.
    poke_bus(&mut core, 0x0000, 0x0A);
    poke_bus(&mut core, 0x4000, 0x08);
    assert_eq!(core.bus_read(0xA000).unwrap(), 9);
    assert_eq!(read_rtc(&mut core), [1, 2, 3, 4, 0]);
}

#[test]
fn a_32_bit_vba_footer_is_read_too() {
    let mut sav = vec![0u8; RAM_32K_BYTES];
    let mut tail = footer([5, 6, 7, 8, 1], [0; 5], 0);
    tail.truncate(44);
    sav.extend(tail);
    let mut core = core(cart(MBC3_TIMER_RAM_BATTERY, 4, RAM_32K));
    assert_eq!(core.load_battery_sav(&sav).unwrap(), SavClock::Restored);
    assert_eq!(read_rtc(&mut core), [5, 6, 7, 8, 1]);
}

#[test]
fn imported_ram_counts_as_written_not_as_power_on_bytes() {
    let mut core = core(cart(MBC5_RAM_BATTERY, 8, RAM_32K));
    assert!(core.written(Region::Sram).unwrap().never_written(0x123));
    core.load_battery_sav(&[0x42; RAM_32K_BYTES]).unwrap();
    let written = core.written(Region::Sram).unwrap();
    assert!((0..RAM_32K_BYTES).all(|i| written.was_poked(i)));
}

#[test]
fn importing_twice_gives_identical_machines() {
    let sav = clocked().battery_sav().unwrap().unwrap();
    let mut states = (0..2).map(|_| {
        let mut core = core(cart(MBC3_TIMER_RAM_BATTERY, 4, RAM_32K));
        core.load_battery_sav(&sav).unwrap();
        run(&mut core, Budget::frames(3));
        core.save()
    });
    assert_eq!(states.next(), states.next());
}

#[test]
fn a_sav_without_its_clock_footer_loads_ram_and_flags_the_clock() {
    let ram: Vec<u8> = (0..RAM_32K_BYTES).map(|i| i as u8).collect();
    let mut core = core(cart(MBC3_TIMER_RAM_BATTERY, 4, RAM_32K));
    assert_eq!(
        core.load_battery_sav(&ram).unwrap(),
        SavClock::FooterMissing
    );
    assert_eq!(core.region(Region::Sram).unwrap().bytes, ram.as_slice());
    assert_eq!(read_rtc(&mut core)[4] & 0x80, 0x80, "the carry bit hints");
}

#[test]
fn a_sav_whose_size_fits_no_layout_is_refused_naming_the_sizes_that_do() {
    let mut core = core(cart(MBC3_TIMER_RAM_BATTERY, 4, RAM_32K));
    let refused = core.load_battery_sav(&[0; 1000]).unwrap_err();
    let CoreError::InvalidState { reason } = &refused else {
        panic!("{refused:?}")
    };
    for size in ["32768", "32812", "32816"] {
        assert!(reason.contains(size), "{reason}");
    }
    assert_eq!(
        core.region(Region::Sram).unwrap().bytes[0],
        0xFF,
        "unchanged"
    );
}

#[test]
fn advancing_the_clock_moves_it_by_whole_seconds() {
    let mut core = clocked();
    core.advance_rtc(2 * 86_400 + 3 * 3_600 + 5).unwrap();
    assert_eq!(read_rtc(&mut core), [9, 3, 5, 3, 0]);
}

#[test]
fn advancing_a_halted_clock_leaves_it_alone() {
    let mut core = core(cart(MBC3_TIMER_RAM_BATTERY, 4, RAM_32K));
    set_rtc(&mut core, [1, 0, 0, 0, 0], true);
    core.advance_rtc(3_600).unwrap();
    assert_eq!(read_rtc(&mut core), [1, 0, 0, 0, 0x40]);
}

#[test]
fn advancing_the_clock_of_a_cart_without_one_is_refused() {
    let mut core = core(cart(MBC5_RAM_BATTERY, 8, RAM_32K));
    let refused = core.advance_rtc(1).unwrap_err();
    assert!(
        matches!(&refused, CoreError::InvalidState { reason } if reason.contains("no clock")),
        "{refused:?}"
    );
}
