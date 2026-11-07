//! `opcode`: an instruction's size, cycles and flags (a help verb).

use super::flags::JSON;
use super::{Arity, Class, FlagSpec, Verb, VerbSpec};

/// `--json` alone: the lookup needs no session, so no `--dir` or `--as`.
const JSON_ONLY: &[FlagSpec] = &[JSON];

pub const OPCODE_VERB: VerbSpec = VerbSpec {
    verb: Verb::Opcode,
    name: "opcode",
    class: Class::Help,
    summary: "an instruction's bytes, cycles and flags, by text or by byte",
    usage: "QUERY… [--json]",
    arity: Arity {
        min: 1,
        max: None,
        name: "QUERY",
    },
    flags: &[],
    groups: &[JSON_ONLY],
    about: "Looks QUERY up in gbdev's opcode table (the one the disassembler uses) and \
            prints one `opcode` row per match: code, asm, bytes, T-cycles t and M-cycles m \
            (a conditional branch also gives t_not_taken and m_not_taken), and flags as \
            Z N H C (the letter: set by the result; 0 or 1: forced; -: untouched). QUERY is \
            a byte ($20, 0x20), $CB and a byte ($CB37), or an instruction with every operand, \
            exactly as rgbasm 1.0.4 takes it ('xor a', 'cp 5', 'ldi a, [hl]', \
            'ld [$ff00+c], a'); words after the first are joined with spaces. A mnemonic \
            alone lists every form; an operand that isn't a register, condition or \
            placeholder is data, so 'ld a, 5' finds ld a, n8 and 'ld [wFoo], a' finds \
            ld [a16], a. Every operand that holds data (a value, an address, sp's \
            offset, an RST vector, a bit number, $FF00+C's base) is an expression, read \
            as rgbasm reads it: numbers in every base with _ between digits (wider than \
            32 bits is 0, as in rgbasm), characters and one-character strings with \
            rgbasm's escapes, parentheses, every operator with rgbasm's precedence, \
            HIGH() and LOW(); symbols (labels, @, :+), rgbasm's other number functions \
            (BANK(), STRLEN(), …), fixed-point and graphics constants are read but have \
            no value here. So data may name a symbol ('jp Top + 3'), while an RST vector, \
            a bit number and $FF00+C's base must have a value ('rst 8*7', \
            \"bit 'C' - 64, a\", 'ld a, [255*256+c]'), and a known ldh address must be \
            $FF00-$FFFF. Refused, never guessed: a symbol where a value is needed, a \
            register inside an expression, interpolation ({…}), and nesting past 64 \
            levels. Not checked, as they need the source or the link: whether a symbol \
            is defined, whether a value fits its slot (other than ldh's), and a \
            function's argument kinds. Quote anything with brackets, which the shell \
            may expand. A line rgbasm refuses ('jr nz', 'ld a, (hl)', 'ld [c], a', \
            'ld a, 15&7') is bad_request (exit 2), with the spelling rgbasm takes, the \
            nearest mnemonic or how to list the forms.",
    example: (
        "gbcli opcode 'jr nz, .loop'",
        "prints jr nz, e8: 2 bytes, 12 T-cycles taken and 8 not taken, flags untouched",
    ),
};
