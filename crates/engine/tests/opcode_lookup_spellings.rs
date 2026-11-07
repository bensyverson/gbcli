//! `disasm::lookup` takes an instruction exactly as rgbasm 1.0.4 takes it
//! and refuses what rgbasm refuses, with a remedy (leaf dwM7t6). The table
//! below is checked against rgbasm itself (`every_spelling_agrees_with_rgbasm`),
//! so a case can't drift from what the assembler does.

use std::path::Path;
use std::process::Command;

use gbcli_engine::disasm::{LookupError, Opcode, lookup};

fn codes(query: &str) -> Result<Vec<Opcode>, String> {
    lookup(query)
        .map(|found| found.into_iter().map(|f| f.opcode).collect())
        .map_err(|e| e.to_string())
}

fn refusal(query: &str) -> LookupError {
    match lookup(query) {
        Ok(found) => panic!("{query}: rgbasm refuses it, lookup found {found:?}"),
        Err(e) => e,
    }
}

use Opcode::{Base, Cb};

/// Each spelling and the one opcode rgbasm assembles it to, or `None` where
/// rgbasm refuses it. Labels: `Top` (a code label), `wFoo` ($C000) and
/// `rLCDC` ($FF40).
const CASES: &[(&str, Option<Opcode>)] = &[
    // The 8-bit ALU ops may leave out `a,`.
    ("xor a", Some(Base(0xAF))),
    ("xor a, a", Some(Base(0xAF))),
    ("xor A", Some(Base(0xAF))),
    ("xor [hl]", Some(Base(0xAE))),
    ("cp 5", Some(Base(0xFE))),
    ("cp a, 5", Some(Base(0xFE))),
    ("cp a", Some(Base(0xBF))),
    ("sub b", Some(Base(0x90))),
    ("sub a, b", Some(Base(0x90))),
    ("add b", Some(Base(0x80))),
    ("add 5", Some(Base(0xC6))),
    ("adc b", Some(Base(0x88))),
    ("sbc a, 5", Some(Base(0xDE))),
    ("and b", Some(Base(0xA0))),
    ("or b", Some(Base(0xB0))),
    ("add hl", None),
    ("add sp", None),
    ("add a, sp", None),
    ("xor a, b, c", None),
    // Memory is written in brackets; parentheses are an expression's.
    ("ld a, (hl)", None),
    ("set 0, (hl)", None),
    ("jp (hl)", None),
    ("ld a, (5)", Some(Base(0x3E))),
    ("cp (5)", Some(Base(0xFE))),
    ("ld b, hl", None),
    ("ld a, [hl+1]", None),
    // $FF00+C: `ldh [c]`, or `[$ff00+c]` with either mnemonic, never `ld [c]`.
    ("ldh [c], a", Some(Base(0xE2))),
    ("ldh [$ff00+c], a", Some(Base(0xE2))),
    ("ld [$ff00+c], a", Some(Base(0xE2))),
    ("ld [$FF00 + C], a", Some(Base(0xE2))),
    ("ld a, [0xff00+c]", Some(Base(0xF2))),
    ("ld a, [65280+c]", Some(Base(0xF2))),
    ("ldh a, [$ff00+c]", Some(Base(0xF2))),
    ("ld [c], a", None),
    ("ld a, [c]", None),
    ("ld [$ff00+c], b", None),
    ("ldh [c], b", None),
    // ldh's address is $FF00-$FFFF.
    ("ldh [rLCDC], a", Some(Base(0xE0))),
    ("ldh [$ff40], a", Some(Base(0xE0))),
    ("ldh [$ff00+5], a", Some(Base(0xE0))),
    ("ldh a, [$ff00]", Some(Base(0xF0))),
    ("ldh [$40], a", None),
    ("ldh a, [$10000]", None),
    // The increment forms: [hl+], [hli], and ldi/ldd with plain [hl].
    ("ld a, [hl+]", Some(Base(0x2A))),
    ("ld a, [hli]", Some(Base(0x2A))),
    ("ld a, [ hl + ]", Some(Base(0x2A))),
    ("ld [hld], a", Some(Base(0x32))),
    ("ld a, [hl-]", Some(Base(0x3A))),
    ("ldi a, [hl]", Some(Base(0x2A))),
    ("ldi [hl], a", Some(Base(0x22))),
    ("ldd a, [hl]", Some(Base(0x3A))),
    ("ldd [hl], a", Some(Base(0x32))),
    ("ldi a, [hl+]", None),
    ("ldi a, [hli]", None),
    ("ldd a, [hld]", None),
    ("ldi b, [hl]", None),
    ("ldi hl, a", None),
    // Every operand, and no more.
    ("jr nz", None),
    ("jr nz, Top", Some(Base(0x20))),
    ("jr Top", Some(Base(0x18))),
    ("ld a, [hl], 5", None),
    ("inc a, b", None),
    ("halt 0", None),
    ("ccf a", None),
    ("cpl b", None),
    ("cpl a", Some(Base(0x2F))),
    ("stop", Some(Base(0x10))),
    ("stop 0", Some(Base(0x10))),
    // sp+e8, jp hl, and the rest.
    ("ld hl, sp+5", Some(Base(0xF8))),
    ("ld hl, sp + -2", Some(Base(0xF8))),
    ("ld hl, sp-2", Some(Base(0xF8))),
    ("ld hl, sp", None),
    ("ld hl, [sp]", None),
    ("ld hl, [sp+5]", None),
    ("jp hl", Some(Base(0xE9))),
    ("jp [hl]", None),
    ("ld [hl], [hl]", None),
    ("ld a, [wFoo]", Some(Base(0xFA))),
    ("ld [rLCDC], a", Some(Base(0xEA))),
    ("ld [wFoo], sp", Some(Base(0x08))),
    ("ld a, %101", Some(Base(0x3E))),
    ("ld a, 0b101", Some(Base(0x3E))),
    ("ld a, -1", Some(Base(0x3E))),
    ("rst $38", Some(Base(0xFF))),
    ("rst 0", Some(Base(0xC7))),
    ("rst 7", None),
    ("rst $40", None),
    ("bit 7, h", Some(Cb(0x7C))),
    ("bit 8, a", None),
    ("rl a", Some(Cb(0x17))),
    ("ld a, [hl] ; a comment", Some(Base(0x7E))),
    // Not instructions in rgbasm 1.0.4.
    ("ldhl sp, 5", None),
    ("ldio a, [$ff00+c]", None),
    // The second review (leaf FPQsnp). Operands are separated by commas:
    // whitespace between two names doesn't make two operands.
    ("jr nz Top", None),
    ("call z Top", None),
    ("jp c Top", None),
    ("ld a b", None),
    ("ld a, [hl -]", Some(Base(0x3A))),
    // HIGH() and LOW() of a register pair name its halves.
    ("ld a, high(bc)", Some(Base(0x78))),
    ("ld a, low(hl)", Some(Base(0x7D))),
    ("ld [hl], low(de)", Some(Base(0x73))),
    ("ld a, high(af)", Some(Base(0x7F))),
    ("ld a, low(af)", None),
    ("ld a, HIGH( BC )", Some(Base(0x78))),
    ("ld hl, high(bc)", None),
    ("ld a, high(bc) + 1", None),
    // RST vectors and bit numbers are constant expressions, with rgbasm's
    // precedence (`&` binds tighter than `+`).
    ("rst 8*7", Some(Base(0xFF))),
    ("rst 8 * 7", Some(Base(0xFF))),
    ("rst $0038", Some(Base(0xFF))),
    ("rst (1 << 3)", Some(Base(0xCF))),
    ("rst 7*8+1", None),
    ("rst -1", None),
    ("bit 3+4, a", Some(Cb(0x7F))),
    ("set 2*3, [hl]", Some(Cb(0xF6))),
    ("bit 1 + 1 * 2, b", Some(Cb(0x58))),
    ("bit 2 + 7 & 1, a", Some(Cb(0x5F))),
    ("res 15 & 7, a", Some(Cb(0xBF))),
    ("bit 2 ** 2 - 1, a", Some(Cb(0x5F))),
    ("bit -1 % 8, a", Some(Cb(0x7F))),
    ("bit 1_0 / 3, a", Some(Cb(0x5F))),
    ("bit 'x', a", None),
    ("bit wFoo, a", None),
    ("bit 8 / 0, a", None),
    // The rest of that review's probe, which already agreed.
    ("push AF", Some(Base(0xF5))),
    ("stop $10", Some(Base(0x10))),
    ("add sp, -2", Some(Base(0xE8))),
    ("ld a,b", Some(Base(0x78))),
    ("LD A,[HL]", Some(Base(0x7E))),
    ("ldh a, [rLCDC + 1]", Some(Base(0xF0))),
    ("ldh a, [$ff00 + 5]", Some(Base(0xF0))),
    ("ld a, [$ff00+c+0]", None),
    ("ldio a, [$ff40]", None),
    ("ldio [c], a", None),
    ("ld a, hli", None),
    ("ld a, [hl], ", None),
    ("jp z, hl", None),
    ("ld [$c000], a", Some(Base(0xEA))),
    ("ld a, 'x'", Some(Base(0x3E))),
    ("ld a, \"x\"", Some(Base(0x3E))),
    ("ld b, b", Some(Base(0x40))),
    ("ld hl, sp+0", Some(Base(0xF8))),
    ("ld a, [bc]", Some(Base(0x0A))),
    ("ld a, [de]", Some(Base(0x1A))),
    ("ld [bc], a", Some(Base(0x02))),
    // The third review (leaf iH2KOO): constant expressions as rgbasm 1.0.4
    // reads them. Multiplying by zero panicked.
    ("rst 8*0", Some(Base(0xC7))),
    ("bit 3*0, a", Some(Cb(0x47))),
    // Character constants keep their case: 'h' - 'H' is $20, not 0.
    ("bit 'C' - 64, a", Some(Cb(0x5F))),
    ("rst 'H' - 16", Some(Base(0xFF))),
    ("rst 'h' - 'H'", Some(Base(0xE7))),
    ("bit 'a' - 'a', a", Some(Cb(0x47))),
    // Characters: a space, the quote of the other kind, `;` and `,`.
    ("rst ' ' + 8", Some(Base(0xEF))),
    ("rst ' '+8", Some(Base(0xEF))),
    ("bit '\"' - 32, a", Some(Cb(0x57))),
    ("bit '}' - 120, a", Some(Cb(0x6F))),
    ("rst ';' - 51", Some(Base(0xCF))),
    ("bit ',' - 40, a", Some(Cb(0x67))),
    ("bit 'ab', a", None),
    ("bit '', a", None),
    ("bit 'é' - 200, a", None),
    ("bit '{' - 120, a", None),
    ("rst ''' - 39", None),
    // Escapes.
    ("bit '\\n' - 7, a", Some(Cb(0x5F))),
    ("bit '\\t' - 2, a", Some(Cb(0x7F))),
    ("bit '\\0', a", Some(Cb(0x47))),
    ("bit '\\\\' - 90, a", Some(Cb(0x57))),
    ("bit '\\'' - 36, a", Some(Cb(0x5F))),
    ("bit '\\\"' - 32, a", Some(Cb(0x57))),
    ("bit '\\r' - 10, a", Some(Cb(0x5F))),
    ("bit '\\{' - 120, a", Some(Cb(0x5F))),
    ("bit '\\}' - 120, a", Some(Cb(0x6F))),
    ("bit '\\,' - 40, a", None),
    ("bit '\\x', a", None),
    ("bit '\\(', a", None),
    // A one-character string is a number (rgbasm warns that it is obsolete).
    ("bit \"x\" - \"u\", a", Some(Cb(0x5F))),
    ("bit \"\\n\" - 7, a", Some(Cb(0x5F))),
    ("bit \"\\\"\" - 32, a", Some(Cb(0x57))),
    ("bit \"'\" - 36, a", Some(Cb(0x5F))),
    ("bit \"\\{\" - 120, a", Some(Cb(0x5F))),
    ("bit \"ab\", a", None),
    ("bit \"\", a", None),
    ("bit \"{\" - 120, a", None),
    // HIGH() and LOW() of a constant.
    ("rst high($3800)", Some(Base(0xFF))),
    ("rst HIGH($3800)", Some(Base(0xFF))),
    ("rst high ($3800)", Some(Base(0xFF))),
    ("rst high($38_00)", Some(Base(0xFF))),
    ("rst low($1238)", Some(Base(0xFF))),
    ("rst high(-1) - $C7", Some(Base(0xFF))),
    ("bit low(-8) - 248, a", Some(Cb(0x47))),
    ("rst high($3800", None),
    ("rst high($38, 1)", None),
    ("bit high(bc), a", None),
    // A negative shift reverses it (rgbasm warns).
    ("bit 1 >> -1, a", Some(Cb(0x57))),
    ("bit 4 << -1, a", Some(Cb(0x57))),
    ("bit 1 >>> -1, a", Some(Cb(0x57))),
    ("bit 8 >> -33, a", Some(Cb(0x47))),
    ("bit 1 << -32, a", Some(Cb(0x47))),
    ("bit -8 >> -1 + 20, a", Some(Cb(0x67))),
    // A power's exponent is any unary expression; a negative one is refused.
    ("rst 8 ** !0", Some(Base(0xCF))),
    ("bit 2 ** ~-2, a", Some(Cb(0x57))),
    ("bit 2 ** +1, a", Some(Cb(0x57))),
    ("bit 2 ** - - 1, a", Some(Cb(0x57))),
    ("bit 2 ** -2 ** 0, a", None),
    ("bit 1 ** -1, a", None),
    ("bit 2 ** -1, a", None),
    ("bit 0 ** 0, a", Some(Cb(0x4F))),
    ("rst 0 ** $7FFFFFFF", Some(Base(0xC7))),
    ("bit 2 ** $7FFFFFFF, a", Some(Cb(0x47))),
    ("bit 3 ** $7FFFFFFF & 7, a", Some(Cb(0x5F))),
    ("bit 3 == 3 ** 1, a", Some(Cb(0x4F))),
    // Division and modulo with a negative divisor; precedence.
    ("bit 6 / -2 + 6, a", Some(Cb(0x5F))),
    ("bit 6 % -3 + 3, a", Some(Cb(0x5F))),
    ("bit 7 / -2 + 7, a", Some(Cb(0x5F))),
    ("bit -6 / 3 + 5, a", Some(Cb(0x5F))),
    ("bit 1 | 2 + 1, a", Some(Cb(0x67))),
    ("bit 1 << 1 + 1, a", Some(Cb(0x5F))),
    ("bit 5 % 3 << 1, a", Some(Cb(0x67))),
    ("bit 1 ^ 3 & 2, a", Some(Cb(0x57))),
    ("bit 2 + 3 == 5, a", Some(Cb(0x4F))),
    ("bit 16 / 2 / 2, a", Some(Cb(0x67))),
    ("bit 2 ** 1 ** 3, a", Some(Cb(0x57))),
    ("bit 7 - 8 - -4, a", Some(Cb(0x5F))),
    ("bit 3 + + 1, a", Some(Cb(0x67))),
    ("bit !1 + 3, a", Some(Cb(0x5F))),
    ("bit -1 >> 40 + 4, a", Some(Cb(0x5F))),
    ("bit -1 >>> 31 + 2, a", Some(Cb(0x5F))),
    ("bit 1 << 32, a", Some(Cb(0x47))),
    ("bit $80000000 / -1 + $80000003, a", Some(Cb(0x5F))),
    ("rst 0 % 0", None),
    ("bit (3), a", Some(Cb(0x5F))),
    ("bit 1 + (2 * (1)), a", Some(Cb(0x5F))),
    // Whitespace separates operator characters: `& &` is not `&&`.
    ("bit 7 & & 1, a", None),
    ("bit 3 * * 1, a", None),
    ("bit 3 *** 1, a", None),
    ("bit 7 / / 2, a", None),
    ("rst 1 < < 3", None),
    ("bit 1 = = 1, a", None),
    ("bit 1 | | 0, a", None),
    ("bit 2 > > 1, a", None),
    ("bit 2 ! = 1, a", None),
    ("bit $ 3, a", None),
    ("bit 1 + , a", None),
    // Numbers: `_` may follow a base prefix, never another `_` or end one.
    ("bit 0X3, a", Some(Cb(0x5F))),
    ("bit $_3, a", Some(Cb(0x5F))),
    ("bit 0x_3, a", Some(Cb(0x5F))),
    ("bit 0X_3, a", Some(Cb(0x5F))),
    ("bit %_1, a", Some(Cb(0x4F))),
    ("bit 0b_1, a", Some(Cb(0x4F))),
    ("bit &_3, a", Some(Cb(0x5F))),
    ("bit 3_, a", None),
    ("bit $3_, a", None),
    ("bit $__3, a", None),
    ("bit 3__4 & 7, a", None),
    ("bit 0x, a", None),
    // The rest of that review's probe, which already agreed.
    ("ld a, \";\"", Some(Base(0x3E))),
    ("ld a, b ; copy it to a", Some(Base(0x78))),
    ("ld a, \" \"", Some(Base(0x3E))),
    ("ld a, ' '", Some(Base(0x3E))),
    ("ld high(bc), a", Some(Base(0x47))),
    ("ld high(hl), low(de)", Some(Base(0x63))),
    ("bit 3, low(hl)", Some(Cb(0x5D))),
    ("ld a, [high(bc)]", None),
    ("ld a, high((bc))", None),
    ("ld a, (b)", None),
    ("ld a, high (bc)", Some(Base(0x78))),
];

