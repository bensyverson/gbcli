//! Finding opcodes in the table by byte (`$20`, `$CB37`) or by the
//! instruction as rgbasm 1.0.4 takes it (`jr nz, .loop`, `xor a`,
//! `ld a, [hl+]`, `ld [wFoo], a`): each form's size, cycles and flags, for
//! `gbcli opcode`. A line rgbasm refuses is refused, with the spelling it
//! takes where one is near (`crates/engine/tests/opcode_lookup_spellings.rs`
//! checks the cases against rgbasm itself).

use std::fmt;

use super::Opcode;
use super::expr;
use super::table;
use super::types::{Cycles, Mnemonic, OpInfo, OperandKind};

/// One opcode as the table describes it, written as RGBDS writes it with
/// the table's placeholders for data (`n8`, `n16`, `a8`, `a16`, `e8`).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Form {
    pub opcode: Opcode,
    /// Lowercase: `jr nz, e8`, `ld a, [hl+]`, `ldh [a8], a`, `ld hl, sp+e8`.
    pub asm: String,
    /// Bytes, including the 0xCB prefix.
    pub len: u8,
    /// T-cycles at single speed.
    pub cycles: Cycles,
    /// Z, N, H and C in that order: the flag's letter when the result sets
    /// it, 0 or 1 when forced, `-` when untouched.
    pub flags: &'static str,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum LookupError {
    Empty,
    /// `$…` or `0x…` that is neither one byte nor $CB and one byte.
    BadByte {
        text: String,
    },
    NoMnemonic {
        word: String,
        near: Vec<String>,
    },
    /// rgbasm would refuse the line: no form of the mnemonic takes exactly
    /// these operands.
    NoForm {
        query: String,
        mnemonic: String,
        forms: usize,
        /// A spelling rgbasm takes, when one is near enough to name.
        fix: Option<Fix>,
    },
}

/// How to write a refused line so rgbasm takes it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Fix {
    /// The same instruction as rgbasm spells it (`ld a, [hl]` for
    /// `ld a, (hl)`, `ldh [c], a` for `ld [c], a`, `jp hl` for `jp [hl]`).
    Write(String),
    /// The forms the line starts, with every operand (`jr nz, e8` for
    /// `jr nz`).
    Complete(Vec<String>),
    /// An `ldh` address given as a number outside $FF00-$FFFF.
    LdhRange,
    /// Operands separated by whitespace (`jr nz Top`): rgbasm separates
    /// them with commas. The line with them, when it looks up.
    Comma(Option<String>),
}

impl fmt::Display for LookupError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            LookupError::Empty => write!(f, "no opcode or instruction given"),
            LookupError::BadByte { text } => write!(
                f,
                "`{text}` is not an opcode: one byte, or $CB and the byte after it"
            ),
            LookupError::NoMnemonic { word, .. } => write!(f, "no instruction `{word}`"),
            LookupError::NoForm {
                query, mnemonic, ..
            } => write!(
                f,
                "no form of `{mnemonic}` matches `{query}` as rgbasm writes it"
            ),
        }
    }
}

