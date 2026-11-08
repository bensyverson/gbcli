//! The disassembler against the sourced opcode table (crates/engine/data/
//! gb-opcodes/Opcodes.json) for all 256 base and 256 CB-prefixed opcodes.

use gbcli_engine::disasm::{Mnemonic, NoSymbols, Opcode, decode, form};
use serde_json::Value;

const TABLE: &str = include_str!("../../data/gb-opcodes/Opcodes.json");

fn check(entry: &Value, bytes: &[u8], what: &str) {
    let ins = decode(bytes, 0x0150, &NoSymbols).unwrap_or_else(|e| panic!("{what}: {e:?}"));
    let mnemonic = entry["mnemonic"].as_str().unwrap();
    if mnemonic.starts_with("ILLEGAL_") {
        assert!(ins.illegal, "{what} is illegal");
        assert_eq!(ins.mnemonic, Mnemonic::Illegal, "{what}");
    } else {
        assert!(!ins.illegal, "{what} is legal");
        assert_eq!(ins.mnemonic.as_str(), mnemonic, "{what}");
    }
    assert_eq!(
        u64::from(ins.len),
        entry["bytes"].as_u64().unwrap(),
        "{what} length"
    );
    let cycles: Vec<u64> = entry["cycles"]
        .as_array()
        .unwrap()
        .iter()
        .map(|c| c.as_u64().unwrap())
        .collect();
    let ours: Vec<u64> = std::iter::once(ins.cycles.taken)
        .chain(ins.cycles.not_taken)
        .map(u64::from)
        .collect();
    assert_eq!(ours, cycles, "{what} cycles");
    let flags: String = ["Z", "N", "H", "C"]
        .iter()
        .map(|f| entry["flags"][f].as_str().unwrap())
        .collect();
    assert_eq!(form(ins.opcode).flags, flags, "{what} flags");
    let operands = entry["operands"].as_array().unwrap();
    assert_eq!(ins.operands.len(), operands.len(), "{what} operand count");
    for (ours, theirs) in ins.operands.iter().zip(operands) {
        let flag = |key: &str| theirs.get(key).and_then(Value::as_bool).unwrap_or(false);
        assert_eq!(ours.kind.name(), theirs["name"].as_str().unwrap(), "{what}");
        assert_eq!(
            ours.indirect,
            !flag("immediate"),
            "{what} {}",
            ours.kind.name()
        );
        assert_eq!(ours.increment, flag("increment"), "{what}");
        assert_eq!(ours.decrement, flag("decrement"), "{what}");
    }
}

#[test]
fn all_512_opcodes_match_the_sourced_table() {
    let table: Value = serde_json::from_str(TABLE).unwrap();
    for op in 0..=255u8 {
        let key = format!("0x{op:02X}");
        let base = &table["unprefixed"][&key];
        let cb = &table["cbprefixed"][&key];
        assert!(
            base.is_object() && cb.is_object(),
            "{key} missing from the table"
        );
        if op != 0xCB {
            check(base, &[op, 0x34, 0x12], &format!("base {key}"));
        }
        let ins = decode(&[0xCB, op], 0x0150, &NoSymbols).unwrap();
        assert_eq!(ins.opcode, Opcode::Cb(op));
        check(cb, &[0xCB, op], &format!("CB {key}"));
    }
}

#[test]
fn the_prefix_byte_alone_is_not_an_instruction() {
    assert!(decode(&[0xCB], 0, &NoSymbols).is_err());
}

#[test]
fn eleven_opcodes_are_illegal() {
    let illegal: Vec<u8> = (0..=255u8)
        .filter(|&op| op != 0xCB && decode(&[op, 0, 0], 0, &NoSymbols).unwrap().illegal)
        .collect();
    assert_eq!(
        illegal,
        [
            0xD3, 0xDB, 0xDD, 0xE3, 0xE4, 0xEB, 0xEC, 0xED, 0xF4, 0xFC, 0xFD
        ]
    );
}
