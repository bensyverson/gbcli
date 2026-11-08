//! How far one clock advance goes on each kind of cart clock (design §6,
//! the update of leaf XJV3UL): an advance of N days reads N days on, up to
//! the clock's own bound, where its readings start to repeat.

use crate::common;

use common::cart::*;
use gbcli_core::{CartClock, Core, CoreError, Region};
use gbcli_sameboy::SameBoy;

const DAY: u64 = 86_400;
const MBC3_RAM_BATTERY: u8 = 0x13;

/// The bytes of a battery state after the cart RAM: the clock, in the shim's
/// layout (`gbcli_save_battery_state`).
fn clock_state(core: &mut SameBoy) -> Vec<u8> {
    let ram = core.region(Region::Sram).map_or(0, |r| r.bytes.len());
    core.battery_state().unwrap().unwrap()[ram..].to_vec()
}

/// A TPP1 cart with its clock started (MR3 $19).
fn tpp1_started() -> SameBoy {
    let mut core = core(tpp1_cart());
    poke_bus(&mut core, 0x0003, 0x19);
    core
}

/// TPP1's MR4, weekday and weeks.
fn tpp1_clock(core: &mut SameBoy) -> (u8, u8, u8) {
    let c = clock_state(core);
    (c[0], c[3] >> 5, c[4])
}

/// HuC3's minutes of the day and days.
fn huc3_clock(core: &mut SameBoy) -> (u16, u16) {
    let c = clock_state(core);
    (
        u16::from_le_bytes([c[8], c[9]]),
        u16::from_le_bytes([c[10], c[11]]),
    )
}

fn refused_naming(result: Result<(), CoreError>, words: &[&str]) {
    let Err(CoreError::InvalidState { reason }) = &result else {
        panic!("{result:?}")
    };
    for word in words {
        assert!(reason.contains(word), "{word}: {reason}");
    }
}

#[test]
fn the_cart_clock_is_reported_by_kind() {
    let kinds = [
        (
            cart(MBC3_TIMER_RAM_BATTERY, 4, RAM_8K),
            Some(CartClock::Mbc3),
        ),
        (cart(MBC3_RAM_BATTERY, 4, RAM_8K), None),
        (cart(HUC3, 4, RAM_8K), Some(CartClock::Huc3)),
        (tpp1_cart(), Some(CartClock::Tpp1)),
        (cart(MBC5_RAM_BATTERY, 4, RAM_8K), None),
    ];
    for (rom, kind) in kinds {
        assert_eq!(core(rom).cart_clock().unwrap(), kind);
    }
}

/// Review of e65e509: past 1024 days TPP1 was refused, though its weeks count
/// on to 256, so weeks 147 to 255 took a second power cycle.
#[test]
fn a_tpp1_advance_reads_its_weeks_on_past_1024_days() {
    let mut once = tpp1_started();
    once.advance_rtc(1500 * DAY).unwrap();
    assert_eq!(tpp1_clock(&mut once), (0x04, 2, 214), "1500 d = 214 w 2 d");
    let mut chained = tpp1_started();
    chained.advance_rtc(1000 * DAY).unwrap();
    chained.advance_rtc(500 * DAY).unwrap();
    assert_eq!(clock_state(&mut once), clock_state(&mut chained));
}

#[test]
fn a_tpp1_advance_reaches_the_overflow_flag_and_no_further() {
    let mut core = tpp1_started();
    let before = clock_state(&mut core);
    refused_naming(
        core.advance_rtc(CartClock::Tpp1.max_advance_s() + 1),
        &["TPP1", "3584 days"],
    );
    assert_eq!(clock_state(&mut core), before, "unchanged");
    core.advance_rtc(CartClock::Tpp1.max_advance_s()).unwrap();
    // 512 weeks: the 8-bit week counter wrapped twice, and MR4 bit 3 is set.
    assert_eq!(tpp1_clock(&mut core), (0x04 | 0x08, 0, 0));
}

/// Review of e65e509: HuC3 counts days in 16 bits, but was refused past 1024.
#[test]
fn a_huc3_advance_reads_its_days_on_past_1024_days() {
    let mut core = core(cart(HUC3, 4, RAM_8K));
    core.advance_rtc(1500 * DAY + 7 * 60).unwrap();
    assert_eq!(huc3_clock(&mut core), (7, 0x5DC));
}

#[test]
fn a_huc3_advance_reaches_the_day_counters_wrap_and_no_further() {
    let mut core = core(cart(HUC3, 4, RAM_8K));
    core.advance_rtc(60).unwrap();
    refused_naming(
        core.advance_rtc(CartClock::Huc3.max_advance_s() + 1),
        &["HuC3", "65536 days"],
    );
    assert_eq!(huc3_clock(&mut core), (1, 0), "unchanged");
    // From minute 1, one minute short of 65536 days is the last minute of
    // the last day.
    core.advance_rtc(CartClock::Huc3.max_advance_s() - 2 * 60)
        .unwrap();
    assert_eq!(huc3_clock(&mut core), (1439, 0xFFFF), "the last day");
    core.advance_rtc(60).unwrap();
    assert_eq!(huc3_clock(&mut core), (0, 0), "wrapped, with no flag");
    // The full bound comes back to the same reading.
    core.advance_rtc(CartClock::Huc3.max_advance_s()).unwrap();
    assert_eq!(huc3_clock(&mut core), (0, 0));
}
