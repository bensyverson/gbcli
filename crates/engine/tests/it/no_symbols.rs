//! A name on a machine with no `.sym` at all: not a typo to check against
//! `gbcli sym`, but a missing file, said as such with the way to load one.

use gbcli_engine::addr::{AddrError, parse};
use gbcli_engine::cond::{Condition, ExprError};
use gbcli_protocol::ErrorCode;

#[test]
fn an_address_naming_a_symbol_with_no_sym_loaded_says_the_file_is_missing() {
    let err = parse("Update", None).unwrap_err();
    assert_eq!(
        err,
        AddrError::NoSymbols {
            name: "Update".into()
        }
    );
    assert_eq!(err.code(), ErrorCode::UnknownSymbol);
    assert_eq!(
        err.to_string(),
        "unknown symbol \"Update\": no .sym file is loaded"
    );
    assert!(err.remedy().contains("--sym FILE"), "{}", err.remedy());
    assert!(
        err.remedy().contains("the ROM's name with .sym"),
        "{}",
        err.remedy()
    );
}

#[test]
fn an_expression_naming_a_symbol_with_no_sym_loaded_says_the_file_is_missing() {
    let err = Condition::compile("[wFrame] >= 60", None).unwrap_err();
    assert_eq!(
        err,
        ExprError::NoSymbols {
            name: "wFrame".into()
        }
    );
    assert_eq!(
        err.to_string(),
        "unknown symbol \"wFrame\": no .sym file is loaded"
    );
    assert!(err.remedy().contains("--sym FILE"), "{}", err.remedy());
    assert_eq!(gbcli_engine::expr_code(&err), ErrorCode::UnknownSymbol);
}
