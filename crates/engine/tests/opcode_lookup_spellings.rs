//! `disasm::lookup` takes an instruction exactly as rgbasm 1.0.4 takes it
//! and refuses what rgbasm refuses, with a remedy (leaf dwM7t6). The table
//! below is checked against rgbasm itself (`every_spelling_agrees_with_rgbasm`),
//! so a case can't drift from what the assembler does.

mod rgbasm;

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
/// rgbasm refuses it. Labels: `Top` (a code label at the line), `wFoo`
/// ($C000) and `rLCDC` ($FF40); `wBar` is not defined.
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
    // The fourth review (leaf radJxi). rgbasm's lexer reads `&` before an
    // octal digit or `_` as an octal number, and `%` before 0, 1 or `_` as a
    // binary one, so `15&7` is two numbers in a row.
    ("res 15&7, a", None),
    ("bit 7&7, a", None),
    ("rst $38&56", None),
    ("bit 5%1, a", None),
    ("bit 7 %1, a", None),
    ("bit 7&_7, a", None),
    ("bit 7%_1, a", None),
    ("ld a, 15&7", None),
    ("ld a, &8", None),
    ("bit 7&8, a", Some(Cb(0x47))),
    ("bit 5%2, a", Some(Cb(0x4F))),
    ("bit 7%8, a", Some(Cb(0x7F))),
    ("bit 7% 1, a", Some(Cb(0x47))),
    // A number wider than 32 bits is 0 (rgbasm warns, -Wlarge-constant).
    ("rst $100000038", Some(Base(0xC7))),
    ("rst 4294967352", Some(Base(0xC7))),
    ("bit $1_0000_0003, a", Some(Cb(0x47))),
    ("bit $10000000000000003, a", Some(Cb(0x47))),
    ("bit $FFFFFFFF & 7, a", Some(Cb(0x7F))),
    // Data, addresses and sp's offset are expressions too, read as rgbasm
    // reads them; a symbol's value is unknown, but its syntax is checked.
    ("ld a, $ 38", None),
    ("jp $ 100", None),
    ("ld a, [$c0 00]", None),
    ("ldh [$ff 40], a", None),
    ("ld a, 5_", None),
    ("ld a, 1 +", None),
    ("ld a, )(", None),
    ("ld a, 'x", None),
    ("ld a, \"ab\"", None),
    ("ld a, 'ab'", None),
    ("ld a, ''", None),
    ("ld hl, \"ab\"", None),
    ("ld a, (wFoo", None),
    ("ld a, wFoo +", None),
    ("ld a, 7&a", None),
    ("ld a, 7 % a", None),
    ("jr nz, Top +", None),
    ("add sp, wFoo +", None),
    ("ld hl, sp+$ 5", None),
    ("ld hl, sp +", None),
    ("ld a, wFoo % 0", None),
    ("ld a, wFoo ** -1", None),
    ("ld a, FOO(1)", None),
    ("ld a, BANK(1 +)", None),
    ("ld a, BANK()", None),
    ("ld a, LOW(Top, 1)", None),
    ("ld a, 5.q", None),
    ("ld a, 5._5", None),
    ("ld a, 5_.5", None),
    ("ld a, 0x5.5", None),
    ("ld a, 1.5e", None),
    ("ld a, `0124", None),
    ("ld a, `", None),
    ("ld a, `012345678", None),
    ("ld a, Top.", None),
    ("ld a, 300", Some(Base(0x3E))),
    ("ld a, [wFoo + 1]", Some(Base(0xFA))),
    ("ld a, high(Top)", Some(Base(0x3E))),
    ("ld a, low(wFoo) + 1", Some(Base(0x3E))),
    ("ld a, 2 ** wFoo", Some(Base(0x3E))),
    ("ld a, !wFoo", Some(Base(0x3E))),
    ("ld a, @", Some(Base(0x3E))),
    ("jp @+3", Some(Base(0xC3))),
    ("ld a, BANK(Top)", Some(Base(0x3E))),
    ("ld a, bank (Top)", Some(Base(0x3E))),
    ("ld a, DEF(wFoo)", Some(Base(0x3E))),
    ("ld a, ISCONST(wFoo)", Some(Base(0x3E))),
    ("ld a, SIZEOF(\"s\")", Some(Base(0x3E))),
    ("ld a, STRCMP(\"a\", \"b\")", Some(Base(0x3E))),
    ("call STRLEN(\"abc\")", Some(Base(0xCD))),
    ("ld a, bitwidth(5)", Some(Base(0x3E))),
    ("ld a, MUL(6.0, 7.0, 8)", Some(Base(0x3E))),
    ("ld a, 1.5", Some(Base(0x3E))),
    ("ld a, 5.", Some(Base(0x3E))),
    ("ld a, 1.5q8", Some(Base(0x3E))),
    ("ld a, 5.5_5", Some(Base(0x3E))),
    ("ld a, 1.5 + wFoo", Some(Base(0x3E))),
    ("ld a, `00001111", Some(Base(0x3E))),
    ("ld a, `0123_0123", Some(Base(0x3E))),
    ("ld hl, sp - 2 + 1", Some(Base(0xF8))),
    ("ld hl, sp + (5)", Some(Base(0xF8))),
    ("ld hl, sp - -2", Some(Base(0xF8))),
    ("ld hl, sp+wFoo", Some(Base(0xF8))),
    // $FF00+C's base and ldh's address are evaluated.
    ("ld a, [$fe00+$100+c]", Some(Base(0xF2))),
    ("ld a, [$ff_00+c]", Some(Base(0xF2))),
    ("ld a, [255*256+c]", Some(Base(0xF2))),
    ("ld a, [$ff00 | 0 + c]", Some(Base(0xF2))),
    ("ld a, [ $ff00+c ]", Some(Base(0xF2))),
    ("ld a, [1 == 1 + c]", None),
    ("ld a, [c + $ff00]", None),
    ("ldh [$100+$80], a", None),
    ("ldh a, [$ffff + 1]", None),
    ("ldh a, [-256]", None),
    ("ldh [$fe00+$180], a", Some(Base(0xE0))),
    ("ldh [255*256+$40], a", Some(Base(0xE0))),
    // The differential test's first sweep (leaf mDk1lR,
    // `opcode_rgbasm_differential.rs`). `LOW(bc)` names the register c,
    // never the condition.
    ("call low(bc), Top", None),
    ("jp LOW(BC), $100", None),
    ("ret low (bc)", None),
    ("jr low(bc), Top", None),
    ("ld low(bc), a", Some(Base(0x4F))),
    // `++` is one token, rgbasm's string concatenation.
    ("bit ++3, a", None),
    ("bit 1 ++ 2, a", None),
    ("ld a, ++3", None),
    ("ld a, 1++2", None),
    ("bit + + 3, a", Some(Cb(0x5F))),
    ("bit 1 + +2, a", Some(Cb(0x5F))),
    // The mnemonic ends where its name does, not at whitespace.
    ("rl[hl]", Some(Cb(0x16))),
    ("ld[hl], b", Some(Base(0x70))),
    ("set!$6, e", Some(Cb(0xC3))),
    ("bit($5),b", Some(Cb(0x68))),
    ("add-$66", Some(Base(0xC6))),
    ("sbc`3233", Some(Base(0xDE))),
    ("cp'x'", Some(Base(0xFE))),
    ("jp$100", None),
    ("ld_a, b", None),
    // A graphics constant may have more than eight pixels (rgbasm warns
    // and keeps the first eight).
    ("ld a, `230223223", Some(Base(0x3E))),
    ("jp `0123012301230123", Some(Base(0xC3))),
    // ldi and ldd take `HIGH(af)` for a.
    ("ldi [hl], high(af)", Some(Base(0x22))),
    ("ldd HIGH(AF), [hl]", Some(Base(0x3A))),
    // A condition after `!` is its opposite.
    ("ret !nz", Some(Base(0xC8))),
    ("ret ! Z", Some(Base(0xC0))),
    ("jp !c, Top", Some(Base(0xD2))),
    ("jr !nc, Top", Some(Base(0x38))),
    ("call !!nz, Top", Some(Base(0xC4))),
    ("ret !!!z", Some(Base(0xC0))),
    ("ld a, !nc", None),
    ("ld a, !c", None),
    ("ret !a", None),
    // `.` is the current label scope's symbol.
    ("ld a, .", Some(Base(0x3E))),
    ("xor .", Some(Base(0xEE))),
    ("jp . + 3", Some(Base(0xC3))),
    // rgbasm folds `&&` with 0, `||` with a non-zero value and `&` with 0
    // to a constant, whatever the other side.
    ("bit wBar && 0, a", Some(Cb(0x47))),
    ("bit 0 && wBar, a", Some(Cb(0x47))),
    ("bit wBar || 5, a", Some(Cb(0x4F))),
    ("bit 1 || wBar, a", Some(Cb(0x4F))),
    ("bit wBar & 0, a", Some(Cb(0x47))),
    ("bit (0 & wBar) + 3, a", Some(Cb(0x5F))),
    ("rst wBar & 0", Some(Base(0xC7))),
    ("ld a, [wBar & 0 + $ff00 + c]", Some(Base(0xF2))),
    ("bit wBar && 0 + 1, a", None),
    ("bit -1 && wBar, a", None),
    ("bit wBar * 0, a", None),
    ("bit 0 & 1 + wBar, a", None),
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

/// Each case through rgbasm and rgblink 1.0.4, in one batch: an accepted
/// line must assemble to the case's opcode, a refused one must not assemble.
#[test]
fn every_spelling_agrees_with_rgbasm() {
    let lines: Vec<&str> = CASES.iter().map(|&(query, _)| query).collect();
    let wrong: Vec<String> = CASES
        .iter()
        .zip(rgbasm::assemble(&lines))
        .filter_map(|(&(query, expected), got)| {
            let want = expected.map(|op| match op {
                Base(b) => vec![b],
                Cb(b) => vec![0xCB, b],
            });
            let agrees = match (&want, &got) {
                (Some(want), Some(bytes)) => bytes.starts_with(want),
                (None, None) => true,
                _ => false,
            };
            (!agrees).then(|| format!("{query:?}: table {want:02X?}, rgbasm {got:02X?}"))
        })
        .collect();
    assert!(wrong.is_empty(), "{}", wrong.join("\n"));
}