impl LookupError {
    /// The next command to try.
    pub fn remedy(&self) -> String {
        match self {
            LookupError::Empty | LookupError::BadByte { .. } => {
                "give a byte or an instruction: `gbcli opcode '$20'`, `gbcli opcode '$CB37'`, \
                 `gbcli opcode 'ld a, [hl+]'`"
                    .into()
            }
            LookupError::NoMnemonic { near, .. } => match near.first() {
                Some(name) => format!("did you mean `gbcli opcode {name}`?"),
                None => format!("mnemonics: {}", mnemonics().join(" ")),
            },
            LookupError::NoForm {
                mnemonic,
                forms,
                fix,
                ..
            } => match fix {
                Some(Fix::Write(text)) => {
                    format!("rgbasm writes it `{text}`: `gbcli opcode '{text}'`")
                }
                Some(Fix::Complete(found)) => {
                    let found: Vec<String> = found.iter().map(|f| format!("`{f}`")).collect();
                    format!("rgbasm needs every operand: {}", found.join(" or "))
                }
                Some(Fix::LdhRange) => "an `ldh` address is $FF00 to $FFFF, as in \
                     `ldh [$FF40], a`; any other address takes `ld`"
                    .into(),
                Some(Fix::Comma(Some(text))) => {
                    format!("rgbasm separates operands with a comma: `gbcli opcode '{text}'`")
                }
                Some(Fix::Comma(None)) => format!(
                    "rgbasm separates operands with a comma; `gbcli opcode {mnemonic}` lists its \
                     {forms} forms"
                ),
                None => format!("`gbcli opcode {mnemonic}` lists its {forms} forms"),
            },
        }
    }
}

/// The table's entry for one opcode.
pub fn form(opcode: Opcode) -> Form {
    let info = info(opcode);
    Form {
        opcode,
        asm: asm(info),
        len: info.len,
        cycles: info.cycles,
        flags: info.flags,
    }
}

/// Every form `query` names, in opcode order (base, then CB): one for a
/// byte; every form of a mnemonic given alone (`jr`, `ret`); otherwise the
/// one form an instruction with all its operands assembles to. An operand
/// that isn't a register, condition or placeholder is data, so `ld a, 5`
/// finds `ld a, n8` and `ld [wFoo], a` finds `ld [a16], a`.
pub fn lookup(query: &str) -> Result<Vec<Form>, LookupError> {
    // A pasted line may carry its comment.
    let query = query.split(';').next().unwrap_or_default().trim();
    if query.is_empty() {
        return Err(LookupError::Empty);
    }
    if let Some(digits) = query
        .strip_prefix('$')
        .or_else(|| query.strip_prefix("0x"))
        .or_else(|| query.strip_prefix("0X"))
    {
        return byte(digits)
            .map(|op| vec![form(op)])
            .ok_or_else(|| LookupError::BadByte { text: query.into() });
    }
    let lower = query.to_ascii_lowercase();
    let (word, rest) = lower
        .split_once(char::is_whitespace)
        .unwrap_or((&lower, ""));
    let written: Vec<String> = if rest.trim().is_empty() {
        Vec::new()
    } else {
        rest.split(',')
            .map(|t| t.chars().filter(|c| !c.is_whitespace()).collect())
            .collect()
    };
    let spelled = spell(query, word, &written);
    if let Some(gaps) = missing_commas(query) {
        if let Err(e @ LookupError::NoMnemonic { .. }) = spelled {
            return Err(e);
        }
        // Only `ldi` and `ldd` aren't the table's names: both are `ld`.
        let mnemonic = by_name(word).unwrap_or(Mnemonic::Ld);
        let fixed = (lookup(&gaps).is_ok()).then_some(gaps);
        return Err(LookupError::NoForm {
            query: query.into(),
            mnemonic: mnemonic_name(mnemonic),
            forms: all().filter(|&op| info(op).mnemonic == mnemonic).count(),
            fix: Some(Fix::Comma(fixed)),
        });
    }
    let (mnemonic, tokens) = spelled?;
    let of_mnemonic: Vec<Opcode> = all().filter(|&op| info(op).mnemonic == mnemonic).collect();
    let found: Vec<Form> = of_mnemonic
        .iter()
        .filter(|&&op| tokens.is_empty() || exact(&slots(info(op)), &tokens))
        .map(|&op| form(op))
        .collect();
    if found.is_empty() {
        return Err(LookupError::NoForm {
            query: query.into(),
            mnemonic: mnemonic_name(mnemonic),
            forms: of_mnemonic.len(),
            fix: fix(&lower, mnemonic, &tokens, &of_mnemonic),
        });
    }
    Ok(found)
}

