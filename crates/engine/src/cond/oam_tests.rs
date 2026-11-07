//! The structured OAM forms (design §7): `oam[N].y|x|tile|attr` is one byte
//! of OAM wherever an expression takes a value, and a bare `oam[N]` is a
//! whole entry, which only a read prints.

use super::*;

fn parse(text: &str) -> Expr {
    Condition::compile(text, None)
        .unwrap_or_else(|e| panic!("{text}: {e:?}"))
        .expr
}

fn oam_byte(offset: u32) -> Expr {
    Expr::Read {
        width: Width::U8,
        place: Place::Region {
            region: Region::Oam,
            offset,
        },
    }
}

/// The syntax error's message and hint.
fn refused(text: &str) -> (String, Option<String>) {
    match Condition::compile(text, None) {
        Err(ExprError::Syntax { message, hint, .. }) => (message, hint),
        other => panic!("{text}: {other:?}"),
    }
}

#[test]
fn a_field_is_one_byte_of_the_entry() {
    assert_eq!(parse("oam[0].y"), oam_byte(0));
    assert_eq!(parse("oam[0].x"), oam_byte(1));
    assert_eq!(parse("oam[0].tile"), oam_byte(2));
    assert_eq!(parse("oam[0].attr"), oam_byte(3));
    assert_eq!(parse("oam[39].attr"), oam_byte(39 * 4 + 3));
}

#[test]
fn the_index_is_an_integer_in_any_radix() {
    assert_eq!(parse("oam[$27].x"), oam_byte(39 * 4 + 1));
    assert_eq!(parse("oam[%11].y"), oam_byte(12));
    assert_eq!(parse("oam[ 2 ] .tile"), oam_byte(10));
}

#[test]
fn a_field_takes_part_in_an_expression() {
    assert_eq!(
        parse("oam[1].x > 80 && oam[1].attr & $20"),
        Expr::Binary(
            BinOp::And,
            Box::new(Expr::Binary(
                BinOp::Gt,
                Box::new(oam_byte(5)),
                Box::new(Expr::Num(80))
            )),
            Box::new(Expr::Binary(
                BinOp::BitAnd,
                Box::new(oam_byte(7)),
                Box::new(Expr::Num(0x20))
            ))
        )
    );
}

#[test]
fn an_index_past_the_last_entry_is_refused_naming_the_range() {
    let (message, hint) = refused("oam[40].x");
    assert!(message.contains("oam[0] to oam[39]"), "{message}");
    assert_eq!(hint.as_deref(), Some("oam[39].x"));
}

#[test]
fn an_index_that_isnt_a_number_is_refused() {
    let (message, _) = refused("oam[hl].x");
    assert!(message.contains("oam[0] to oam[39]"), "{message}");
}

#[test]
fn an_unknown_field_is_refused_naming_the_fields() {
    let (message, hint) = refused("oam[3].flags");
    assert!(message.contains(".flags"), "{message}");
    let hint = hint.unwrap();
    for field in ["oam[3].y", "oam[3].x", "oam[3].tile", "oam[3].attr"] {
        assert!(hint.contains(field), "{hint}");
    }
}

#[test]
fn a_whole_entry_in_an_expression_is_refused_naming_the_fields() {
    let (message, hint) = refused("oam[0] == 3");
    assert!(message.contains("whole entry"), "{message}");
    assert!(hint.unwrap().contains("oam[0].y"));
}

#[test]
fn a_whole_entry_alone_is_what_a_read_prints_as_fields() {
    assert_eq!(oam_entry("oam[5]"), Ok(Some(5)));
    assert_eq!(oam_entry(" oam[ $27 ] "), Ok(Some(39)));
    assert_eq!(oam_entry("oam[5].x"), Ok(None));
    assert_eq!(oam_entry("oam[5] + 1"), Ok(None));
    assert_eq!(oam_entry("[$C000]"), Ok(None));
    assert!(oam_entry("oam[40]").is_err());
}

#[test]
fn a_symbol_named_oam_still_reads_as_a_symbol() {
    let syms = Symbols::parse("00:c100 oam\n").unwrap();
    let expr = Condition::compile("[oam]", Some(&syms)).unwrap().expr;
    assert!(
        matches!(
            &expr,
            Expr::Read {
                place: Place::Banked { addr: 0xC100, .. },
                ..
            } | Expr::Read {
                place: Place::Cpu(_),
                ..
            }
        ),
        "{expr:?}"
    );
}

#[test]
fn a_field_runs_on_every_instruction_as_any_oam_read_does() {
    // DMA writes OAM without the write hook, so no store trigger covers it.
    let cadence = |text: &str| {
        cadence::analyze(
            &parse(text),
            gbcli_core::Support::Yes,
            crate::mem::CartSizes::default(),
        )
        .cadence()
    };
    assert_eq!(
        cadence("oam[0].x == 1"),
        gbcli_protocol::Cadence::EveryInstruction
    );
}
