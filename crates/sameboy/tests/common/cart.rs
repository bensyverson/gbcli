//! Synthetic cartridges for mapper and battery tests: every ROM bank tagged
//! with its own number, a header naming the cart, and a program that loops
//! at $0100, so the mapper is driven through the core's bus from the test.

use gbcli_core::{Core, CoreConfig, Model};
use gbcli_sameboy::SameBoy;

use super::quick_boot;

pub const ROM_ONLY: u8 = 0x00;
pub const MBC1: u8 = 0x01;
pub const MBC3: u8 = 0x11;
pub const MBC3_RAM: u8 = 0x12;
pub const MBC3_TIMER_RAM_BATTERY: u8 = 0x10;
pub const MBC5: u8 = 0x19;
pub const MBC5_RAM: u8 = 0x1A;
pub const MBC5_RAM_BATTERY: u8 = 0x1B;
pub const MBC5_RUMBLE_RAM: u8 = 0x1D;
pub const HUC3: u8 = 0xFE;

/// $0149 codes.
pub const NO_RAM: u8 = 0x00;
pub const RAM_8K: u8 = 0x02;
pub const RAM_32K: u8 = 0x03;
pub const RAM_128K: u8 = 0x04;

const BANK: usize = 0x4000;

/// `banks` 16 KiB banks (a power of two, at least 2), each filled with its
/// bank number as a little-endian u16, under a header for `kind` and `ram`.
pub fn cart(kind: u8, banks: usize, ram: u8) -> Vec<u8> {
    assert!(banks.is_power_of_two() && banks >= 2);
    let mut rom = vec![0u8; banks * BANK];
    for (bank, chunk) in rom.chunks_mut(BANK).enumerate() {
        let tag = (bank as u16).to_le_bytes();
        for pair in chunk.chunks_mut(2) {
            pair.copy_from_slice(&tag);
        }
    }
    rom[0x100..0x150].fill(0);
    rom[0x100] = 0x18; // jr $0100
    rom[0x101] = 0xFE;
    rom[0x147] = kind;
    rom[0x148] = (banks / 2).trailing_zeros() as u8;
    rom[0x149] = ram;
    rom[0x14D] = rom[0x134..0x14D]
        .iter()
        .fold(0u8, |x, b| x.wrapping_sub(*b).wrapping_sub(1));
    rom
}

/// A TPP1 cart with a clock and 8 KiB of battery RAM: SameBoy knows TPP1 by
/// $BC at $0147 and the magic $C1 $65 at $0149; $0152 is the RAM size and
/// $0153's bit 3 the battery.
pub fn tpp1_cart() -> Vec<u8> {
    let mut rom = cart(0xBC, 4, 0xC1);
    rom[0x14A] = 0x65;
    rom[0x152] = 0x01;
    rom[0x153] = 0x08;
    rom
}

/// A DMG core on `rom`, handed over to $0100 by the quick boot ROM.
pub fn core(rom: Vec<u8>) -> SameBoy {
    quick_boot(CoreConfig::new(Model::DmgB, rom))
}

/// The bank tag at a CPU address in ROM.
pub fn tag_at(core: &mut SameBoy, addr: u16) -> u16 {
    let lo = core.bus_read(addr).unwrap();
    let hi = core.bus_read(addr + 1).unwrap();
    u16::from_le_bytes([lo, hi])
}

/// Writes through the bus, as the CPU would.
pub fn poke_bus(core: &mut SameBoy, addr: u16, value: u8) {
    core.bus_write(addr, value).unwrap();
}

/// The MBC3 clock as the CPU reads it after a latch: seconds, minutes,
/// hours, days low, days high. Leaves RAM enabled and bank 0 selected.
pub fn read_rtc(core: &mut SameBoy) -> [u8; 5] {
    poke_bus(core, 0x0000, 0x0A);
    poke_bus(core, 0x6000, 0x00);
    poke_bus(core, 0x6000, 0x01);
    let mut clock = [0; 5];
    for (i, reg) in (0x08..=0x0C).enumerate() {
        poke_bus(core, 0x4000, reg);
        clock[i] = core.bus_read(0xA000).unwrap();
    }
    poke_bus(core, 0x4000, 0x00);
    clock
}

/// Sets the MBC3 clock through its registers (halting it first, as games
/// do), then starts it again unless `halted`.
pub fn set_rtc(core: &mut SameBoy, clock: [u8; 5], halted: bool) {
    poke_bus(core, 0x0000, 0x0A);
    poke_bus(core, 0x4000, 0x0C);
    poke_bus(core, 0xA000, 0x40);
    for (i, reg) in (0x08..=0x0B).enumerate() {
        poke_bus(core, 0x4000, reg);
        poke_bus(core, 0xA000, clock[i]);
    }
    poke_bus(core, 0x4000, 0x0C);
    let high = if halted {
        clock[4] | 0x40
    } else {
        clock[4] & !0x40
    };
    poke_bus(core, 0xA000, high);
    poke_bus(core, 0x4000, 0x00);
}