/// The mnemonic and operands as the table writes them, from a line's word
/// and its operands (whitespace removed): rgbasm's other spellings of the
/// same instruction (`ldi`/`ldd`, `[hli]`, `[$ff00+c]`, the ALU ops'
/// implied `a`, `cpl a`) mapped to the table's.
fn spell(
    query: &str,
    word: &str,
    written: &[String],
) -> Result<(Mnemonic, Vec<String>), LookupError> {
    let refuse = |mnemonic: Mnemonic, fix: Option<Fix>| LookupError::NoForm {
        query: query.into(),
        mnemonic: mnemonic_name(mnemonic),
        forms: all().filter(|&op| info(op).mnemonic == mnemonic).count(),
        fix,
    };
    // rgbasm's `ldi`/`ldd` take `a` and plain `[hl]`, either way round.
    if let Some(step) = match word {
        "ldi" => Some("+"),
        "ldd" => Some("-"),
        _ => None,
    } {
        let marked = format!("[hl{step}]");
        let tokens: Vec<String> = written
            .iter()
            .map(|t| {
                if t == "[hl]" {
                    marked.clone()
                } else {
                    t.clone()
                }
            })
            .collect();
        return match written {
            [a, hl] | [hl, a] if a == "a" && hl == "[hl]" => Ok((Mnemonic::Ld, tokens)),
            _ => {
                // `ldi a, [hli]` means `ld a, [hli]`; otherwise show the form.
                let as_ld = format!("ld {}", tokens.join(", "));
                let fix = if lookup(&as_ld).is_ok() {
                    as_ld
                } else {
                    format!("{word} a, [hl]")
                };
                Err(refuse(Mnemonic::Ld, Some(Fix::Write(fix))))
            }
        };
    }
    let Some(mut mnemonic) = by_name(word) else {
        return Err(LookupError::NoMnemonic {
            word: word.into(),
            near: near(word),
        });
    };
    if written.iter().any(String::is_empty) {
        return Err(refuse(mnemonic, None));
    }
    if mnemonic == Mnemonic::Ld {
        // $FF00+C is `ldh [c]`, or `[$ff00+c]` with either mnemonic; rgbasm
        // has no `ld [c]`.
        if written.iter().any(|t| t == "[c]") {
            let fixed = written
                .iter()
                .map(String::as_str)
                .collect::<Vec<_>>()
                .join(", ");
            return Err(refuse(mnemonic, Some(Fix::Write(format!("ldh {fixed}")))));
        }
        if written.iter().any(|t| is_high_c(t)) {
            mnemonic = Mnemonic::Ldh;
        }
    }
    let mut tokens: Vec<String> = written.iter().map(|t| token(t)).collect();
    if is_alu8(mnemonic) && tokens.len() == 1 {
        tokens.insert(0, "a".into());
    }
    if mnemonic == Mnemonic::Cpl && tokens == ["a"] {
        tokens.clear();
    }
    Ok((mnemonic, tokens))
}

/// `query` with a comma where whitespace separates two operands
/// (`jr nz Top`), which rgbasm refuses: whitespace between two names or
/// numbers, with no operator, outside a string. `None` when there is none.
fn missing_commas(query: &str) -> Option<String> {
    let (word, rest) = query.split_once(char::is_whitespace)?;
    let ends = |c: u8| c.is_ascii_alphanumeric() || b"_.#@'\")]".contains(&c);
    let starts = |c: u8| c.is_ascii_alphanumeric() || b"_.#@$'\"[`".contains(&c);
    let mut found = false;
    let operands: Vec<String> = rest
        .split(',')
        .map(|operand| {
            let bytes = operand.trim().as_bytes();
            let mut out = String::new();
            let (mut i, mut last) = (0, None);
            while i < bytes.len() {
                let c = bytes[i];
                if c == b'"' || c == b'\'' {
                    let end = bytes[i + 1..]
                        .iter()
                        .position(|&b| b == c)
                        .map_or(bytes.len(), |p| i + 2 + p);
                    out.push_str(&String::from_utf8_lossy(&bytes[i..end]));
                    last = Some(c);
                    i = end;
                } else if c.is_ascii_whitespace() {
                    let next = i + bytes[i..]
                        .iter()
                        .take_while(|b| b.is_ascii_whitespace())
                        .count();
                    let gap = last.is_some_and(ends) && bytes.get(next).is_some_and(|&n| starts(n));
                    found |= gap;
                    out.push_str(if gap { ", " } else { " " });
                    i = next;
                } else {
                    out.push(char::from(c));
                    last = Some(c);
                    i += 1;
                }
            }
            out
        })
        .collect();
    found.then(|| format!("{word} {}", operands.join(", ")))
}

