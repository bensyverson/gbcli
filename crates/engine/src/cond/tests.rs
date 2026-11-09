//! Parser tests; evaluation is tested against a real core in tests/it/conditions.rs.

use super::*;
use crate::mem::CartSizes;
use gbcli_core::Support;
use gbcli_protocol::Cadence;

const SYM: &str = "00:0150 Main\n00:015e Main.loop\n03:4123 Far\n02:a000 sSave\n\
                   00:c000 wFrame\n01:4000 Init\n02:4000 Init\n";

fn syms() -> Symbols {
    Symbols::parse(SYM).unwrap()
}

fn parse(text: &str) -> Expr {
    Condition::compile(text, Some(&syms()))
        .unwrap_or_else(|e| panic!("{text}: {e:?}"))
        .expr
}

fn num(n: i64) -> Box<Expr> {
    Box::new(Expr::Num(n))
}

#[test]
fn numbers_in_every_radix() {
    assert_eq!(parse("$C0a0"), Expr::Num(0xC0A0));
    assert_eq!(parse("0x10"), Expr::Num(16));
    assert_eq!(parse("42"), Expr::Num(42));
    assert_eq!(parse("%101"), Expr::Num(5));
}

#[test]
fn precedence_follows_c() {
    assert_eq!(
        parse("1 + 2 * 3 == 7 && 1"),
        Expr::Binary(
            BinOp::And,
            Box::new(Expr::Binary(
                BinOp::Eq,
                Box::new(Expr::Binary(
                    BinOp::Add,
                    num(1),
                    Box::new(Expr::Binary(BinOp::Mul, num(2), num(3)))
                )),
                num(7)
            )),
            num(1)
        )
    );
    assert_eq!(
        parse("a | b ^ c & d"),
        Expr::Binary(
            BinOp::BitOr,
            Box::new(Expr::Reg(Reg::A)),
            Box::new(Expr::Binary(
                BinOp::BitXor,
                Box::new(Expr::Reg(Reg::B)),
                Box::new(Expr::Binary(
                    BinOp::BitAnd,
                    Box::new(Expr::Reg(Reg::C)),
                    Box::new(Expr::Reg(Reg::D))
                ))
            ))
        )
    );
    assert_eq!(
        parse("10 % 3 - -1"),
        Expr::Binary(
            BinOp::Sub,
            Box::new(Expr::Binary(BinOp::Rem, num(10), num(3))),
            Box::new(Expr::Unary(UnOp::Neg, num(1)))
        )
    );
}

#[test]
fn registers_flags_and_machine_values() {
    assert_eq!(parse("hl"), Expr::Reg(Reg::Hl));
    assert_eq!(parse("zf"), Expr::Flag(Flag::Z));
    assert_eq!(
        parse("!ime"),
        Expr::Unary(UnOp::LogicalNot, Box::new(Expr::Ime))
    );
    for (text, expr) in [
        ("bank", Expr::Bank),
        ("frame", Expr::Frame),
        ("t", Expr::T),
        ("ly", Expr::Ly),
        ("old", Expr::Old),
        ("new", Expr::New),
    ] {
        assert_eq!(parse(text), expr);
    }
}

#[test]
fn reads_take_explicit_widths() {
    let cpu = |addr| Place::Cpu(num(addr));
    assert_eq!(
        parse("[$C000]"),
        Expr::Read {
            width: Width::U8,
            place: cpu(0xC000)
        }
    );
    assert_eq!(
        parse("u16[$C000]"),
        Expr::Read {
            width: Width::U16,
            place: cpu(0xC000)
        }
    );
    assert_eq!(
        parse("s8[$C000]"),
        Expr::Read {
            width: Width::S8,
            place: cpu(0xC000)
        }
    );
    assert_eq!(
        parse("s16[hl]"),
        Expr::Read {
            width: Width::S16,
            place: Place::Cpu(Box::new(Expr::Reg(Reg::Hl)))
        }
    );
}

#[test]
fn symbols_resolve_to_bank_and_address() {
    assert_eq!(
        parse("pc == Main.loop"),
        Expr::Binary(
            BinOp::Eq,
            Box::new(Expr::Reg(Reg::Pc)),
            Box::new(Expr::Sym {
                name: "Main.loop".into(),
                bank: 0,
                addr: 0x015E
            })
        )
    );
    // An unbanked symbol reads in the current mapping; a banked one in its own bank.
    assert!(matches!(
        parse("[wFrame]"),
        Expr::Read {
            place: Place::Cpu(_),
            ..
        }
    ));
    assert_eq!(
        parse("[Far+2]"),
        Expr::Read {
            width: Width::U8,
            place: Place::Banked {
                bank: 3,
                addr: 0x4125
            }
        }
    );
    assert_eq!(
        parse("[sSave]"),
        Expr::Read {
            width: Width::U8,
            place: Place::Banked {
                bank: 2,
                addr: 0xA000
            }
        }
    );
}

