//! A standing differential test of `disasm::lookup` against rgbasm and
//! rgblink 1.0.4 (leaf mDk1lR), in place of further review rounds: a seeded
//! generator (`rgbasm/grammar.rs`) writes instruction lines from every form
//! in the opcode table, with registers, conditions and brackets in rgbasm's
//! other spellings and cases (`[hli]`, `LOW(bc)`, `!nz`, `[$ff00+c]`, a
//! mnemonic glued to its operand), and data as expressions over every number
//! base and prefix, `_`s, glued and spaced operators, character constants and
//! escapes, HIGH()/LOW(), functions, fixed-point and graphics constants and
//! symbols; then comments, missing and extra commas, parentheses for
//! brackets, cut lines and stray characters. rgbasm assembles them in one
//! batch (`rgbasm/mod.rs`) and the lookup must agree on every line: the same
//! opcode and length, or both refuse. A disagreement prints the line, both
//! answers and rgbasm's error; once fixed it becomes a row of
//! `opcode_lookup_spellings.rs`.
//!
//! **The documented differences, all in this one place.** Where rgbasm
//! needs the source or the link, the lookup answers otherwise by design
//! (design §12, `disasm/lookup.rs`'s module doc). The generator never writes
//! some; `documented` sets the rest aside, by rgbasm's error where it names
//! the cause, and the sweep counts them:
//!
//! - never written: a symbol, `@`, a fixed-point number or a graphics
//!   constant where a value is needed (an RST vector, a bit number,
//!   $FF00+C's base), which the lookup refuses; a symbol other than `wFoo`,
//!   `rLCDC`, `Top` and `.`, or one recased; `BANK()` of a constant;
//!   the table's placeholders (`n8`, `a16`); nesting past a few levels;
//! - set aside: a symbol or section that isn't defined (rgblink's
//!   "Undefined symbol", made by junk joining two words), which the lookup
//!   takes as defined; `jr`'s target out of reach and a symbol in `ldh`'s
//!   address out of range, which the lookup can't value; a function's
//!   argument of the wrong kind (`BANK(-Top)`); an interpolation
//!   (`{wFoo}`) and nesting past 64 levels, which the lookup refuses; a
//!   trailing `\`, which continues the line onto the next, so the lookup
//!   refuses it; and a mnemonic alone, which lists its forms.

mod rgbasm;

use std::collections::BTreeMap;

use gbcli_engine::disasm::LookupError;
use rgbasm::{Ours, grammar::lines};

/// Why rgbasm and the lookup may disagree on a line by design, if they do:
/// the documented differences the generator can't keep out on its own,
/// recognized from the lookup's answer and rgbasm's errors.
fn documented(line: &str, ours: &Ours, rgbasm: &Result<Vec<u8>, String>) -> Option<&'static str> {
    let only = |what: &str, why: &str| why.lines().all(|l| l.contains(what));
    match (ours, rgbasm) {
        (Ours::Listing(_), _) => Some("a mnemonic alone lists its forms"),
        (Ours::Refused(LookupError::TooDeep { .. }), Ok(_)) => Some("nesting past 64"),
        (Ours::Refused(_), Ok(_)) if line.contains('{') => Some("an interpolation"),
        (Ours::Refused(_), Ok(_)) if line.trim_end().ends_with('\\') => Some("a line continuation"),
        (Ours::Opcode(..), Err(why))
            if only("ndefined symbol", why)
                || only("ndefined section", why)
                || only("has no value outside of a local label scope", why) =>
        {
            Some("a symbol or section that isn't defined")
        }
        (Ours::Opcode(..), Err(_)) if takes_a_name(line) => {
            Some("a function's argument of the wrong kind")
        }
        (Ours::Opcode(..), Err(why)) if only("target must be between -128 and 127", why) => {
            Some("jr's target out of reach")
        }
        // A known `ldh` address out of range is refused; one with a symbol
        // in it is unknown.
        (Ours::Opcode(..), Err(why))
            if only("not between $FF00 to $FFFF", why) && line.contains("rLCDC") =>
        {
            Some("a symbol in ldh's address")
        }
        _ => None,
    }
}

/// Whether `line` calls a function whose argument is a symbol or a
/// section's name, which the lookup reads as any expression.
fn takes_a_name(line: &str) -> bool {
    let lower = line.to_ascii_lowercase();
    ["bank", "sizeof", "startof", "def", "isconst"]
        .iter()
        .any(|f| lower.contains(&format!("{f}(")) || lower.contains(&format!("{f} (")))
}

/// What a sweep found.
struct Sweep {
    lines: usize,
    /// Lines rgbasm takes.
    taken: usize,
    /// Disagreements a documented difference explains, by difference.
    set_aside: BTreeMap<&'static str, usize>,
    /// The rest, each with the line and both answers.
    wrong: Vec<String>,
}

fn sweep(seeds: &[u64], per_seed: usize) -> Sweep {
    let all: Vec<String> = seeds.iter().flat_map(|&s| lines(s, per_seed)).collect();
    let theirs = rgbasm::answers(&all);
    let mut sweep = Sweep {
        lines: all.len(),
        taken: theirs.iter().filter(|b| b.is_ok()).count(),
        set_aside: BTreeMap::new(),
        wrong: Vec::new(),
    };
    for (line, rgbasm) in all.iter().zip(&theirs) {
        let ours = rgbasm::ours(line);
        if rgbasm::agrees(&ours, rgbasm) {
            continue;
        }
        match documented(line, &ours, rgbasm) {
            Some(why) => *sweep.set_aside.entry(why).or_default() += 1,
            None => sweep.wrong.push(rgbasm::describe(line, &ours, rgbasm)),
        }
    }
    sweep
}

fn check(sweep: Sweep) {
    eprintln!(
        "{} lines, {} taken by rgbasm; set aside as documented: {:?}",
        sweep.lines, sweep.taken, sweep.set_aside
    );
    let wrong = sweep.wrong;
    assert!(
        wrong.is_empty(),
        "{} disagreements:\n{}",
        wrong.len(),
        wrong.join("\n")
    );
    // The generator writes a fair share of lines rgbasm takes and refuses.
    let share = sweep.taken * 100 / sweep.lines;
    assert!(
        (30..=90).contains(&share),
        "rgbasm took {share}% of the lines"
    );
}

/// The warm suite's sample: one seed, one rgbasm batch.
#[test]
fn generated_lines_agree_with_rgbasm() {
    check(sweep(&[1], 400));
}

/// The large sweep: ten seeds, 40,000 lines in ten batches.
#[test]
#[ignore = "pre-release: 40,000 lines through rgbasm, about 6 s"]
fn many_generated_lines_agree_with_rgbasm() {
    let seeds: Vec<u64> = (2..12).collect();
    check(sweep(&seeds, 4000));
}
