//! The power cycle (design §6, the 2026-11-07 battery-save ruling): a fresh
//! power-on that keeps the cart's battery-backed RAM and clock, or imports
//! them from a `.sav`, and resets everything else.

mod common;

use common::{BUSY, RAM_8K, machine, steps, with_cart};
use gbcli_core::{Budget, Keys, Region};
use gbcli_engine::{Battery, Flash, Machine, PowerCycled, RunOptions};

const MBC3_TIMER_RAM_BATTERY: u8 = 0x10;
const MBC5_RAM: u8 = 0x1A;
const HUC3: u8 = 0xFE;

fn cart(kind: u8) -> Vec<u8> {
    with_cart(BUSY.to_vec(), kind, RAM_8K)
}

/// The MBC3 clock after a latch: seconds, minutes, hours, days, high.
fn clock(m: &mut Machine) -> [u8; 5] {
    let core = m.core_mut();
    core.bus_write(0x0000, 0x0A).unwrap();
    core.bus_write(0x6000, 0).unwrap();
    core.bus_write(0x6000, 1).unwrap();
    let mut out = [0; 5];
    for (i, reg) in (0x08..=0x0C).enumerate() {
        core.bus_write(0x4000, reg).unwrap();
        out[i] = core.bus_read(0xA000).unwrap();
    }
    core.bus_write(0x4000, 0).unwrap();
    out
}

/// A machine that has run a while, with cart RAM and WRAM written and keys held.
fn used(kind: u8) -> Machine {
    let mut m = machine(&cart(kind), None);
    m.run(&RunOptions::new(Budget::frames(130))).unwrap();
    m.core_mut()
        .poke(Region::Sram, 0x100, &[0xDE, 0xAD, 0xBE, 0xEF])
        .unwrap();
    m.core_mut().poke(Region::Wram, 0x10, &[1, 2, 3]).unwrap();
    m.set_keys(Keys::A);
    m
}

fn region(m: &Machine, region: Region) -> Vec<u8> {
    m.core().region(region).unwrap().bytes.to_vec()
}

#[test]
fn a_power_cycle_keeps_battery_ram_and_the_clock_and_resets_everything_else() {
    let mut m = used(MBC3_TIMER_RAM_BATTERY);
    let sram = region(&m, Region::Sram);
    let before = clock(&mut m);
    assert_eq!(before[0], 2, "two emulated seconds: {before:?}");

    let cycled = m.power_cycle(Battery::Keep, Flash::Keep, 0).unwrap();
    assert_eq!(
        cycled,
        PowerCycled {
            battery_bytes: Some(8 * 1024 + 48),
            clock: gbcli_core::SavClock::Restored,
            flash_bytes: None,
            flash_sectors_in_image: 0,
        }
    );
    assert_eq!(region(&m, Region::Sram), sram);
    assert_eq!(clock(&mut m), before);

    // Everything else is what a machine just built from the same setup has.
    let fresh = Machine::new(
        gbcli_engine::Backend::sameboy(),
        common::config(&cart(MBC3_TIMER_RAM_BATTERY)),
        None,
    )
    .unwrap();
    for r in [
        Region::Wram,
        Region::Hram,
        Region::Vram,
        Region::Oam,
        Region::Io,
    ] {
        assert_eq!(region(&m, r), region(&fresh, r), "{r:?}");
    }
    assert_eq!(m.core().registers(), fresh.core().registers());
    assert_eq!(m.core().now().0, 0);
    assert_eq!(m.boot(), None);
    assert_eq!(m.keys(), Keys::A, "a held key stays held across the cycle");
}

#[test]
fn a_cart_without_a_battery_loses_its_ram() {
    let mut m = used(MBC5_RAM);
    assert_eq!(region(&m, Region::Sram)[0x100], 0xDE);
    let cycled = m.power_cycle(Battery::Keep, Flash::Keep, 0).unwrap();
    assert_eq!(cycled.battery_bytes, None);
    assert!(region(&m, Region::Sram).iter().all(|&b| b == 0xFF));
}

#[test]
fn a_power_cycle_can_import_a_sav_and_advance_the_clock() {
    let mut a = used(MBC3_TIMER_RAM_BATTERY);
    let sav = a.core_mut().battery_sav().unwrap().unwrap();
    let before = clock(&mut a);

    let mut b = machine(&cart(MBC3_TIMER_RAM_BATTERY), None);
    b.power_cycle(Battery::Sav(&sav), Flash::Keep, 3 * 3600)
        .unwrap();
    assert_eq!(region(&b, Region::Sram), region(&a, Region::Sram));
    let after = clock(&mut b);
    assert_eq!(
        after,
        [before[0], before[1], before[2] + 3, before[3], before[4]]
    );
}