#[test]
fn bank_qualified_and_region_addresses() {
    assert_eq!(
        parse("[03:41AB]"),
        Expr::Read {
            width: Width::U8,
            place: Place::Banked {
                bank: 3,
                addr: 0x41AB
            }
        }
    );
    assert_eq!(
        parse("[$03:$4123]"),
        Expr::Read {
            width: Width::U8,
            place: Place::Banked {
                bank: 3,
                addr: 0x4123
            }
        }
    );
    assert_eq!(
        parse("u16[wram:0x1A0]"),
        Expr::Read {
            width: Width::U16,
            place: Place::Region {
                region: Region::Wram,
                offset: 0x1A0
            }
        }
    );
}

#[test]
fn hardware_registers_are_always_available() {
    let none = Condition::compile("[rLY] == 144 && [rLCDC] & $80", None).unwrap();
    assert!(matches!(none.expr, Expr::Binary(BinOp::And, ..)));
    assert_eq!(parse("rIE"), Expr::Num(0xFFFF));
}

/// The address grammar takes the same order (`addr::hwreg_tests`).
#[test]
fn a_hardware_register_shadows_a_sym_symbol_of_the_same_name() {
    let s = Symbols::parse("00:C000 rDMA\n").unwrap();
    let cond = Condition::compile("rDMA", Some(&s)).unwrap();
    assert_eq!(cond.expr, Expr::Num(0xFF46));
}

#[test]
fn a_c_style_signed_width_names_s8_and_s16() {
    for (text, wanted) in [("i8[wFrame] < 0", "s8[ADDR]"), ("i16[$C000]", "s16[ADDR]")] {
        let err = Condition::compile(text, Some(&syms())).unwrap_err();
        let ExprError::Syntax {
            at: 0,
            hint: Some(hint),
            ..
        } = &err
        else {
            panic!("{text}: {err:?}")
        };
        assert!(hint.contains(wanted), "{hint}");
        assert!(err.remedy().contains("s8") && err.remedy().contains("s16"));
    }
    // Without the C name, a symbol named i8 still reads as one.
    let s = Symbols::parse("00:c000 i8\n").unwrap();
    assert!(Condition::compile("[i8] == 1", Some(&s)).is_ok());
    // And the real widths parse.
    parse("s8[wFrame] < 0 && s16[wFrame] < 0");
}

#[test]
fn a_bare_ram_symbol_compared_suggests_a_read() {
    let err = Condition::compile("wFrame >= 60", Some(&syms())).unwrap_err();
    let ExprError::Syntax {
        hint: Some(hint), ..
    } = &err
    else {
        panic!("{err:?}")
    };
    assert!(
        hint.contains("[wFrame] >= 60") && hint.contains("u16[wFrame] >= 60"),
        "{hint}"
    );
    assert!(err.remedy().contains("[wFrame] >= 60"));
}

#[test]
fn unknown_and_ambiguous_symbols_list_candidates() {
    let err = Condition::compile("[wFrme]", Some(&syms())).unwrap_err();
    assert_eq!(
        err,
        ExprError::UnknownSymbol {
            name: "wFrme".into(),
            near: vec!["wFrame".into()]
        }
    );
    let err = Condition::compile("pc == Init", Some(&syms())).unwrap_err();
    let ExprError::AmbiguousSymbol { candidates, .. } = &err else {
        panic!("{err:?}")
    };
    assert_eq!(candidates.len(), 2);
    assert!(err.remedy().contains("01:4000") && err.remedy().contains("02:4000"));
    // With no .sym at all, the name is no typo: its own variant says so
    // (tests/it/no_symbols.rs).
    assert!(matches!(
        Condition::compile("[wFrame]", None),
        Err(ExprError::NoSymbols { .. })
    ));
}

/// The boot ROM overlays 0000-00FF and 0200-08FF until a write to FF50
/// unmaps it, and that write lands in neither range, so a read there must
/// be re-evaluated after every instruction.
#[test]
fn a_read_the_boot_rom_overlays_is_switchable() {
    for addr in ["$0000", "$00FF", "$0200", "$08FF"] {
        let expr = parse(&format!("[{addr}] == 1"));
        let cadence = cadence::analyze(&expr, Support::Yes, CartSizes::default()).cadence();
        assert_eq!(cadence, Cadence::EveryInstruction, "{addr}");
    }
}