#[test]
fn every_spelling_finds_its_one_opcode_or_is_refused() {
    let mut wrong = Vec::new();
    for &(query, expected) in CASES {
        let got = lookup(query);
        let ok = match (expected, &got) {
            (Some(op), Ok(found)) => found.len() == 1 && found[0].opcode == op,
            (None, Err(_)) => true,
            _ => false,
        };
        if !ok {
            wrong.push(format!("{query:?}: expected {expected:?}, got {got:?}"));
        }
    }
    assert!(wrong.is_empty(), "{}", wrong.join("\n"));
}

/// The reviewer's probes (8c1f5de): `xor a` found all nine xor forms, and
/// `cp 5` and `sub b` found nothing.
#[test]
fn alu_short_forms_find_their_one_opcode() {
    assert_eq!(codes("xor a"), Ok(vec![Base(0xAF)]), "xor a");
    assert_eq!(codes("cp 5"), Ok(vec![Base(0xFE)]), "cp 5");
    assert_eq!(codes("sub b"), Ok(vec![Base(0x90)]), "sub b");
}

/// `(hl)` is not RGBDS memory syntax; it was taken for immediate data.
#[test]
fn parenthesized_hl_is_refused_with_the_bracket_spelling() {
    assert_ne!(codes("ld a, (hl)"), Ok(vec![Base(0x3E)]));
    let e = refusal("ld a, (hl)");
    assert!(e.remedy().contains("ld a, [hl]"), "{}", e.remedy());
}