/// The 8-bit ALU ops, whose `a,` rgbasm lets a line leave out.
fn is_alu8(m: Mnemonic) -> bool {
    use Mnemonic::*;
    matches!(m, Add | Adc | Sub | Sbc | And | Xor | Or | Cp)
}

/// Whether every operand matches its slot, and there are as many of each.
fn exact(slots: &[Slot], tokens: &[String]) -> bool {
    tokens.len() == slots.len() && tokens.iter().zip(slots).all(|(t, s)| s.matches(t))
}

/// The spelling rgbasm takes that a refused line was nearest to.
fn fix(lower: &str, mnemonic: Mnemonic, tokens: &[String], forms: &[Opcode]) -> Option<Fix> {
    if lower.contains('(') {
        let bracketed = lower.replace('(', "[").replace(')', "]");
        if lookup(&bracketed).is_ok() {
            return Some(Fix::Write(bracketed));
        }
    }
    if mnemonic == Mnemonic::Jp && tokens == ["[hl]"] {
        return Some(Fix::Write("jp hl".into()));
    }
    if mnemonic == Mnemonic::Ldh
        && tokens.iter().any(|t| {
            t.strip_prefix('[')
                .and_then(|t| t.strip_suffix(']'))
                .and_then(literal)
                .is_some_and(|v| !(0xFF00..=0xFFFF).contains(&v))
        })
    {
        return Some(Fix::LdhRange);
    }
    let started: Vec<String> = forms
        .iter()
        .filter(|&&op| {
            let slots = slots(info(op));
            tokens.len() < slots.len() && tokens.iter().zip(&slots).all(|(t, s)| s.matches(t))
        })
        .map(|&op| asm(info(op)))
        .collect();
    (!started.is_empty()).then_some(Fix::Complete(started))
}

fn info(opcode: Opcode) -> &'static OpInfo {
    match opcode {
        Opcode::Base(b) => &table::BASE[usize::from(b)],
        Opcode::Cb(b) => &table::CB[usize::from(b)],
    }
}

fn all() -> impl Iterator<Item = Opcode> {
    (0..=255).map(Opcode::Base).chain((0..=255).map(Opcode::Cb))
}

fn byte(digits: &str) -> Option<Opcode> {
    // Hex digits only: from_str_radix takes a sign, and slicing needs ASCII.
    if !digits.chars().all(|c| c.is_ascii_hexdigit()) {
        return None;
    }
    let parse = |d: &str| u8::from_str_radix(d, 16).ok();
    match digits.len() {
        1 | 2 => parse(digits).map(Opcode::Base),
        4 if digits[..2].eq_ignore_ascii_case("cb") => parse(&digits[2..]).map(Opcode::Cb),
        _ => None,
    }
}

/// Mnemonics an instruction can be written with: not the prefix byte or
/// the opcodes with no instruction.
fn searchable(m: Mnemonic) -> bool {
    !matches!(m, Mnemonic::Prefix | Mnemonic::Illegal)
}

/// Lowercase, as RGBDS sources are usually written.
fn mnemonic_name(m: Mnemonic) -> String {
    m.as_str().to_ascii_lowercase()
}