#[test]
fn the_cadence_names_the_costliest_trigger_the_reads_need() {
    let cadence =
        |text: &str| cadence::analyze(&parse(text), Support::Yes, CartSizes::default()).cadence();

    // The write hook covers these, so only a write can change them.
    assert_eq!(cadence("u16[wFrame] >= 60"), Cadence::Writes);
    assert_eq!(cadence("[wFrame] == 3"), Cadence::Writes);
    assert_eq!(cadence("[wram:0x1A0] == 1"), Cadence::Writes);
    assert_eq!(cadence("[hram:0] == 1"), Cadence::Writes);
    assert_eq!(cadence("[ie:0] == 1"), Cadence::Writes);
    // A symbol in a switchable area reads in its own bank, so a bank switch
    // cannot change it: only a write can.
    assert_eq!(cadence("[Far+2] == 1"), Cadence::Writes);
    assert_eq!(cadence("[sSave] == 1"), Cadence::Writes);
    // Nothing a run does changes ROM: no trigger at all.
    assert_eq!(cadence("[rom:0x4000] == 1"), Cadence::Writes);
    assert_eq!(cadence("[$00:$0100] == 1"), Cadence::Writes);
    // A CPU address in ROM changes on a bank switch: ROMX on any mapper,
    // ROM0 on MBC1 (mode 1) and MMM01.
    assert_eq!(cadence("[$4123] == 1"), Cadence::EveryInstruction);
    assert_eq!(cadence("[$0100] == 1"), Cadence::EveryInstruction);
    // VRAM and OAM writes can come from DMA, which the write hook misses.
    assert_eq!(cadence("[$8000] == 1"), Cadence::EveryInstruction);
    assert_eq!(cadence("[$FE00] == 1"), Cadence::EveryInstruction);
    assert_eq!(cadence("[vram:0] == 1"), Cadence::EveryInstruction);
    // Registers, flags, machine values and dynamic addresses.
    assert_eq!(cadence("a == 1"), Cadence::EveryInstruction);
    assert_eq!(cadence("pc == Main"), Cadence::EveryInstruction);
    assert_eq!(cadence("zf == 1"), Cadence::EveryInstruction);
    assert_eq!(cadence("ime == 1"), Cadence::EveryInstruction);
    assert_eq!(cadence("bank == 1"), Cadence::EveryInstruction);
    assert_eq!(cadence("t > 100"), Cadence::EveryInstruction);
    assert_eq!(cadence("[hl] == 1"), Cadence::EveryInstruction);
    assert_eq!(cadence("[wFrame + b] == 1"), Cadence::EveryInstruction);
    // Vblank and scanline counters.
    assert_eq!(cadence("frame >= 3"), Cadence::Vblank);
    assert_eq!(cadence("ly == 100"), Cadence::Scanline);
    // The costliest trigger names the cadence; a mixed predicate also
    // keeps its stores, so a mid-frame change is not missed.
    assert_eq!(cadence("frame >= 3 && [wFrame] == 2"), Cadence::Vblank);
    assert_eq!(cadence("ly == 100 && frame >= 3"), Cadence::Scanline);
    // Cart RAM on a mapper whose RAM path the hook doesn't mirror.
    let no = Support::No { reason: "test" };
    assert_eq!(
        cadence::analyze(&parse("[sSave] == 1"), no, CartSizes::default()).cadence(),
        Cadence::EveryInstruction
    );
    assert_eq!(
        cadence::analyze(&parse("[wFrame] == 1"), no, CartSizes::default()).cadence(),
        Cadence::Writes
    );
    assert_eq!(
        cadence("frame >= 3 && pc == Main"),
        Cadence::EveryInstruction
    );
    // Any IO read takes the baseline: internal writes (serial, APU, PPU) do
    // not go through the write hook.
    assert_eq!(cadence("[rLCDC] & $80"), Cadence::EveryInstruction);
    assert_eq!(cadence("[rLY] >= 100"), Cadence::EveryInstruction);
    assert_eq!(cadence("[rDIV] > 0"), Cadence::EveryInstruction);
    assert_eq!(cadence("[rTIMA] > 0"), Cadence::EveryInstruction);
    assert_eq!(cadence("[rIF] != 0"), Cadence::EveryInstruction);
    assert_eq!(cadence("[rSTAT] != 0"), Cadence::EveryInstruction);
    // A CPU address in a switchable area changes with the bank, not the byte.
    assert_eq!(cadence("[$D000] == 1"), Cadence::EveryInstruction);
    assert_eq!(cadence("[$A000] == 1"), Cadence::EveryInstruction);
}

