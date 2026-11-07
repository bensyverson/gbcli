//! Finding opcodes in the table by byte (`$20`, `$CB37`) or by the
//! instruction as rgbasm 1.0.4 takes it (`jr nz, .loop`, `xor a`,
//! `ld a, [hl+]`, `ld [wFoo], a`): each form's size, cycles and flags, for
//! `gbcli opcode`. A line rgbasm refuses is refused, with the spelling it
//! takes where one is near (`crates/engine/tests/opcode_lookup_spellings.rs`
//! checks the cases against rgbasm itself), except for what needs the
//! source or the link: a symbol is taken as defined, a value as in range
//! for its slot (only `ldh`'s address is checked), and the arguments of a
//! function other than `HIGH()` and `LOW()` as the kind and number it
//! takes; and rgbasm's keywords other than registers and conditions read as
//! symbols. Every operand's expression is read by `expr` (data, an address,
//! sp's offset, an RST vector, a bit number, $FF00+C's base).

use std::fmt;

use super::Opcode;
use super::expr::{MAX_DEPTH, Refused, Value};
use super::operand::{
    Operand, Shape, lowercase_outside_quotes, missing_commas, norms, operands, without_comment,
};
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
    /// An operand's parentheses, unary operators, exponents and functions
    /// nest deeper than the evaluator follows (rgbasm takes deeper).
    TooDeep {
        mnemonic: String,
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
            LookupError::TooDeep { mnemonic } => write!(
                f,
                "an operand of `{mnemonic}` nests deeper than {MAX_DEPTH} levels"
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
            LookupError::TooDeep { .. } => format!(
                "nest parentheses, unary operators, `**` and HIGH()/LOW() at most {MAX_DEPTH} \
                 deep: work the inner part out first"
            ),
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
    let query = without_comment(query).trim();
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
    let lower = lowercase_outside_quotes(query);
    let (word, rest) = query.split_once(char::is_whitespace).unwrap_or((query, ""));
    let word = word.to_ascii_lowercase();
    let word = word.as_str();
    let written: Vec<Operand> = if rest.trim().is_empty() {
        Vec::new()
    } else {
        operands(rest).into_iter().map(Operand::written).collect()
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
    if tokens.iter().any(|t| t.data == Err(Refused::TooDeep)) {
        return Err(LookupError::TooDeep {
            mnemonic: mnemonic_name(mnemonic),
        });
    }
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
/// and its operands: rgbasm's other spellings of the same instruction
/// (`ldi`/`ldd`, `[hli]`, `[$ff00+c]`, the ALU ops' implied `a`, `cpl a`)
/// mapped to the table's.
fn spell(
    query: &str,
    word: &str,
    written: &[Operand],
) -> Result<(Mnemonic, Vec<Operand>), LookupError> {
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
        let tokens: Vec<Operand> = written
            .iter()
            .map(|t| {
                if t.norm == "[hl]" {
                    Operand::implied(&marked)
                } else {
                    t.clone()
                }
            })
            .collect();
        return match written {
            [a, hl] | [hl, a] if a.norm == "a" && hl.norm == "[hl]" => Ok((Mnemonic::Ld, tokens)),
            _ => {
                // `ldi a, [hli]` means `ld a, [hli]`; otherwise show the form.
                let as_ld = format!("ld {}", norms(&tokens).join(", "));
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
    if written.iter().any(|t| t.norm.is_empty()) {
        return Err(refuse(mnemonic, None));
    }
    if mnemonic == Mnemonic::Ld {
        // $FF00+C is `ldh [c]`, or `[$ff00+c]` with either mnemonic; rgbasm
        // has no `ld [c]`.
        if written.iter().any(|t| t.norm == "[c]") {
            let fixed = norms(written).join(", ");
            return Err(refuse(mnemonic, Some(Fix::Write(format!("ldh {fixed}")))));
        }
        if written.iter().any(|t| t.high_c) {
            mnemonic = Mnemonic::Ldh;
        }
    }
    let mut tokens: Vec<Operand> = written
        .iter()
        .map(|t| {
            t.renamed(if t.high_c {
                "[c]".into()
            } else {
                token(&t.norm)
            })
        })
        .collect();
    if is_alu8(mnemonic) && tokens.len() == 1 {
        tokens.insert(0, Operand::implied("a"));
    }
    if mnemonic == Mnemonic::Cpl && norms(&tokens) == ["a"] {
        tokens.clear();
    }
    Ok((mnemonic, tokens))
}

/// The 8-bit ALU ops, whose `a,` rgbasm lets a line leave out.
fn is_alu8(m: Mnemonic) -> bool {
    use Mnemonic::*;
    matches!(m, Add | Adc | Sub | Sbc | And | Xor | Or | Cp)
}

/// Whether every operand matches its slot, and there are as many of each.
fn exact(slots: &[Slot], tokens: &[Operand]) -> bool {
    tokens.len() == slots.len() && tokens.iter().zip(slots).all(|(t, s)| s.matches(t))
}

/// The spelling rgbasm takes that a refused line was nearest to.
fn fix(lower: &str, mnemonic: Mnemonic, tokens: &[Operand], forms: &[Opcode]) -> Option<Fix> {
    if lower.contains('(') {
        let bracketed = lower.replace('(', "[").replace(')', "]");
        if lookup(&bracketed).is_ok() {
            return Some(Fix::Write(bracketed));
        }
    }
    if mnemonic == Mnemonic::Jp && norms(tokens) == ["[hl]"] {
        return Some(Fix::Write("jp hl".into()));
    }
    if mnemonic == Mnemonic::Ldh
        && tokens.iter().any(|t| {
            t.shape == Shape::Memory
                && matches!(t.data, Ok(Value::Known(v)) if !LDH_RANGE.contains(&v))
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
        _ => t.into(),
    }
}

/// An operand position of a form, and what a query may write there.
enum Slot {
    /// A register, a condition or `[hl+]`: only its own spelling.
    Fixed(String),
    /// Immediate data (`n8`, `n16`, `a16`, `e8`): any expression rgbasm
    /// takes, its value unchecked.
    Value(&'static str),
    /// `[a16]` or `[a8]`: any expression rgbasm takes, in brackets; a known
    /// value in `[a8]` (ldh) must be $FF00-$FFFF.
    Address(&'static str),
    /// `sp+e8`: any expression after `sp+` or `sp-`.
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

    fn matches(&self, operand: &Operand) -> bool {
        let token = operand.norm.as_str();
        let data = |shape: Shape| operand.shape == shape && operand.data.is_ok();
        match self {
            Slot::Number { value, .. } => {
                operand.shape == Shape::Plain && operand.data == Ok(Value::Known(i32::from(*value)))
            }
            _ if token == self.text() => true,
            Slot::Fixed(_) => false,
            // A placeholder names its own slot: `n8` is not `n16`.
            Slot::Value(_) => data(Shape::Plain) && !PLACEHOLDERS.contains(&token),
            Slot::Address(p) => {
                let inside = token
                    .strip_prefix('[')
                    .and_then(|t| t.strip_suffix(']'))
                    .unwrap_or(token);
                data(Shape::Memory)
                    && !PLACEHOLDERS.contains(&inside)
                    && (*p != "a8"
                        || !matches!(operand.data, Ok(Value::Known(v)) if !LDH_RANGE.contains(&v)))
            }
            Slot::SpOffset => data(Shape::SpOffset),
        }
    }
}

/// The table's placeholders, which a query may write for themselves.
const PLACEHOLDERS: &[&str] = &["n8", "n16", "a8", "a16", "e8"];

/// The addresses `ldh` reaches.
const LDH_RANGE: std::ops::RangeInclusive<i32> = 0xFF00..=0xFFFF;

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