fn mnemonics() -> Vec<String> {
    let mut seen: Vec<Mnemonic> = Vec::new();
    for op in all() {
        let m = info(op).mnemonic;
        if searchable(m) && !seen.contains(&m) {
            seen.push(m);
        }
    }
    seen.into_iter().map(mnemonic_name).collect()
}

fn by_name(word: &str) -> Option<Mnemonic> {
    all()
        .map(|op| info(op).mnemonic)
        .find(|&m| searchable(m) && mnemonic_name(m) == word)
}

fn near(word: &str) -> Vec<String> {
    let mut near: Vec<(usize, String)> = mnemonics()
        .into_iter()
        .map(|m| (crate::sym::edit_distance(word, &m), m))
        .filter(|&(d, _)| d <= 2)
        .collect();
    near.sort();
    near.into_iter().map(|(_, m)| m).take(5).collect()
}

/// One operand, whitespace removed, with rgbasm's other spellings of the
/// same operand mapped to the table's.
fn token(t: &str) -> String {
    match t {
        "[hli]" => "[hl+]".into(),
        "[hld]" => "[hl-]".into(),
        // rgbasm names a register pair's halves so.
        "high(af)" => "a".into(),
        "high(bc)" => "b".into(),
        "low(bc)" => "c".into(),
        "high(de)" => "d".into(),
        "low(de)" => "e".into(),
        "high(hl)" => "h".into(),
        "low(hl)" => "l".into(),
        _ if is_high_c(t) => "[c]".into(),
        _ => t.into(),
    }
}

/// `[$ff00+c]`, with $FF00 in any base rgbasm reads.
fn is_high_c(t: &str) -> bool {
    t.strip_prefix('[')
        .and_then(|t| t.strip_suffix("+c]"))
        .and_then(literal)
        == Some(0xFF00)
}

