//! Parser tests; evaluation is tested against a real core in tests/conditions.rs.

use super::*;

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
                offset: num(0x1A0)
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
    assert!(matches!(
        Condition::compile("[wFrame]", None),
        Err(ExprError::UnknownSymbol { .. })
    ));
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
