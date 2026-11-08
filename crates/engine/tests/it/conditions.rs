//! Conditions evaluated against a real core (design §8).

use crate::common;

use common::*;
use gbcli_core::{Region, Registers};
use gbcli_engine::cond::{AccessValues, Condition, EvalCtx, EvalError, EvalWarning, Verdict};
use gbcli_engine::{Machine, sha256};
use gbcli_protocol::TimeMark;

const POWER_ON: TimeMark = TimeMark { ticks: 0, frame: 0 };

fn ctx(m: &Machine) -> EvalCtx<'_> {
    EvalCtx {
        core: m.core(),
        symbols: m.symbols(),
        access: None,
        base: POWER_ON,
        allow_uninit: false,
    }
}

fn eval(m: &Machine, text: &str) -> Result<i64, EvalError> {
    let cond = Condition::compile(text, m.symbols()).expect("compiles");
    cond.eval(&ctx(m), &mut Vec::new())
}

/// v1 with wFrame, wSpeed initialized: stopped at Main.loop.
fn running() -> Machine {
    let mut m = machine(V1, Some(V1_SYM));
    step_to(&mut m, 0x015E);
    m
}

#[test]
fn registers_and_flags_read_from_the_cpu() {
    let mut m = running();
    let regs = Registers {
        a: 0x12,
        f: 0b1001_0000,
        b: 0x34,
        c: 0x56,
        h: 0xC0,
        l: 0x01,
        sp: 0xC020,
        ..m.core().registers()
    };
    m.core_mut().set_registers(&regs);
    assert_eq!(eval(&m, "a").unwrap(), 0x12);
    assert_eq!(eval(&m, "bc").unwrap(), 0x3456);
    assert_eq!(eval(&m, "hl == $C001 && sp == $C020").unwrap(), 1);
    assert_eq!(eval(&m, "zf + nf * 2 + hf * 4 + cf * 8").unwrap(), 1 + 8);
    assert_eq!(eval(&m, "pc == Main.loop").unwrap(), 1);
}

#[test]
fn memory_reads_take_their_width_little_endian() {
    let mut m = running();
    m.core_mut().poke(Region::Wram, 0, &[0xFE, 0xFF]).unwrap();
    assert_eq!(eval(&m, "[wFrame]").unwrap(), 0xFE);
    assert_eq!(eval(&m, "u16[wFrame]").unwrap(), 0xFFFE);
    assert_eq!(eval(&m, "s8[wFrame]").unwrap(), -2);
    assert_eq!(eval(&m, "s16[wFrame]").unwrap(), -2);
    assert_eq!(eval(&m, "u16[wram:0]").unwrap(), 0xFFFE);
    assert_eq!(eval(&m, "[$E000]").unwrap(), 0xFE, "echo RAM");
    assert_eq!(eval(&m, "[00:0150]").unwrap(), 0xF3, "ROM: di");
    assert!(eval(&m, "[hl + 0]").is_ok());
}

#[test]
fn hardware_registers_read_decoded() {
    let m = running();
    assert_eq!(eval(&m, "[rLY]").unwrap(), i64::from(m.core().ppu().ly));
    assert_eq!(eval(&m, "ly == [rLY]").unwrap(), 1);
}

#[test]
fn a_never_written_read_fails_naming_the_symbol_unless_allowed() {
    let m = running();
    let err = eval(&m, "[wNever] == 0").unwrap_err();
    let EvalError::UninitializedRead(read) = &err else {
        panic!("{err:?}")
    };
    assert_eq!(read.symbol.as_deref(), Some("wNever"));
    assert_eq!(read.addr, Some(0xC003));
    assert!(err.remedy().contains("--allow-uninit"));
    assert!(matches!(
        eval(&m, "u16[wSpeed]"),
        Err(EvalError::UninitializedRead(_))
    ));

    let cond = Condition::compile("[wNever] | 1", m.symbols()).unwrap();
    let mut warnings = Vec::new();
    let allowed = EvalCtx {
        allow_uninit: true,
        ..ctx(&m)
    };
    assert_eq!(cond.eval(&allowed, &mut warnings).unwrap() & 1, 1);
    assert!(matches!(&warnings[..], [EvalWarning::UninitializedRead(r)] if r.addr == Some(0xC003)));
}

#[test]
fn evaluating_memory_reads_leaves_the_state_unchanged() {
    let mut m = running();
    let before = sha256(&m.core_mut().save());
    for text in [
        "[wFrame] + u16[wFrame]",
        "[rLY] + [rDIV] + [rSTAT] + [rTIMA] + [rIF] + [rP1]",
        "[$FE00] + [$8000] + [$9800] + [$4000] + [$0000]",
        "[$FF80] + [rIE] + [rLCDC]",
        "u16[vram:0x10] + [oam:4] + [hram:2] + [io:0x44]",
    ] {
        let _ = eval(&m, text);
    }
    let _ = eval(&m, "[wNever]");
    assert_eq!(sha256(&m.core_mut().save()), before);
}

#[test]
fn frame_and_t_count_from_the_time_base() {
    let m = running();
    let now = m.core().now().0;
    let base = EvalCtx {
        base: TimeMark {
            ticks: now - 200,
            frame: 0,
        },
        ..ctx(&m)
    };
    let t = Condition::compile("t", None).unwrap();
    assert_eq!(t.eval(&base, &mut Vec::new()).unwrap(), 100);
    let after = EvalCtx {
        base: TimeMark {
            ticks: now + 20,
            frame: 5,
        },
        ..ctx(&m)
    };
    assert_eq!(t.eval(&after, &mut Vec::new()).unwrap(), -10);
    let frame = Condition::compile("frame", None).unwrap();
    assert!(frame.eval(&after, &mut Vec::new()).unwrap() <= -4);
}

