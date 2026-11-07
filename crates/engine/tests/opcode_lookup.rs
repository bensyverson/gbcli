//! `disasm::lookup`: an opcode's size, cycles and flags from the sourced
//! table, found by byte or by the instruction as RGBDS spells it.

use gbcli_engine::disasm::{Cycles, Form, LookupError, Opcode, form, lookup};

fn one(query: &str) -> Form {
    let found = lookup(query).unwrap_or_else(|e| panic!("{query}: {e}"));
    assert_eq!(found.len(), 1, "{query}: {found:?}");
    found.into_iter().next().unwrap()
}

fn codes(query: &str) -> Vec<Opcode> {
    lookup(query)
        .unwrap_or_else(|e| panic!("{query}: {e}"))
        .into_iter()
        .map(|f| f.opcode)
        .collect()
}

#[test]
fn a_byte_finds_its_opcode_with_both_branch_costs() {
    let f = one("$20");
    assert_eq!(f.opcode, Opcode::Base(0x20));
    assert_eq!(f.asm, "jr nz, e8");
    assert_eq!(f.len, 2);
    assert_eq!(
        f.cycles,
        Cycles {
            taken: 12,
            not_taken: Some(8)
        }
    );
    assert_eq!(f.flags, "----");
    assert_eq!(one("0x20").opcode, Opcode::Base(0x20));
}

#[test]
fn a_cb_prefixed_opcode_is_written_with_its_prefix() {
    let f = one("$CB37");
    assert_eq!(f.opcode, Opcode::Cb(0x37));
    assert_eq!(f.asm, "swap a");
    assert_eq!((f.len, f.cycles.taken, f.flags), (2, 8, "Z000"));
    assert_eq!(one("0xcb37").opcode, Opcode::Cb(0x37));
    let bit = one("$CB7E");
    assert_eq!(bit.asm, "bit 7, [hl]");
    assert_eq!((bit.cycles.taken, bit.flags), (12, "Z01-"));
}

#[test]
fn the_prefix_and_the_illegal_bytes_say_what_they_are() {
    assert_eq!(one("$CB").asm, "prefix");
    let d3 = one("$D3");
    assert_eq!(d3.asm, "illegal");
    assert_eq!(d3.len, 1);
}

#[test]
fn a_mnemonic_lists_every_form_in_opcode_order() {
    assert_eq!(
        codes("jr"),
        [0x18, 0x20, 0x28, 0x30, 0x38].map(Opcode::Base).to_vec()
    );
    let ret: Vec<String> = lookup("ret").unwrap().into_iter().map(|f| f.asm).collect();
    assert_eq!(ret, ["ret nz", "ret z", "ret", "ret nc", "ret c"]);
    assert_eq!(lookup("ld").unwrap().len(), 88);
}

#[test]
fn case_and_spacing_dont_matter() {
    assert_eq!(one("JR  NZ, .Loop").opcode, Opcode::Base(0x20));
    assert_eq!(one("ld a,[ hl + ]").opcode, Opcode::Base(0x2A));
}

#[test]
fn hli_and_hld_are_the_increment_forms() {
    assert_eq!(one("ld a, [hli]").opcode, Opcode::Base(0x2A));
    assert_eq!(one("ld [hld], a").opcode, Opcode::Base(0x32));
    assert_eq!(one("ld a, [hl+]").asm, "ld a, [hl+]");
}

#[test]
fn a_line_of_real_code_finds_its_form() {
    assert_eq!(one("ld a, 5").asm, "ld a, n8");
    assert_eq!(one("ld hl, wBuffer").asm, "ld hl, n16");
    assert_eq!(one("ld [wFoo], a").asm, "ld [a16], a");
    assert_eq!(one("ldh [rLCDC], a").asm, "ldh [a8], a");
    assert_eq!(one("ldh [c], a").opcode, Opcode::Base(0xE2));
    assert_eq!(one("ld [hl], $FF").asm, "ld [hl], n8");
    assert_eq!(one("jr nz, .loop").opcode, Opcode::Base(0x20));
    assert_eq!(one("add sp, -2").asm, "add sp, e8");
    let call = one("call nz, Update");
    assert_eq!((call.len, call.cycles.taken), (3, 24));
    assert_eq!(call.cycles.not_taken, Some(12));
}