#[test]
fn power_cycling_twice_from_one_sav_gives_identical_machines() {
    let sav = used(MBC3_TIMER_RAM_BATTERY)
        .core_mut()
        .battery_sav()
        .unwrap()
        .unwrap();
    let mut blobs = (0..2).map(|_| {
        let mut m = machine(&cart(MBC3_TIMER_RAM_BATTERY), None);
        m.power_cycle(Battery::Sav(&sav), Flash::Keep, 0).unwrap();
        steps(&mut m, 100);
        m.core_mut().save()
    });
    assert_eq!(blobs.next(), blobs.next());
}

#[test]
fn a_refused_import_leaves_the_machine_as_it_was() {
    let mut m = used(MBC3_TIMER_RAM_BATTERY);
    let ticks = m.core().now();
    assert!(
        m.power_cycle(Battery::Sav(&[0; 10]), Flash::Keep, 0)
            .is_err()
    );
    assert_eq!(m.core().now(), ticks);
    assert_eq!(region(&m, Region::Sram)[0x100], 0xDE);
}

/// Review of a066271: keeping the battery round-tripped it through the
/// `.sav` import, which refuses HuC3's footer, so every power cycle of a
/// HuC3 cart was refused.
#[test]
fn a_power_cycle_keeps_a_huc3_carts_battery_ram_and_clock() {
    let mut m = used(HUC3);
    m.core_mut().advance_rtc(2 * 86_400).unwrap();
    let sram = region(&m, Region::Sram);
    let sav = m.core_mut().battery_sav().unwrap().unwrap();
    let cycled = m.power_cycle(Battery::Keep, Flash::Keep, 0).unwrap();
    assert_eq!(
        cycled,
        PowerCycled {
            battery_bytes: Some(sav.len()),
            clock: gbcli_core::SavClock::Restored,
            flash_bytes: None,
            flash_sectors_in_image: 0,
        }
    );
    assert_eq!(region(&m, Region::Sram), sram);
    // HuC3's export is all emulated state, so an equal export is an equal clock.
    assert_eq!(m.core_mut().battery_sav().unwrap().unwrap(), sav);
}

#[test]
fn a_power_cycle_can_take_a_battery_state_carried_from_another_machine() {
    let mut a = used(HUC3);
    let state = a.core_mut().battery_state().unwrap().unwrap();
    let mut b = machine(&cart(HUC3), None);
    b.power_cycle(Battery::Carried(&state), Flash::Keep, 0)
        .unwrap();
    assert_eq!(region(&b, Region::Sram), region(&a, Region::Sram));
}

#[test]
fn a_power_cycle_keeps_the_rom_patches() {
    let mut m = used(MBC5_RAM);
    m.patch(0x200, &[0x12, 0x34]).unwrap();
    m.power_cycle(Battery::Keep, Flash::Keep, 0).unwrap();
    assert_eq!(&region(&m, Region::Rom)[0x200..0x202], &[0x12, 0x34]);
    assert_eq!(m.patches().len(), 1);
}

/// Review of e65e509: a power cycle keeping the battery and a fresh machine
/// given the same battery state end up in one machine, and stay so as the
/// clock runs and through a checkpoint.
#[test]
fn kept_and_carried_power_cycles_agree_and_stay_agreed() {
    for kind in [MBC3_TIMER_RAM_BATTERY, HUC3] {
        let mut a = used(kind);
        a.set_keys(Keys::NONE);
        a.core_mut().advance_rtc(3 * 86_400 + 77).unwrap();
        let state = a.core_mut().battery_state().unwrap().unwrap();
        a.power_cycle(Battery::Keep, Flash::Keep, 0).unwrap();
        let mut b = machine(&cart(kind), None);
        b.power_cycle(Battery::Carried(&state), Flash::Keep, 0)
            .unwrap();
        assert_eq!(a.core_mut().save(), b.core_mut().save(), "kind {kind:#x}");
        for m in [&mut a, &mut b] {
            m.run(&RunOptions::new(Budget::frames(200))).unwrap();
        }
        assert_eq!(
            a.core_mut().battery_state().unwrap(),
            b.core_mut().battery_state().unwrap(),
            "kind {kind:#x} after running"
        );
        assert_eq!(a.core_mut().save(), b.core_mut().save(), "kind {kind:#x}");
        let blob = a.core_mut().save();
        let mut c = machine(&cart(kind), None);
        c.core_mut().load(&blob).unwrap();
        for m in [&mut a, &mut c] {
            m.run(&RunOptions::new(Budget::frames(120))).unwrap();
        }
        assert_eq!(a.core_mut().save(), c.core_mut().save(), "kind {kind:#x}");
    }
}

/// Review of e65e509: carried cart RAM counts as written, as a `.sav`
/// import does, so a game reading its save is not an uninitialized read.
#[test]
fn carried_cart_ram_counts_as_written() {
    let mut m = used(MBC3_TIMER_RAM_BATTERY);
    m.power_cycle(Battery::Keep, Flash::Keep, 0).unwrap();
    let written = m.core().written(Region::Sram).unwrap();
    for offset in [0, 0x100, 0x1FFF] {
        assert!(!written.never_written(offset), "{offset:#x}");
    }
}