#[test]
fn malformed_conditions_point_at_the_problem() {
    for text in [
        "",
        "1 +",
        "[1",
        "(1",
        "1 2",
        "u16 1",
        "$zz",
        "1 @ 2",
        "[foo:1]",
        "u16[03:x]",
    ] {
        assert!(
            matches!(
                Condition::compile(text, Some(&syms())),
                Err(ExprError::Syntax { .. }) | Err(ExprError::UnknownSymbol { .. })
            ),
            "{text:?} should fail"
        );
    }
    let ExprError::Syntax { at, .. } = Condition::compile("1 + )", None).unwrap_err() else {
        panic!()
    };
    assert_eq!(at, 4);
}

#[test]
fn the_triggers_are_a_set_and_reads_arm_the_bytes_the_evaluator_reads() {
    let triggers = |text: &str| cadence::analyze(&parse(text), Support::Yes, CartSizes::default());

    // `frame` needs the vblank even beside `ly`: with the LCD off LY never
    // changes while frames still end.
    let both = triggers("frame >= 3 && ly == 0");
    assert!(both.vblank && both.scanline && !both.every_instruction);

    // A WRAMX symbol in bank 00 (`rgblink -w`) reads bank 1's byte, as the
    // CPU sees it with bank 0 selected.
    let syms = Symbols::parse("00:d000 wHigh\n").unwrap();
    let high = Condition::compile("u16[wHigh] == 1", Some(&syms)).unwrap();
    let armed = cadence::analyze(&high.expr, Support::Yes, CartSizes::default()).stores;
    assert_eq!(
        armed.entries(),
        [(Region::Wram, 0x1000), (Region::Wram, 0x1001)]
    );

    let armed = triggers("[sSave] == 1 && [$C010] == 2 && [hram:3] == 4 && [ie:0] == 5").stores;
    assert_eq!(
        armed.entries(),
        [
            (Region::Sram, 0x4000),
            (Region::Wram, 0x10),
            (Region::Hram, 3),
            (Region::Ie, 0)
        ]
    );
    assert!(triggers("[Far] == 1").stores.is_empty(), "ROM is constant");
}

#[test]
fn a_bank_qualified_address_may_start_with_a_hex_letter() {
    // The address grammar reads `02:A000`; `A000` lexes as a name, which
    // must still read as hex after a bank.
    let banked = Expr::Read {
        width: Width::U8,
        place: Place::Banked {
            bank: 2,
            addr: 0xA000,
        },
    };
    assert_eq!(parse("[$02:A000]"), banked);
    assert_eq!(parse("[02:a000]"), banked);
    assert_eq!(parse("[0x02:A000]"), banked);
}

#[test]
fn a_bank_head_that_is_all_hex_letters_is_a_bank_as_in_the_address_grammar() {
    // `ff:4000` and `A:4000` are banks $FF and $0A to `addr::parse`; a head
    // that isn't hex names a region there, and must here too.
    for (text, grammar) in [("[ff:4000]", "ff:4000"), ("[A:4000]", "A:4000")] {
        let at = crate::addr::parse(grammar, None).expect("the grammar reads it");
        assert_eq!(
            parse(text),
            Expr::Read {
                width: Width::U8,
                place: Place::Banked {
                    bank: at.bank.expect("a bank"),
                    addr: at.addr,
                },
            },
            "{text}"
        );
    }
}

/// Design §7: a region offset is hex, prefixed or not, as the address
/// grammar reads it (`poke sram:10` writes offset $10).
#[test]
fn a_region_offset_is_hex_prefixed_or_not_as_in_the_address_grammar() {
    let offset = |text: &str| match parse(text) {
        Expr::Read {
            place: Place::Region { offset, .. },
            ..
        } => offset,
        other => panic!("{text}: {other:?}"),
    };
    assert_eq!(offset("[sram:10]"), 0x10);
    assert_eq!(offset("[sram:$10]"), 0x10);
    assert_eq!(offset("[sram:0x10]"), 0x10);
    assert_eq!(offset("[wram:1A0]"), 0x1A0);
    assert_eq!(offset("[wram:a0]"), 0xA0, "hex that lexes as a name");
    assert_eq!(offset("[rom:14E]"), 0x14E);
    assert_eq!(offset("[rom:$1FFFFF]"), 0x1F_FFFF, "past 16 bits");
    for bad in ["[sram:1G]", "[wram:hl]", "[sram:-1]"] {
        assert!(
            Condition::compile(bad, None).is_err(),
            "{bad}: an offset is one hex number"
        );
    }
}

