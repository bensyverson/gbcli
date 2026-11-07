//! Predicates over memory in switchable banks: each must stop exactly where
//! the per-instruction oracle does, whatever bank the write was made through.

use gbcli_core::{BootRom, CoreConfig, Model};
use gbcli_engine::Backend;

use super::*;

/// A CGB-mode machine on `rom`, handed over by the quick boot ROM (which,
/// unlike the real one, never switches a CGB to DMG mode).
fn cgb_machine(rom: &[u8], sym: Option<&str>) -> Machine {
    let mut config = CoreConfig::new(Model::CgbE, rom.to_vec());
    config.boot_rom = BootRom::Custom(QUICK_BOOT.to_vec());
    let mut m = Machine::new(Backend::sameboy(), config, sym).expect("machine");
    steps(&mut m, 4);
    assert_eq!(
        m.core().registers().pc,
        0x0100,
        "quick boot didn't hand over"
    );
    m
}

/// Counts up at $D000 forever: `ld hl, $D000; xor a; ld [hl], a; .loop:
/// inc [hl]; jr .loop`.
const COUNT_D000: &[u8] = &[0x21, 0x00, 0xD0, 0xAF, 0x77, 0x34, 0x18, 0xFD];

#[test]
fn a_wramx_symbol_in_bank_00_stops_on_its_write() {
    // `rgblink -w` links WRAMX as bank 00; on a DMG the CPU reaches it with
    // bank 1 mapped (SameBoy never maps WRAM bank 0 at $D000).
    let mut m = machine(&program(COUNT_D000), Some("00:D000 wHigh\n"));
    step_to(&mut m, 0x0155);
    check(&mut m, "[wHigh] == 5", &lenient(), 100, Cadence::Writes);
    check(&mut m, "[$00:$D000] == 9", &lenient(), 100, Cadence::Writes);
}

#[test]
fn cgb_wramx_banks_switched_by_svbk_stop_on_their_own_writes() {
    // Clears $D000 in banks 3 and 1 (SVBK 0 maps bank 1), then loops: one
    // increment in bank 3, two in bank 1.
    #[rustfmt::skip]
    let code = [
        0x3E, 0x03, 0xE0, 0x70, // ld a, 3; ldh [rSVBK], a
        0xAF, 0xEA, 0x00, 0xD0, // xor a; ld [$D000], a
        0xE0, 0x70,             // ldh [rSVBK], a
        0xEA, 0x00, 0xD0,       // ld [$D000], a
        0x21, 0x00, 0xD0,       // ld hl, $D000
        0x3E, 0x03, 0xE0, 0x70, // $0160 .loop: ld a, 3; ldh [rSVBK], a
        0x34,                   // inc [hl]
        0xAF, 0xE0, 0x70,       // xor a; ldh [rSVBK], a
        0x34, 0x34,             // inc [hl]; inc [hl]
        0x18, 0xF4,             // jr .loop
    ];
    let mut m = cgb_machine(&program(&code), None);
    step_to(&mut m, 0x0160);
    check(&mut m, "[$03:$D000] == 5", &lenient(), 200, Cadence::Writes);
    check(
        &mut m,
        "[$01:$D000] == 13",
        &lenient(),
        200,
        Cadence::Writes,
    );
    // Bank 00 names the bank SVBK 0 maps: bank 1.
    check(
        &mut m,
        "[$00:$D000] == 20",
        &lenient(),
        200,
        Cadence::Writes,
    );
    check(
        &mut m,
        "[wram:$3000] == 12",
        &lenient(),
        200,
        Cadence::Writes,
    );
}

#[test]
fn banked_cart_ram_and_rom_stop_on_the_write_in_their_bank() {
    // banked.gb writes $11 to cart RAM bank 1 and $22 to bank 2, each from
    // a routine in the ROM bank of the same number.
    let mut m = machine(BANKED, None);
    check(
        &mut m,
        "[$02:$A000] == $22",
        &lenient(),
        200,
        Cadence::Writes,
    );
    let mut m = machine(BANKED, None);
    check(
        &mut m,
        "[sram:$2000] == $11",
        &lenient(),
        200,
        Cadence::Writes,
    );
    // A ROM byte never changes, so the cadence comes from the cart RAM read.
    let mut m = machine(BANKED, None);
    check(
        &mut m,
        "[$02:$4006] == $22 && [$02:$A000] == $22",
        &lenient(),
        200,
        Cadence::Writes,
    );
}

/// Enables cart RAM, selects RAM bank `bank`, clears `addr` and counts up
/// there forever.
fn count_in_cart_ram(bank: u8, addr: u16) -> Vec<u8> {
    let [lo, hi] = addr.to_le_bytes();
    #[rustfmt::skip]
    let code = [
        0x3E, 0x0A, 0xEA, 0x00, 0x00, // ld a, $0A; ld [$0000], a
        0x3E, bank, 0xEA, 0x00, 0x40, // ld a, bank; ld [$4000], a
        0x21, lo, hi,                 // ld hl, addr
        0x36, 0x00,                   // ld [hl], 0
        0x34,                         // $015F .loop: inc [hl]
        0x18, 0xFD,                   // jr .loop
    ];
    code.to_vec()
}

const MBC5_RAM: u8 = 0x1A;
const RAM_2K: u8 = 0x01;
const RAM_8K: u8 = 0x02;

#[test]
fn a_write_through_an_aliased_ram_bank_reaches_bank_0() {
    // 8 KiB of cart RAM is one bank: RAM bank 1 is bank 0 again.
    let rom = with_cart(program(&count_in_cart_ram(1, 0xA000)), MBC5_RAM, RAM_8K);
    let mut m = machine(&rom, None);
    step_to(&mut m, 0x015F);
    check(&mut m, "[sram:0] == 5", &lenient(), 100, Cadence::Writes);
    check(&mut m, "[$00:$A000] == 9", &lenient(), 100, Cadence::Writes);
}

#[test]
fn a_write_to_a_mirror_of_small_cart_ram_reaches_its_byte() {
    // 2 KiB of cart RAM repeats every $800 bytes: $A800 is offset 0.
    let rom = with_cart(program(&count_in_cart_ram(0, 0xA800)), MBC5_RAM, RAM_2K);
    let mut m = machine(&rom, None);
    step_to(&mut m, 0x015F);
    check(&mut m, "[sram:0] == 5", &lenient(), 100, Cadence::Writes);
}