/// Labels the spellings table can't referee, since rgbasm needs them
/// defined around the line: anonymous labels, a scoped local label, a raw
/// identifier, `BANK(@)`. Data is read as an expression, and each of these
/// is a symbol.
#[test]
fn every_kind_of_label_is_data() {
    assert_eq!(one("jr :+").opcode, Opcode::Base(0x18));
    assert_eq!(one("jr nz, :--").opcode, Opcode::Base(0x20));
    assert_eq!(one("jp Top.loop").asm, "jp a16");
    assert_eq!(one("ld hl, #ld").asm, "ld hl, n16");
    assert_eq!(one("ld a, BANK(@)").asm, "ld a, n8");
}

/// An interpolation can expand to anything, a register included, so it is
/// refused rather than guessed at.
#[test]
fn an_interpolation_is_refused() {
    assert!(lookup("ld a, {wFoo}").is_err());
}

#[test]
fn sp_plus_an_offset_is_one_operand() {
    let f = one("ld hl, sp+5");
    assert_eq!(f.opcode, Opcode::Base(0xF8));
    assert_eq!(f.asm, "ld hl, sp+e8");
    assert_eq!(f.flags, "00HC");
    assert_eq!(one("ld hl, sp - 2").opcode, Opcode::Base(0xF8));
}

#[test]
fn rst_and_bit_numbers_compare_by_value() {
    assert_eq!(one("rst $38").opcode, Opcode::Base(0xFF));
    assert_eq!(one("rst 56").opcode, Opcode::Base(0xFF));
    assert_eq!(one("bit 7, h").opcode, Opcode::Cb(0x7C));
    assert_eq!(one("set 0, [hl]").cycles.taken, 16);
}

#[test]
fn sixteen_bit_inc_and_dec_leave_the_flags_alone() {
    assert_eq!(one("inc bc").flags, "----");
    assert_eq!(one("dec b").flags, "Z1H-");
}

#[test]
fn an_unknown_mnemonic_names_the_near_ones() {
    let e = lookup("lda a, 5").unwrap_err();
    assert!(matches!(&e, LookupError::NoMnemonic { word, .. } if word == "lda"));
    assert!(e.remedy().contains("ld"), "{}", e.remedy());
    assert!(lookup("illegal").is_err(), "not an instruction");
}

/// The mnemonic ends where its name does (`rl[hl]`), so a line that starts
/// with no name at all is named as written, not as an empty word.
#[test]
fn a_line_with_no_mnemonic_is_named_as_written() {
    let e = lookup("[hl], a").unwrap_err();
    assert_eq!(e.to_string(), "no instruction `[hl],`");
}

#[test]
fn a_known_mnemonic_with_no_such_form_says_how_to_list_them() {
    let e = lookup("ld q, a").unwrap_err();
    assert!(matches!(e, LookupError::NoForm { .. }), "{e:?}");
    assert!(e.remedy().contains("gbcli opcode ld"), "{}", e.remedy());
}

#[test]
fn a_byte_must_be_one_byte_or_cb_and_one() {
    for bad in ["$100", "$CB1", "0x", "$", "$céb", "$+1"] {
        assert!(
            matches!(lookup(bad), Err(LookupError::BadByte { .. })),
            "{bad}"
        );
    }
    assert!(matches!(lookup("  "), Err(LookupError::Empty)));
}

#[test]
fn form_names_any_opcode() {
    assert_eq!(form(Opcode::Base(0x00)).asm, "nop");
    assert_eq!(form(Opcode::Base(0x08)).asm, "ld [a16], sp");
    assert_eq!(form(Opcode::Base(0xE9)).asm, "jp hl");
    assert_eq!(form(Opcode::Base(0xF2)).asm, "ldh a, [c]");
    assert_eq!(form(Opcode::Base(0x10)).asm, "stop n8");
}