#[test]
fn old_and_new_belong_to_watch_conditions() {
    let m = running();
    assert!(matches!(
        eval(&m, "new == 1"),
        Err(EvalError::NoAccess { which: "new" })
    ));
    let cond = Condition::compile("old + 1 == new", None).unwrap();
    let watch = |old| EvalCtx {
        access: Some(AccessValues { old, new: 5 }),
        ..ctx(&m)
    };
    assert!(cond.holds(&watch(Some(4)), &mut Vec::new()).unwrap());
    assert_eq!(
        cond.holds(&watch(None), &mut Vec::new()),
        Err(EvalError::OldUnknown)
    );
}

#[test]
fn arithmetic_errors_are_reported_not_panicked() {
    let m = running();
    assert_eq!(eval(&m, "1 / (a - a)"), Err(EvalError::DivideByZero));
    assert_eq!(eval(&m, "0 && 1 / 0").unwrap(), 0, "short-circuit");
    assert!(matches!(
        eval(&m, "1 << 64"),
        Err(EvalError::ShiftOutOfRange { .. })
    ));
    assert!(matches!(
        eval(&m, "[$10000]"),
        Err(EvalError::AddressOutOfRange { .. })
    ));
}

#[test]
fn a_stop_trigger_reads_a_never_written_byte_as_not_yet() {
    let m = running();
    let verdict = |text: &str, allow_uninit: bool, warnings: &mut Vec<EvalWarning>| {
        let cond = Condition::compile(text, m.symbols()).expect("compiles");
        let ctx = EvalCtx {
            allow_uninit,
            ..ctx(&m)
        };
        cond.verdict(&ctx, warnings).expect("no error")
    };
    let not_yet = |text: &str| match verdict(text, false, &mut Vec::new()) {
        Verdict::NotYet(read) => read,
        other => panic!("{text}: {other:?}"),
    };
    // Never as 0: a test for 0 and a test for nonzero both wait.
    assert_eq!(not_yet("[wNever] == 0").symbol.as_deref(), Some("wNever"));
    assert_eq!(not_yet("[wNever] != 0").addr, Some(0xC003));
    // The whole condition waits, even when another part already holds.
    assert_eq!(not_yet("[wFrame] == 0 && [wNever] == 0").addr, Some(0xC003));
    assert_eq!(not_yet("[wNever] == 0 || 1").addr, Some(0xC003));
    // A part never evaluated reads nothing.
    let mut warnings = Vec::new();
    assert_eq!(
        verdict("[wFrame] == 1 && [wNever] == 0", false, &mut warnings),
        Verdict::DoesNotHold
    );
    assert_eq!(
        verdict("[wFrame] == 0", false, &mut warnings),
        Verdict::Holds
    );
    assert!(warnings.is_empty());
    // --allow-uninit reads the power-on value, and warns.
    assert_eq!(verdict("[wNever] | 1", true, &mut warnings), Verdict::Holds);
    assert!(matches!(&warnings[..], [EvalWarning::UninitializedRead(r)] if r.addr == Some(0xC003)));
    // Other errors still fail.
    let cond = Condition::compile("1 / (a - a)", None).unwrap();
    assert_eq!(
        cond.verdict(&ctx(&m), &mut Vec::new()),
        Err(EvalError::DivideByZero)
    );
}

#[test]
fn a_never_written_read_names_its_symbol_offset_in_hex() {
    let m = running();
    let err = eval(&m, "[wStack + $10]").unwrap_err();
    let EvalError::UninitializedRead(read) = &err else {
        panic!("{err:?}")
    };
    assert_eq!(read.symbol.as_deref(), Some("wStack+$10"));
}

/// The ruling on FwexpS: an offset after a name is decimal in both grammars,
/// hex with `$` or `0x` (as RGBDS reads it). Every byte of WRAM here differs,
/// so equal reads mean equal addresses.
#[test]
fn both_grammars_read_an_offset_after_a_name_in_the_same_radix() {
    let mut m = running();
    let bytes: Vec<u8> = (0..0x40u8).map(|i| i ^ 0xA5).collect();
    m.core_mut().poke(Region::Wram, 0, &bytes).unwrap();
    for (text, addr) in [
        ("wFrame+10", 0xC00A),
        ("wFrame+$10", 0xC010),
        ("wFrame+0x1F", 0xC01F),
        ("wFrame+%101", 0xC005),
        ("wStackTop-16", 0xC014),
        ("wStackTop-$10", 0xC014),
        ("rLCDC+10", 0xFF4A),
        ("rLCDC+$10", 0xFF50),
    ] {
        let at = gbcli_engine::addr::parse(text, m.symbols()).expect(text);
        assert_eq!(at.addr, addr, "the address grammar reads {text}");
        if addr < 0xFF00 {
            let by_condition = eval(&m, &format!("[{text}]")).unwrap();
            let by_address = eval(&m, &format!("[${addr:04X}]")).unwrap();
            assert_eq!(by_condition, by_address, "a condition reads {text}");
        } else {
            let by_condition = eval(&m, &format!("[{text}] == [${addr:04X}]"));
            assert_eq!(by_condition.unwrap(), 1, "a condition reads {text}");
        }
    }
}
