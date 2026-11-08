//! The fourth review's probes of the opcode lookup (leaf radJxi, from
//! review-clock's probes of 273f199), kept as regression tests:
//! `disasm::lookup` against rgbasm and rgblink 1.0.4 on lines beyond
//! `opcode_lookup_spellings.rs`'s table, and its time on long operands.
//! Each rgbasm probe lists every line where the two disagree. Needs RGBDS
//! 1.0.4 on `PATH`, as `opcode_lookup_spellings.rs` does.
//!
//! Its own test binary, not a module of `it/`: its lookups must finish under a
//! wall-clock bound, which the engine's emulation tests running beside it in
//! one process can push it past.

#[path = "it/rgbasm/mod.rs"]
mod rgbasm;

use gbcli_engine::disasm::{LookupError, lookup};

/// `&` before an octal digit and `%` before a binary digit start a number
/// in rgbasm's lexer, so `15&7` is `15` then the octal `&7`.
#[test]
fn probe_prefix_characters_glued_to_an_operand() {
    let wrong = rgbasm::disagreements(&[
        "res 15&7, a",
        "bit 7&7, a",
        "rst $38&56",
        "bit 5%1, a",
        "bit 5%2, a",
        "bit 7%8, a",
        "bit 7 %1, a",
        "bit 7% 1, a",
        "bit 7&&1, a",
        "bit 7&_7, a",
        "bit 7%_1, a",
        "bit 7 & 7, a",
        "bit 5 % 1, a",
    ]);
    assert!(wrong.is_empty(), "{}", wrong.join("\n"));
}

/// Numbers wider than 32 bits.
#[test]
fn probe_wide_numbers() {
    let wrong = rgbasm::disagreements(&[
        "rst $100000038",
        "rst 4294967352",
        "bit 4294967297, a",
        "bit $1_0000_0003, a",
        "bit $10000000000000003, a",
        "bit %100000000000000000000000000000001, a",
    ]);
    assert!(wrong.is_empty(), "{}", wrong.join("\n"));
}

/// Value and address slots still match on the whitespace-free, lowercased
/// `norm` and check only `literal`, not the evaluator.
#[test]
fn probe_value_and_address_slots() {
    let wrong = rgbasm::disagreements(&[
        "ld a, $ 38",
        "jp $ 100",
        "ld a, [$c0 00]",
        "ldh [$ff 40], a",
        "ld a, 5_",
        "ld a, 1 +",
        "ld a, )(",
        "ld a, 'x",
        "ld a, \"ab\"",
        "ld a, 'ab'",
        "ld a, ''",
        "ldh [$100+$80], a",
        "ldh [$fe00+$180], a",
        "ld a, [$fe00+$100+c]",
        "ld a, [$ff_00+c]",
        "ldh [255*256+$40], a",
        "ld a, [255*256+c]",
        "ld hl, sp+$ 5",
    ]);
    assert!(wrong.is_empty(), "{}", wrong.join("\n"));
}

/// The evaluator on lines the table leaves out.
#[test]
fn probe_evaluator_edges() {
    // Documented refusals (design §12): `@`, fixed-point numbers and
    // graphics constants are read but not valued, so an RST vector or bit
    // number written with one is refused, though rgbasm takes these.
    for line in ["rst @", "rst 0.0", "rst 56.0", "bit `00000001, a"] {
        assert!(lookup(line).is_err(), "{line}: a documented refusal");
    }
    let wrong = rgbasm::disagreements(&[
        "bit -2 ** 2 + 7, a",
        "bit 2 ** 3 ** 0, a",
        "bit $80000000 % -1, a",
        "bit '\\1', a",
        "rst \"8\"",
        "BIT 7, A",
        "Rst $38",
        "Ld A, [Hl+]",
        "LD A, [HLI]",
        "LDH A, [C]",
        "ld A, [$FF00+C]",
        "bit 7, [hl+]",
        "bit 07, a",
        "bit 0o7, a",
        "bit 0O7, a",
        "bit 0B1, a",
        "bit HIGH(HIGH($0300)), a",
        "bit LoW (7), a",
        "bit high(7)<<1|1, a",
        "bit $, a",
        "bit 1 == 1 == 1, a",
        "bit 3 != 2 + 2, a",
        "bit ~-4, a",
        "bit !!5, a",
        "bit - 3 + 6, a",
        "bit ';' - 56, a",
        "rst $38 ; '",
        "bit 1 << 2 >> 1, a",
        "bit -1 >>> 29, a",
        "bit '\t' - 6, a",
        "bit 3 4, a",
        "bit (3)(4), a",
        "bit 1 ** 0 ** -1, a",
        "bit high(-1) & 7, a",
        "rst 0x38",
        "rst 0X38",
        "rst 38h",
        "bit 3 ==3, a",
        "bit 3!=4, a",
        "bit 1<<2, a",
        "bit 8>>1, a",
        "bit !0, a",
        "bit 1&&1, a",
        "bit 0||1, a",
        "bit 1|2, a",
        "bit 1^3, a",
        "bit 7%3, a",
        "bit 14%8, a",
        "bit 10%4, a",
    ]);
    assert!(wrong.is_empty(), "{}", wrong.join("\n"));
}

/// Deep mixed nesting must be answered, quickly, without overflowing the
/// stack.
#[test]
fn probe_deep_mixed_nesting() {
    let mut slow = Vec::new();
    for line in [
        format!("rst {}8{}", "-(".repeat(100_000), ")".repeat(100_000)),
        format!("rst {}8{}", "HIGH(~".repeat(100_000), ")".repeat(100_000)),
        format!("rst 1{}", "+1".repeat(200_000)),
        format!("rst 1{}", "**1".repeat(200_000)),
        format!("ld a, {}1{}", "(".repeat(100_000), ")".repeat(100_000)),
        format!("ld a, [{}1{}]", "(".repeat(100_000), ")".repeat(100_000)),
        format!(
            "ldh [{}$ff00{}+c], a",
            "(".repeat(100_000),
            ")".repeat(100_000)
        ),
        format!("jr nz {}", "(".repeat(100_000)),
        format!("ldi a, {}", "[".repeat(100_000)),
    ] {
        let start = std::time::Instant::now();
        let got = lookup(&line);
        let took = start.elapsed();
        eprintln!(
            "{}…: {:?} in {took:?}",
            &line[..15],
            got.as_ref()
                .map(|f| f.len())
                .map_err(|e| matches!(e, LookupError::TooDeep { .. }))
        );
        if took.as_secs_f64() > 0.5 {
            slow.push(format!("{}…: {took:?}", &line[..15]));
        }
    }
    assert!(slow.is_empty(), "{}", slow.join("\n"));
}

/// A long flat chain of binary operators: lookup time against its length.
/// Linear would double per row; red if 200,000 terms take over 0.5 s.
#[test]
fn probe_long_flat_chains_scale() {
    let mut slow = Vec::new();
    for (prefix, term) in [
        ("rst 1", "+1"),
        ("bit 1", "+1"),
        ("rst 1", "**1"),
        ("ld a, 1", "+1"),
    ] {
        for n in [25_000, 50_000, 100_000, 200_000] {
            let line = format!("{prefix}{}", term.repeat(n));
            let start = std::time::Instant::now();
            let _ = lookup(&line);
            let took = start.elapsed();
            eprintln!("{prefix} {term} x{n}: {took:?}");
            if n == 200_000 && took.as_secs_f64() > 0.5 {
                slow.push(format!("{prefix}{term} x{n}: {took:?}"));
            }
        }
    }
    assert!(slow.is_empty(), "{}", slow.join("\n"));
}
