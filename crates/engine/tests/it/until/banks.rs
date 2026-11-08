//! Predicates over memory in switchable banks: each must stop exactly where
//! the per-instruction oracle does, whatever bank the write was made through.

use super::*;

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
fn a_cpu_write_that_cgb_oam_dma_moves_stops_where_it_lands() {
    // On CGB, while OAM DMA reads ROM, a CPU write to WRAM lands in the
    // other 4 KiB half: ld [$D123] writes $C123 (SameBoy memory.c).
    let (rom, call_at) = dma_conflict_rom(true);
    let mut m = cgb_machine(&rom, None);
    step_to(&mut m, call_at);
    check(&mut m, "[$C123] == $77", &lenient(), 200, Cadence::Writes);
    assert_eq!(m.core().region(Region::Wram).unwrap().bytes[0x1123], 0);
}

#[test]
fn a_write_to_a_mirror_of_small_cart_ram_reaches_its_byte() {
    // 2 KiB of cart RAM repeats every $800 bytes: $A800 is offset 0.
    let rom = with_cart(program(&count_in_cart_ram(0, 0xA800)), MBC5_RAM, RAM_2K);
    let mut m = machine(&rom, None);
    step_to(&mut m, 0x015F);
    check(&mut m, "[sram:0] == 5", &lenient(), 100, Cadence::Writes);
}
