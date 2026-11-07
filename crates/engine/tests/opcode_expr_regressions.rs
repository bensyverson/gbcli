//! The third review of the opcode lookup's constant expressions (leaf
//! iH2KOO, from review-fpq's probes): hostile input must neither crash nor
//! hang. The review's lines checked against rgbasm are rows of
//! `opcode_lookup_spellings.rs`, which rgbasm referees.

use gbcli_engine::disasm::{LookupError, Opcode, lookup};

/// 200,000 nested parentheses or unary minus signs overflowed the stack, an
/// abort no caller can catch (`opcode` is also a server op). Past the depth
/// cap the line is refused, and the message doesn't echo the line.
#[test]
fn deep_nesting_is_refused_without_overflowing_the_stack() {
    for line in [
        format!("rst {}8{}", "(".repeat(200_000), ")".repeat(200_000)),
        format!("rst {}8", "-".repeat(200_000)),
        format!("rst 2{}", " ** 2".repeat(200_000)),
        format!("bit {}3, a", "high(".repeat(200_000)),
    ] {
        let e = match lookup(&line) {
            Err(e) => e,
            Ok(found) => panic!("{}…: found {found:?}", &line[..20]),
        };
        assert!(matches!(e, LookupError::TooDeep { .. }), "{e:?}");
        assert!(e.to_string().len() < 200, "{e}");
        assert!(e.remedy().contains("64"), "{}", e.remedy());
    }
}

/// Nesting up to the cap still evaluates.
#[test]
fn nesting_up_to_the_cap_evaluates() {
    let line = format!("rst {}8{}", "(".repeat(60), ")".repeat(60));
    assert_eq!(lookup(&line).map(|f| f[0].opcode), Ok(Opcode::Base(0xCF)));
    let line = format!("rst {}8", "- ".repeat(60));
    assert_eq!(lookup(&line).map(|f| f[0].opcode), Ok(Opcode::Base(0xCF)));
}

/// A power was repeated multiplication: `1 ** $7FFFFFFF` took seconds.
#[test]
fn a_huge_power_answers_quickly() {
    let start = std::time::Instant::now();
    let _ = lookup("rst 1 ** $7FFFFFFF + 1 ** $7FFFFFFF + 1 ** $7FFFFFFF + 1 ** $7FFFFFFF");
    let took = start.elapsed();
    assert!(took.as_secs_f64() < 0.5, "took {took:?}");
}

/// `x * 0` computed a remainder by zero before it looked at the operator,
/// and panicked. rgbasm: `rst 8*0` is rst $00 ($C7), `bit 3*0, a` is CB 47.
#[test]
fn multiplying_by_zero_does_not_panic() {
    for (line, want) in [
        ("rst 8*0", Opcode::Base(0xC7)),
        ("bit 3*0, a", Opcode::Cb(0x47)),
    ] {
        let got = std::panic::catch_unwind(|| lookup(line).map(|f| f[0].opcode));
        assert!(matches!(got, Ok(Ok(op)) if op == want), "{line}: {got:?}");
    }
}

/// Short random lines over the characters the lookup treats specially:
/// none may panic. 5,000 lines keep it near 200 ms in the warm suite;
/// 200,000 (the review's count, about 8 s) ran clean once after these
/// fixes, and the alphabet now has `\` and `{` too.
#[test]
fn short_random_lines_never_panic() {
    let alphabet: Vec<char> = "ab hl,()[]'\"$%&01789+-*/<>=!~^|_;é\t:.c\\{"
        .chars()
        .collect();
    let words = [
        "ld ", "bit ", "rst ", "jr ", "ldi ", "ldh ", "high(", "low(", "**", ">>>",
    ];
    let mut seed: u64 = 0x9E37_79B9_7F4A_7C15;
    let mut next = || {
        seed ^= seed << 13;
        seed ^= seed >> 7;
        seed ^= seed << 17;
        seed
    };
    let mut panics = Vec::new();
    for _ in 0..5_000 {
        let mut line = String::from(words[(next() % 5) as usize]);
        for _ in 0..(next() % 14) {
            if next() % 5 == 0 {
                line.push_str(words[(next() % words.len() as u64) as usize]);
            } else {
                line.push(alphabet[(next() % alphabet.len() as u64) as usize]);
            }
        }
        let l = line.clone();
        if std::panic::catch_unwind(move || {
            let _ = lookup(&l);
        })
        .is_err()
        {
            panics.push(line);
            if panics.len() > 10 {
                break;
            }
        }
    }
    assert!(panics.is_empty(), "{panics:#?}");
}
