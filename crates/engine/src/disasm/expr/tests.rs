use super::{MAX_DEPTH, Refused, Value, eval};

/// The value, when it is known.
fn value(text: &str) -> Option<i32> {
    match eval(text) {
        Ok(Value::Known(v)) => Some(v),
        _ => None,
    }
}

#[test]
fn numbers_in_every_base_and_characters() {
    for (text, v) in [
        ("56", 56),
        ("$38", 56),
        ("0x38", 56),
        ("%111000", 56),
        ("0b111000", 56),
        ("&70", 56),
        ("0o70", 56),
        ("5_6", 56),
        ("$_38", 56),
        ("'8'", 56),
        ("'C'", 67),
        ("\"8\"", 56),
        ("' '", 32),
        ("'\\n'", 10),
        ("'\\''", 39),
        ("'\\{'", 123),
        ("'}'", 125),
    ] {
        assert_eq!(value(text), Some(v), "{text}");
    }
    for text in [
        "", "_5", "$", "5x", "wFoo", "'ab'", "''", "'{'", "'\\x'", "\"ab\"", "5_", "5__6", "$__3",
        "$ 3",
    ] {
        assert_eq!(value(text), None, "{text}");
    }
}

#[test]
fn operators_take_rgbasms_precedence() {
    for (text, v) in [
        ("8*7", 56),
        ("8*0", 0),
        ("1+1*2", 3),
        // `&` binds tighter than `+`, unlike C.
        ("2+7 & 1", 3),
        ("(2+7) & 1", 1),
        ("1<<3|1", 9),
        // `**` binds tighter than unary minus, and to the right.
        ("-2**2", -4),
        ("2**3**2", 512),
        ("8**!0", 8),
        ("2**+3", 8),
        ("0**0", 1),
        ("-1%8", 7),
        ("7%-3", -2),
        ("-7/2", -4),
        ("~0", -1),
        ("!0", 1),
        ("1==1&&2>1", 1),
        ("-8>>1", -4),
        ("-8>>>28", 15),
        ("1>>-1", 2),
        ("4<<-1", 2),
        ("high($3812)", 0x38),
        ("LOW ( $3812 )", 0x12),
        ("2 ** $7FFFFFFF", 0),
    ] {
        assert_eq!(value(text), Some(v), "{text}");
    }
    for text in [
        "8/0", "8%0", "2**-1", "(1", "1)", "1+", "7 & & 1", "3 * * 1", "high(5", "bank(5)",
    ] {
        assert_eq!(value(text), None, "{text}");
    }
}

#[test]
fn nesting_past_the_cap_is_refused_as_too_deep() {
    let at = format!("{}1{}", "(".repeat(MAX_DEPTH), ")".repeat(MAX_DEPTH));
    assert_eq!(eval(&at), Ok(Value::Known(1)));
    let past = format!(
        "{}1{}",
        "(".repeat(MAX_DEPTH + 1),
        ")".repeat(MAX_DEPTH + 1)
    );
    assert_eq!(eval(&past), Err(Refused::TooDeep));
    assert_eq!(eval(&"-".repeat(100_000)), Err(Refused::TooDeep));
}

/// rgbasm's lexer reads `&` before an octal digit or `_` as an octal number,
/// and `%` before 0, 1 or `_` as a binary one: `15&7` is two numbers.
#[test]
fn a_prefix_glued_to_a_digit_is_a_number_not_an_operator() {
    for text in [
        "15&7", "7 &7", "7&_7", "5%1", "7 %1", "7%_1", "7%0", "$38&56", "2+7&1", "(2+7)&1", "7%0b1",
    ] {
        assert_eq!(eval(text), Err(Refused::Invalid), "{text}");
    }
    for (text, v) in [
        ("7 & 7", 7),
        ("7& 7", 7),
        ("7&8", 0),
        ("7&&1", 1),
        ("5%2", 1),
        ("7%8", 7),
        ("7% 1", 0),
        ("&7", 7),
        ("%1", 1),
        ("1+&7", 8),
        ("7&$1", 1),
        ("7%%1", 0),
    ] {
        assert_eq!(value(text), Some(v), "{text}");
    }
}

/// rgbasm 1.0.4 makes a number wider than 32 bits 0, with a warning
/// (-Wlarge-constant).
#[test]
fn a_number_wider_than_32_bits_is_zero() {
    for text in [
        "$100000038",
        "4294967296",
        "$1_0000_0003",
        "$10000000000000003",
        "%100000000000000000000000000000001",
        "&777777777777",
        "0x1FFFFFFFF",
    ] {
        assert_eq!(value(text), Some(0), "{text}");
    }
    assert_eq!(value("$FFFFFFFF"), Some(-1));
    assert_eq!(value("4294967295"), Some(-1));
}

/// A symbol, a function other than HIGH() and LOW(), a fixed-point number
/// and a graphics constant have no value here, but the expression around
/// them is still read as rgbasm reads it.
#[test]
fn symbols_have_no_value_but_their_syntax_is_checked() {
    for text in [
        "wFoo",
        "wFoo + 1",
        "Top.loop",
        ".loop",
        "_x",
        "#ld",
        "x@y",
        "@",
        "@+3",
        ":+",
        ":--",
        "high(wFoo)",
        "low(wFoo) + 1",
        "2 ** wFoo",
        "!wFoo",
        "BANK(Top)",
        "bank (@)",
        "STRCMP(\"a\", \"b\")",
        "SIZEOF(\"s\")",
        "MUL(6.0, 7.0, 8)",
        "1.5",
        "5.",
        "1.5q8",
        "5.5_5",
        "`01012323",
        "`0123_0123",
    ] {
        assert_eq!(eval(text), Ok(Value::Unknown), "{text}");
    }
    for text in [
        "wFoo +",
        "wFoo wFoo",
        "(wFoo",
        "wFoo % 0",
        "wFoo / 0",
        "wFoo ** -1",
        "FOO(1)",
        "BANK",
        "BANK()",
        "BANK(1 +)",
        "LOW(Top, 1)",
        "Top.",
        "a",
        "7 & a",
        "HL + 1",
        "nz",
        "5.q",
        "5._5",
        "5_.5",
        "0x5.5",
        "1.5e",
        "`0124",
        "`",
        "`012345678",
        ":",
        "'x",
        "\"ab\"",
    ] {
        assert_eq!(eval(text), Err(Refused::Invalid), "{text}");
    }
}
