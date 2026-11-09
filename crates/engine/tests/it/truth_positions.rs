//! Edge cases from review-fonts (d6c136c): a RAM symbol taken as true or false is
//! refused anywhere in a condition; values and valid conditions still
//! compile.

use gbcli_engine::Symbols;
use gbcli_engine::cond::Condition;

fn syms() -> Symbols {
    Symbols::parse("00:C000 wFrame\n00:C010 wTable\n01:4000 Data\n").unwrap()
}

fn refused(text: &str) -> bool {
    Condition::compile(text, Some(&syms())).is_err()
}

#[test]
fn truth_positions_are_refused_everywhere() {
    for text in [
        "wFrame",
        "!wFrame",
        "!!wFrame",
        "wFrame && [wFrame] == 1",
        "[wFrame] == 1 || wFrame",
        "!wFrame == 0",
        "(wFrame || 0) == 1",
        "([wFrame] == 1) + (!wFrame) == 1",
        "-(!wFrame) == 0",
        "wFrame == 1",
        "1 < wFrame",
    ] {
        assert!(refused(text), "accepted: {text}");
    }
}

#[test]
fn truth_positions_inside_a_read_address() {
    // A comparison or a logical operator inside a read's address takes the
    // symbol's truth or compares its address just the same.
    let mut accepted = Vec::new();
    for text in [
        "[wTable + (wFrame == 0)] == 1",
        "[wTable + !wFrame] == 1",
        "[wTable + (wFrame && 1)] == 1",
    ] {
        if !refused(text) {
            accepted.push(text);
        }
    }
    assert!(accepted.is_empty(), "accepted: {accepted:?}");
}

#[test]
fn valid_conditions_and_values_still_compile() {
    for text in [
        "[wFrame] == 0",
        "![wFrame]",
        "[wFrame] && u16[wTable] > 3",
        "[wTable + 1] == 2",
        "Data == $4000",
        "!Data",
        "pc == Data",
        "a == 3 && !zf",
    ] {
        Condition::compile(text, Some(&syms())).unwrap_or_else(|e| panic!("{text}: {e:?}"));
    }
    // --read prints a bare symbol as its address.
    for text in ["wFrame", "wFrame + 1", "wTable - wFrame"] {
        Condition::compile_value(text, Some(&syms())).unwrap_or_else(|e| panic!("{text}: {e:?}"));
    }
}
