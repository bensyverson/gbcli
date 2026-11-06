//! Operand values, branch targets and symbol annotation.

use gbcli_engine::disasm::{
    DecodeError, NoSymbols, Opcode, OperandKind, SymbolLookup, SymbolRef, Value, decode,
};

struct Fake;

impl SymbolLookup for Fake {
    fn symbol(&self, addr: u16) -> Option<SymbolRef> {
        match addr {
            0x1234 => Some(SymbolRef {
                name: "Init".into(),
                offset: 0,
            }),
            0x0150..=0x015F => Some(SymbolRef {
                name: "Main".into(),
                offset: addr - 0x0150,
            }),
            0xFF80 => Some(SymbolRef {
                name: "hFrame".into(),
                offset: 0,
            }),
            _ => None,
        }
    }
}

#[test]
fn immediates_are_little_endian_values() {
    let ins = decode(&[0x01, 0x34, 0x12], 0x0150, &NoSymbols).unwrap(); // LD BC, n16
    assert_eq!(ins.opcode, Opcode::Base(0x01));
    assert_eq!(ins.operands[0].kind, OperandKind::Bc);
    assert_eq!(ins.operands[1].value, Some(Value::U16(0x1234)));
    assert_eq!(ins.bytes, [0x01, 0x34, 0x12]);
    let ins = decode(&[0x3E, 0x7F], 0, &NoSymbols).unwrap(); // LD A, n8
    assert_eq!(ins.operands[1].value, Some(Value::U8(0x7F)));
}

#[test]
fn relative_jumps_resolve_their_target_and_symbol() {
    // JR NZ, -2 at $0152: the target is $0152 + 2 - 2 = $0152.
    let ins = decode(&[0x20, 0xFE], 0x0152, &Fake).unwrap();
    let target = &ins.operands[1];
    assert_eq!(target.value, Some(Value::I8(-2)));
    assert_eq!(target.address, Some(0x0152));
    assert_eq!(
        target.symbol,
        Some(SymbolRef {
            name: "Main".into(),
            offset: 2
        })
    );
    assert_eq!(ins.operands[0].kind, OperandKind::CondNz);
}

#[test]
fn absolute_addresses_and_vectors_get_symbols() {
    let call = decode(&[0xCD, 0x34, 0x12], 0, &Fake).unwrap();
    assert_eq!(call.operands[0].address, Some(0x1234));
    assert_eq!(call.operands[0].symbol.as_ref().unwrap().name, "Init");
    let rst = decode(&[0xC7], 0, &NoSymbols).unwrap();
    assert_eq!(rst.operands[0].address, Some(0x0000));
    let ld = decode(&[0x21, 0x34, 0x12], 0, &Fake).unwrap(); // LD HL, n16: a constant
    assert_eq!(ld.operands[1].symbol, None);
}

#[test]
fn high_page_operands_fall_back_to_hardware_register_names() {
    let ldh = decode(&[0xE0, 0x40], 0, &NoSymbols).unwrap(); // LDH [a8], A
    assert_eq!(ldh.operands[0].address, Some(0xFF40));
    assert_eq!(ldh.operands[0].symbol.as_ref().unwrap().name, "rLCDC");
    let hram = decode(&[0xF0, 0x80], 0, &Fake).unwrap(); // LDH A, [a8]
    assert_eq!(hram.operands[1].symbol.as_ref().unwrap().name, "hFrame");
    let ld = decode(&[0xEA, 0xFF, 0xFF], 0, &NoSymbols).unwrap(); // LD [a16], A
    assert_eq!(ld.operands[0].symbol.as_ref().unwrap().name, "rIE");
}

#[test]
fn conditional_branches_report_taken_and_not_taken_cycles() {
    let jp = decode(&[0xC2, 0, 0], 0, &NoSymbols).unwrap();
    assert_eq!((jp.cycles.taken, jp.cycles.not_taken), (16, Some(12)));
    let nop = decode(&[0x00], 0, &NoSymbols).unwrap();
    assert_eq!((nop.cycles.taken, nop.cycles.not_taken), (4, None));
}

#[test]
fn truncated_input_says_how_many_bytes_it_needs() {
    assert_eq!(
        decode(&[0xC3, 0x00], 0, &NoSymbols),
        Err(DecodeError::Truncated { need: 3, have: 2 })
    );
    assert_eq!(
        decode(&[], 0, &NoSymbols),
        Err(DecodeError::Truncated { need: 1, have: 0 })
    );
}

#[test]
fn the_c_register_and_the_c_condition_are_told_apart() {
    let ret_c = decode(&[0xD8], 0, &NoSymbols).unwrap();
    assert_eq!(ret_c.operands[0].kind, OperandKind::CondC);
    let ld_c = decode(&[0x0E, 0], 0, &NoSymbols).unwrap();
    assert_eq!(ld_c.operands[0].kind, OperandKind::C);
    assert_eq!(ret_c.operands[0].kind.name(), ld_c.operands[0].kind.name());
}
