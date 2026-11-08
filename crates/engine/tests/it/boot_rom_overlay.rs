//! While the boot ROM is mapped (rBANK bit 0 clear), the CPU reads it at
//! 0000-00FF, and on CGB also at 0200-08FF; everything the engine reads by
//! CPU address must read there too (design §2.3).

use crate::common;

use common::*;
use gbcli_core::{CoreConfig, Model};
use gbcli_engine::cond::{Condition, EvalCtx};
use gbcli_engine::disasm::SymbolLookup;
use gbcli_engine::{Backend, Machine, mem};
use gbcli_protocol::TimeMark;
use gbcli_sameboy::builtin_boot_rom;

fn eval(m: &Machine, text: &str) -> i64 {
    let ctx = EvalCtx {
        core: m.core(),
        symbols: m.symbols(),
        access: None,
        base: TimeMark { ticks: 0, frame: 0 },
        allow_uninit: false,
    };
    Condition::compile(text, m.symbols())
        .expect("compiles")
        .eval(&ctx, &mut Vec::new())
        .unwrap_or_else(|e| panic!("{text}: {e:?}"))
}

/// A DMG machine on busy.gb at power-on, the quick boot ROM still mapped.
fn dmg_at_power_on(sym: Option<&str>) -> Machine {
    let m = Machine::new(Backend::sameboy(), config(BUSY), sym).expect("machine");
    assert_eq!(m.core().registers().pc, 0x0000);
    m
}

#[test]
fn before_handoff_a_dmg_reads_0000_00ff_from_the_boot_rom_and_after_it_the_cart() {
    assert_ne!(QUICK_BOOT[0], BUSY[0], "the test needs bytes that differ");
    let mut m = dmg_at_power_on(None);
    assert_eq!(eval(&m, "[$0000]"), i64::from(QUICK_BOOT[0]));
    assert_eq!(eval(&m, "[$00FF]"), i64::from(QUICK_BOOT[0xFF]));
    assert_eq!(mem::peek(m.core(), 0x0001).unwrap(), QUICK_BOOT[1]);
    // A DMG boot ROM is 256 bytes: it covers nothing past 00FF.
    assert_eq!(mem::peek(m.core(), 0x0100).unwrap(), BUSY[0x100]);
    assert_eq!(mem::peek(m.core(), 0x0200).unwrap(), BUSY[0x200]);
    // An explicit bank names the cart ROM, mapped or not.
    assert_eq!(eval(&m, "[$00:$0000]"), i64::from(BUSY[0]));
    steps(&mut m, 4);
    assert_eq!(m.core().registers().pc, 0x0100, "handed over");
    assert_eq!(eval(&m, "[$0000]"), i64::from(BUSY[0]));
}

#[test]
fn a_cgb_boot_rom_also_covers_0200_08ff_but_not_the_header() {
    let boot = builtin_boot_rom(Model::CgbE);
    let m = Machine::new(
        Backend::sameboy(),
        CoreConfig::new(Model::CgbE, BUSY.to_vec()),
        None,
    )
    .expect("machine");
    let peek = |addr: u16| mem::peek(m.core(), addr).unwrap();
    assert_eq!(peek(0x0000), boot[0]);
    assert_eq!(peek(0x0150), BUSY[0x150], "0100-01FF is the cart header");
    for addr in [0x0200, 0x0457, 0x08FF] {
        assert_eq!(peek(addr), boot[usize::from(addr)], "{addr:#06x}");
    }
    assert_eq!(peek(0x0900), BUSY[0x900]);
}

#[test]
fn a_cart_symbol_does_not_name_a_boot_rom_address() {
    let mut m = dmg_at_power_on(Some("00:0000 Rst00\n"));
    let named = |m: &Machine| {
        m.symbols()
            .unwrap()
            .mapped(m.core())
            .symbol(0x0000)
            .map(|s| s.name)
    };
    assert_eq!(mem::cart_bank_at(m.core(), 0x0000), None);
    assert_eq!(named(&m), None);
    steps(&mut m, 4);
    assert_eq!(mem::cart_bank_at(m.core(), 0x0000), Some(0));
    assert_eq!(named(&m).as_deref(), Some("Rst00"));
}