#[test]
fn each_refusal_names_a_spelling_rgbasm_takes() {
    for (query, fix) in [
        ("ld [c], a", "ldh [c], a"),
        ("jp [hl]", "jp hl"),
        ("jr nz", "jr nz, e8"),
        ("ldi a, [hli]", "ld a, [hli]"),
        ("ldh [$40], a", "$FF00"),
        ("jr nz Top", "jr nz, Top"),
        ("call z Top", "comma"),
    ] {
        let remedy = refusal(query).remedy();
        assert!(remedy.contains(fix), "{query}: {remedy}");
    }
    let remedy = refusal("ld q, a").remedy();
    assert!(remedy.contains("gbcli opcode ld"), "{remedy}");
}

/// Each case through rgbasm and rgblink 1.0.4: an accepted line must
/// assemble to the case's opcode, a refused one must not assemble.
#[test]
fn every_spelling_agrees_with_rgbasm() {
    let dir = std::env::temp_dir().join(format!("gbcli-opcode-spellings-{}", std::process::id()));
    // Two processes a case, so the cases run on several threads.
    let threads = std::thread::available_parallelism().map_or(4, |n| n.get());
    let wrong: Vec<String> = std::thread::scope(|scope| {
        let workers: Vec<_> = (0..threads)
            .map(|worker| {
                let dir = dir.join(worker.to_string());
                scope.spawn(move || {
                    std::fs::create_dir_all(&dir).unwrap();
                    let mut wrong = Vec::new();
                    for &(query, expected) in CASES.iter().skip(worker).step_by(threads) {
                        let got = assemble(&dir, query);
                        let want = expected.map(|op| match op {
                            Base(b) => vec![b],
                            Cb(b) => vec![0xCB, b],
                        });
                        let agrees = match (&want, &got) {
                            (Some(want), Some(bytes)) => bytes.starts_with(want),
                            (None, None) => true,
                            _ => false,
                        };
                        if !agrees {
                            wrong.push(format!("{query:?}: table {want:02X?}, rgbasm {got:02X?}"));
                        }
                    }
                    wrong
                })
            })
            .collect();
        workers
            .into_iter()
            .flat_map(|w| w.join().expect("worker"))
            .collect()
    });
    std::fs::remove_dir_all(&dir).ok();
    assert!(wrong.is_empty(), "{}", wrong.join("\n"));
}

/// The bytes rgbasm and rgblink make of `line`, or `None` if either refuses it.
fn assemble(dir: &Path, line: &str) -> Option<Vec<u8>> {
    let (src, obj, rom) = (dir.join("t.asm"), dir.join("t.o"), dir.join("t.gb"));
    std::fs::write(
        &src,
        format!(
            "DEF wFoo EQU $C000\nDEF rLCDC EQU $FF40\n\
             SECTION \"s\", ROM0[$0]\nTop:\n    {line}\n"
        ),
    )
    .unwrap();
    let run = |cmd: &str, args: &[&Path]| {
        Command::new(cmd)
            .args(args)
            .output()
            .unwrap_or_else(|e| {
                panic!("{cmd} is needed for this test ({e}): brew install rgbds (1.0.4)")
            })
            .status
            .success()
    };
    let o = Path::new("-o");
    if !run("rgbasm", &[o, &obj, &src]) || !run("rgblink", &[o, &rom, &obj]) {
        return None;
    }
    Some(std::fs::read(&rom).unwrap()[..3].to_vec())
}
