//! Hardware register names in the address grammar (design §7): always
//! available, from inspection's table, as conditions name them.

use super::*;
use crate::inspect::hwreg::HW_REGISTERS;
use crate::sym::Symbols;

fn fixed(addr: u16) -> Location {
    Location {
        addr,
        bank: None,
        symbol: None,
    }
}

#[test]
fn a_hardware_register_needs_no_sym() {
    assert_eq!(parse("rDMA", None), Ok(fixed(0xFF46)));
    assert_eq!(parse("rLY", None), Ok(fixed(0xFF44)));
    assert_eq!(parse("rIE", None), Ok(fixed(0xFFFF)));
}

#[test]
fn every_register_name_and_alias_parses_to_its_address() {
    for reg in HW_REGISTERS {
        for name in std::iter::once(&reg.name).chain(reg.aliases) {
            assert_eq!(parse(name, None), Ok(fixed(reg.addr)), "{name}");
        }
    }
}

#[test]
fn a_register_takes_an_offset() {
    assert_eq!(parse("_AUD3WAVERAM+3", None), Ok(fixed(0xFF33)));
    assert_eq!(parse("rLCDC+1", None), Ok(fixed(0xFF41)));
    // Offsets are hex, as for symbols; only the table's own names
    // (`_AUD3WAVERAM+10`, FF3A) read as hardware.inc spells them.
    assert_eq!(parse("rLCDC+10", None), Ok(fixed(0xFF50)));
    assert_eq!(parse("_AUD3WAVERAM+10", None), Ok(fixed(0xFF3A)));
}

/// Conditions take the same order (`cond::tests`).
#[test]
fn a_register_shadows_a_sym_symbol_of_the_same_name() {
    let s = Symbols::parse("00:C000 rDMA\n00:C001 rdma\n").unwrap();
    assert_eq!(parse("rDMA", Some(&s)), Ok(fixed(0xFF46)));
    // A symbol that differs only in case is not a register name.
    assert_eq!(parse("rdma", Some(&s)).unwrap().addr, 0xC001);
}

#[test]
fn a_write_target_takes_a_register_name() {
    assert_eq!(
        parse_write("rDMA", None),
        Ok(WriteTarget::Cpu(fixed(0xFF46)))
    );
}