/// An operand position of a form, and what a query may write there.
enum Slot {
    /// A register, a condition or `[hl+]`: only its own spelling.
    Fixed(String),
    /// Immediate data (`n8`, `n16`, `a16`, `e8`): any value or label.
    Value(&'static str),
    /// `[a16]` or `[a8]`: any value or label in brackets; a number in
    /// `[a8]` (ldh) must be $FF00-$FFFF.
    Address(&'static str),
    /// `sp+e8`.
    SpOffset,
    /// An RST vector or a bit number: a constant expression of that value.
    Number { text: &'static str, value: u8 },
}

impl Slot {
    fn text(&self) -> String {
        match self {
            Slot::Fixed(s) => s.clone(),
            Slot::Value(p) => (*p).into(),
            Slot::Address(p) => format!("[{p}]"),
            Slot::SpOffset => "sp+e8".into(),
            Slot::Number { text, .. } => (*text).into(),
        }
    }

    fn matches(&self, token: &str) -> bool {
        if token == self.text() {
            return true;
        }
        match self {
            Slot::Fixed(_) => false,
            Slot::Value(_) => is_value(token),
            Slot::Address(p) => token
                .strip_prefix('[')
                .and_then(|t| t.strip_suffix(']'))
                .is_some_and(|t| {
                    is_value(t)
                        && (*p != "a8" || literal(t).is_none_or(|v| (0xFF00..=0xFFFF).contains(&v)))
                }),
            Slot::SpOffset => {
                let offset = token.strip_prefix("sp+").or_else(|| {
                    // `sp-2`: the sign belongs to the offset.
                    token.strip_prefix("sp").filter(|t| t.starts_with('-'))
                });
                offset.is_some_and(is_value)
            }
            Slot::Number { value, .. } => expr::eval(token) == Some(i32::from(*value)),
        }
    }
}

/// The table's placeholders, which a query may write for themselves.
const PLACEHOLDERS: &[&str] = &["n8", "n16", "a8", "a16", "e8"];

/// rgbasm's register and condition names: an operand that names one is not
/// data (`(hl)`, `hl+1`, `a+1`).
const REGISTERS: &[&str] = &[
    "a", "b", "c", "d", "e", "h", "l", "af", "bc", "de", "hl", "hli", "hld", "sp", "nz", "z", "nc",
];

/// Data or a label: an expression that names no register, condition or
/// placeholder, and no memory.
fn is_value(token: &str) -> bool {
    !token.is_empty()
        && !PLACEHOLDERS.contains(&token)
        && !token.contains(['[', ']'])
        && !words(token).any(|w| REGISTERS.contains(&w))
}

/// The identifiers in an expression: not numbers (`$c`, `0xc`, `%10`) or
/// the insides of string and character literals.
fn words(expr: &str) -> impl Iterator<Item = &str> {
    let bytes = expr.as_bytes();
    let mut i = 0;
    std::iter::from_fn(move || {
        while i < bytes.len() {
            let start = i;
            let c = bytes[i];
            let word = |c: u8| c.is_ascii_alphanumeric() || b"_.#@".contains(&c);
            if c == b'"' || c == b'\'' {
                i += 1;
                while i < bytes.len() && bytes[i] != c {
                    i += 1;
                }
                i += 1;
            } else if c == b'$' || c == b'%' || c == b'&' || c.is_ascii_digit() {
                i += 1;
                while i < bytes.len() && bytes[i].is_ascii_alphanumeric() {
                    i += 1;
                }
            } else if word(c) {
                while i < bytes.len() && word(bytes[i]) {
                    i += 1;
                }
                return Some(&expr[start..i]);
            } else {
                i += 1;
            }
        }
        None
    })
}

/// A number written as rgbasm writes one: `$FF00`, `0xFF00`, `%101`,
/// `0b101`, `&17`, `0o17`, or decimal.
fn literal(text: &str) -> Option<u32> {
    let (digits, radix) = if let Some(d) = text.strip_prefix('$').or(text.strip_prefix("0x")) {
        (d, 16)
    } else if let Some(d) = text.strip_prefix('%').or(text.strip_prefix("0b")) {
        (d, 2)
    } else if let Some(d) = text.strip_prefix('&').or(text.strip_prefix("0o")) {
        (d, 8)
    } else {
        (text, 10)
    };
    if digits.is_empty() || !digits.chars().all(|c| c.is_digit(radix)) {
        return None;
    }
    u32::from_str_radix(digits, radix).ok()
}

fn slots(info: &OpInfo) -> Vec<Slot> {
    let mut slots = Vec::new();
    let mut specs = info.operands.iter().peekable();
    while let Some(spec) = specs.next() {
        use OperandKind as K;
        let name = spec.kind.name();
        slots.push(match spec.kind {
            // The table's `SP` with `increment` is `SP+e8`; its e8 follows.
            K::Sp if spec.increment && !spec.indirect => {
                specs.next();
                Slot::SpOffset
            }
            K::N8 | K::N16 | K::A8 | K::A16 | K::Rel8 | K::E8 => {
                if spec.indirect {
                    Slot::Address(name)
                } else {
                    Slot::Value(name)
                }
            }
            K::Vector(value) | K::Bit(value) => Slot::Number { text: name, value },
            _ => {
                let reg = name.to_ascii_lowercase();
                Slot::Fixed(match (spec.indirect, spec.increment, spec.decrement) {
                    (true, true, _) => format!("[{reg}+]"),
                    (true, _, true) => format!("[{reg}-]"),
                    (true, ..) => format!("[{reg}]"),
                    _ => reg,
                })
            }
        });
    }
    slots
}

fn asm(info: &OpInfo) -> String {
    let operands: Vec<String> = slots(info).iter().map(Slot::text).collect();
    let name = mnemonic_name(info.mnemonic);
    if operands.is_empty() {
        name
    } else {
        format!("{name} {}", operands.join(", "))
    }
}