/// The address grammar reads a region offset or a bank-qualified part as
/// hex only, so `%10` is refused there; a condition refused it too, rather
/// than reading binary 2 (leaf dwM7t6).
#[test]
fn a_binary_region_offset_or_bank_part_is_refused_as_in_the_address_grammar() {
    for bad in ["[sram:%10]", "[%1:4000]", "[01:%100]"] {
        let e = Condition::compile(bad, None).expect_err(bad);
        assert!(e.to_string().contains("hex"), "{bad}: {e}");
        assert!(
            address_grammar_refuses(bad),
            "{bad}: the address grammar refuses it"
        );
    }
}

fn address_grammar_refuses(bracketed: &str) -> bool {
    let inner = bracketed.trim_start_matches('[').trim_end_matches(']');
    crate::addr::parse(inner, None).is_err()
}

#[test]
fn region_names_match_the_address_grammar() {
    let region = |text: &str| match parse(text) {
        Expr::Read {
            place: Place::Region { region, .. },
            ..
        } => region,
        other => panic!("{text}: {other:?}"),
    };
    assert_eq!(region("[bootrom:0]"), Region::BootRom);
    assert_eq!(region("[WRAM:0]"), Region::Wram);
    assert_eq!(region("[BgPal:0]"), Region::BgPalette);
    assert!(
        Condition::compile("[boot:0]", None).is_err(),
        "the region is `bootrom` everywhere"
    );
}

/// The hint a refused condition carries.
fn refusal_hint(text: &str) -> String {
    let err = Condition::compile(text, Some(&syms())).unwrap_err();
    let ExprError::Syntax {
        message,
        hint: Some(hint),
        ..
    } = &err
    else {
        panic!("{text}: {err:?}")
    };
    assert!(
        message.contains("wFrame is an address"),
        "{text}: {message}"
    );
    hint.clone()
}

/// A RAM symbol read as true or false is its address, so it always holds
/// (lDEWkI, Ben 2026-11-09): refused wherever a condition takes its truth.
#[test]
fn a_bare_ram_symbol_as_a_truth_value_suggests_a_read() {
    let hint = refusal_hint("wFrame");
    assert!(
        hint.contains("[wFrame]") && hint.contains("u16[wFrame]"),
        "{hint}"
    );
    let hint = refusal_hint("wFrame && pc == Main");
    assert!(hint.contains("[wFrame] && pc == Main"), "{hint}");
    let hint = refusal_hint("pc == Main || wFrame");
    assert!(hint.contains("pc == Main || [wFrame]"), "{hint}");
    let hint = refusal_hint("!wFrame");
    assert!(hint.contains("![wFrame]"), "{hint}");
    let hint = refusal_hint("(wFrame)");
    assert!(hint.contains("([wFrame])"), "{hint}");
    // A logical operator under a comparison takes its operands' truth too
    // (review-fix of 655a334): `!wFrame == 0` is `(!$C000) == 0`.
    let hint = refusal_hint("!wFrame == 0");
    assert!(hint.contains("![wFrame] == 0"), "{hint}");
    let hint = refusal_hint("(wFrame || [wFrame] == 1) == 1");
    assert!(hint.contains("([wFrame] || [wFrame] == 1) == 1"), "{hint}");
    // The hint rewrites the bare occurrence, not one already read.
    let hint = refusal_hint("[wFrame] == 0 || wFrame");
    assert!(hint.starts_with("[wFrame] == 0 || [wFrame] or"), "{hint}");
}

/// The refusal leaves alone what is right: reads, ROM labels compared, and
/// a symbol as an operand of arithmetic or a value read.
#[test]
fn truth_positions_still_take_reads_and_rom_labels() {
    parse("[wFrame] && pc == Main");
    parse("![wFrame] || pc == Main.loop");
    parse("[wFrame + 1]");
    parse("Main");
    assert_eq!(
        Condition::compile_value("wFrame", Some(&syms()))
            .unwrap()
            .expr,
        Expr::Sym {
            name: "wFrame".into(),
            bank: 0,
            addr: 0xC000
        }
    );
    // A value read still refuses a comparison, which is a truth either way.
    assert!(Condition::compile_value("wFrame >= 60", Some(&syms())).is_err());
}
