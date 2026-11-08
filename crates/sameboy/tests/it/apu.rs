//! APU channel state from SameBoy's public getters.

use crate::common;

use common::*;
use gbcli_core::{ApuChannel, ApuSync, Budget, Core, Feature, Support};

/// Powers the APU on and fills wave RAM with 0x01, 0x23, ... 0xEF, twice.
fn power_on_with_wave_ram(core: &mut dyn Core) {
    core.bus_write(0xFF26, 0x80).unwrap(); // NR52: APU on
    core.bus_write(0xFF1A, 0x00).unwrap(); // NR30: wave DAC off, so wave RAM is writable
    for i in 0..16u16 {
        let byte = ((2 * i as u8 % 16) << 4) | ((2 * i as u8 + 1) % 16);
        core.bus_write(0xFF30 + i, byte).unwrap();
    }
}

#[test]
fn sameboy_reports_apu_channel_state() {
    let core = quick_dmg(BUSY);
    assert_eq!(core.supports(Feature::ApuChannels), Support::Yes);
}

#[test]
fn the_wave_table_is_wave_ram_as_nibbles() {
    let mut core = quick_dmg(BUSY);
    power_on_with_wave_ram(&mut core);
    let apu = core.apu().unwrap();
    let expected: Vec<u8> = (0..32).map(|n| n % 16).collect();
    assert_eq!(apu.wave_table.to_vec(), expected);
}

#[test]
fn a_triggered_square_channel_reports_its_envelope_volume() {
    let mut core = quick_dmg(BUSY);
    power_on_with_wave_ram(&mut core);
    core.bus_write(0xFF12, 0xA0).unwrap(); // NR12: volume 10, no envelope
    core.bus_write(0xFF14, 0x80).unwrap(); // NR14: trigger
    let apu = core.apu().unwrap();
    assert_eq!(apu.channel(ApuChannel::Square1).volume, 10);
    assert_eq!(apu.channel(ApuChannel::Square2).volume, 0);
}

#[test]
fn the_noise_period_follows_nr43() {
    let mut core = quick_dmg(BUSY);
    power_on_with_wave_ram(&mut core);
    core.bus_write(0xFF22, 0x25).unwrap(); // NR43: shift 2, divisor code 5
    let apu = core.apu().unwrap();
    assert_eq!(apu.channel(ApuChannel::Noise).period, 5 << 2);
}

#[test]
fn the_apu_is_current_right_after_a_register_write_and_lags_while_running() {
    let mut core = quick_dmg(BUSY);
    power_on_with_wave_ram(&mut core);
    assert_eq!(core.apu().unwrap().sync, ApuSync::Current);
    let mut lagged = false;
    for _ in 0..200 {
        run(&mut core, Budget::instructions(1));
        lagged |= core.apu().unwrap().sync == ApuSync::Lagging;
    }
    assert!(lagged, "SameBoy batches APU cycles, so some boundaries lag");
}
