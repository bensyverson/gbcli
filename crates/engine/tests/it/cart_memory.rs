//! Cart memory read, watched and checked as hardware maps it (design §2.3):
//! mappers whose cart RAM writes the core can't place, ROM0 banking, banks
//! and addresses that alias through the memory's size, and watches and
//! breakpoints named in an aliased bank.

use crate::common;

use common::*;
use gbcli_core::Budget;
use gbcli_engine::cond::{Condition, EvalCtx, EvalError};
use gbcli_engine::{Machine, RunOptions, mem};
use gbcli_protocol::{Cadence, Location, StopReason, TimeMark, WatchAccess};

const POWER_ON: TimeMark = TimeMark { ticks: 0, frame: 0 };

fn eval(m: &Machine, text: &str) -> Result<i64, EvalError> {
    let ctx = EvalCtx {
        core: m.core(),
        symbols: m.symbols(),
        access: None,
        base: POWER_ON,
        allow_uninit: false,
    };
    Condition::compile(text, m.symbols())
        .expect("compiles")
        .eval(&ctx, &mut Vec::new())
}

fn at(addr: u16, bank: u16) -> Location {
    Location {
        addr,
        bank: Some(bank),
        symbol: None,
    }
}

/// A machine counting at `addr` in cart RAM bank `bank`, stopped in the
/// counting loop after the first few increments.
fn counting(kind: u8, ram: u8, bank: u8, addr: u16) -> Machine {
    let mut m = machine(
        &with_cart(program(&count_in_cart_ram(bank, addr)), kind, ram),
        None,
    );
    step_to(&mut m, 0x015F);
    steps(&mut m, 6);
    m
}

const MBC2: u8 = 0x06;

#[test]
fn cart_ram_the_core_cannot_place_writes_in_is_untracked_not_unwritten() {
    // MBC2's RAM path isn't mirrored by the write hook, so the core can't
    // say which bytes the program wrote; it must not call them unwritten.
    let m = counting(MBC2, 0, 0, 0xA000);
    assert!(!mem::never_written(m.core(), 0xA000));
    assert!(!mem::never_written(m.core(), 0xA001));
    assert_eq!(eval(&m, "[$A000] & $0F").unwrap(), 3);
}

/// A 1 MiB MBC1 cart that maps bank $20 at ROM0 (mode 1, upper bits 1), then
/// loops. Bank $20 holds a copy of bank 0, so the code runs on, with a
/// different byte at $3FF0.
fn mbc1_mode_1() -> Vec<u8> {
    #[rustfmt::skip]
    let code = [
        0x3E, 0x01,       // ld a, 1
        0xEA, 0x00, 0x40, // ld [$4000], a: upper bank bits
        0xEA, 0x00, 0x60, // ld [$6000], a: mode 1
        0x18, 0xFE,       // $0158: jr @
    ];
    let mut rom = program(&code);
    rom.resize(0x10_0000, 0);
    rom[0x147] = 0x01; // MBC1
    rom[0x148] = 0x05; // 1 MiB
    fix_header_checksum(&mut rom);
    rom[0x3FF0] = 0x11;
    rom.copy_within(0..0x4000, 0x20 * 0x4000);
    rom[0x20 * 0x4000 + 0x3FF0] = 0x22;
    rom
}

#[test]
fn rom0_reads_follow_mbc1_mode_1_banking() {
    let mut m = machine(&mbc1_mode_1(), None);
    step_to(&mut m, 0x0158);
    assert_eq!(mem::bank_at(m.core(), 0x0150), 0x20);
    assert_eq!(eval(&m, "[$3FF0]").unwrap(), 0x22);
    assert_eq!(eval(&m, "[$20:$3FF0]").unwrap(), 0x22);
    assert_eq!(eval(&m, "[$00:$3FF0]").unwrap(), 0x11);
    // A bank switch can remap ROM0, so a read of it by CPU address is
    // re-evaluated after every instruction.
    let cond = Condition::compile("[$3FF0] == $22", None).unwrap();
    assert_eq!(cond.cadence(m.core()), Cadence::EveryInstruction);
}

#[test]
fn rom0_reads_follow_mmm01_banking() {
    // MMM01 boots into the cart's last 32 KiB, which holds the menu. SameBoy
    // keeps the ROM rotated so that file's first 32 KiB is the last.
    let mut rom = BUSY.to_vec();
    rom.resize(0x1_0000, 0);
    rom[0x147] = 0x0B; // MMM01
    rom[0x148] = 0x01; // 64 KiB
    fix_header_checksum(&mut rom);
    rom[0x3FF0] = 0xAA;
    rom[0xBFF0] = 0x55;
    let m = machine(&rom, None);
    assert_eq!(eval(&m, "[$3FF0]").unwrap(), 0xAA);
}

#[test]
fn a_banked_read_past_the_cart_ram_size_aliases_through_its_mask() {
    // 8 KiB is one bank: bank 1 is bank 0 again.
    let m = counting(MBC5_RAM, RAM_8K, 0, 0xA000);
    assert_eq!(eval(&m, "[$00:$A000]").unwrap(), 3);
    assert_eq!(eval(&m, "[$01:$A000]").unwrap(), 3);
    assert_eq!(eval(&m, "[$03:$A000]").unwrap(), 3);
    // 2 KiB repeats every $800 bytes, in every bank.
    let m = counting(MBC5_RAM, RAM_2K, 0, 0xA000);
    assert_eq!(eval(&m, "[$A800]").unwrap(), 3);
    assert_eq!(eval(&m, "[$01:$B800]").unwrap(), 3);
    assert!(!mem::never_written(m.core(), 0xB000));
}

#[test]
fn a_banked_read_past_the_rom_size_aliases_through_its_mask() {
    // banked.gb has four ROM banks: bank 5 is bank 1.
    let m = machine(BANKED, None);
    assert_eq!(
        eval(&m, "[$05:$4006]").unwrap(),
        eval(&m, "[$01:$4006]").unwrap()
    );
}

#[test]
fn a_banked_cart_ram_watch_sees_writes_aliased_through_another_bank() {
    // The program clears $A000 with RAM bank 1 selected on an 8 KiB cart:
    // the byte is bank 0's.
    for bank in [0, 1] {
        let mut m = machine(
            &with_cart(program(&count_in_cart_ram(1, 0xA000)), MBC5_RAM, RAM_8K),
            None,
        );
        let id = m
            .add_watch(WatchAccess::Write, at(0xA000, bank), None, false)
            .unwrap();
        let stop = m.run(&RunOptions::new(Budget::frames(2))).unwrap().stop;
        assert!(
            matches!(stop.reason, StopReason::Watchpoint { id: i, addr: 0xA000, .. } if i == id),
            "bank {bank}: {stop:?}"
        );
        assert_eq!(stop.pc, 0x015F, "the `ld [hl], 0` before the loop");
    }
}

#[test]
fn a_breakpoint_in_a_bank_past_the_rom_size_fires_in_the_bank_it_aliases() {
    // banked.gb runs bank 1's routine at $4000 first; bank 5 is bank 1.
    let mut m = machine(BANKED, None);
    let id = m.add_breakpoint(at(0x4000, 5), None).unwrap();
    let stop = m.run(&RunOptions::new(Budget::frames(5))).unwrap().stop;
    assert_eq!(stop.reason, StopReason::Breakpoint { id });
    assert_eq!((stop.pc, stop.bank), (0x4000, 1));
    assert_eq!(m.core().region(gbcli_core::Region::Rom).unwrap().bank, 1);
}
