//! Finding opcodes in the table by byte (`$20`, `$CB37`) or by the
//! instruction as RGBDS spells it (`jr nz`, `ld a, [hl+]`, `ld [wFoo], a`):
//! each form's size, cycles and flags, for `gbcli opcode`.

use std::fmt;

use super::Opcode;
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
    NoForm {
        query: String,
        mnemonic: String,
        forms: usize,
    },
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
            } => write!(f, "no form of `{mnemonic}` matches `{query}`"),
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
                mnemonic, forms, ..
            } => format!("`gbcli opcode {mnemonic}` lists its {forms} forms"),
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
/// byte; for an instruction, every form whose leading operands match. An
/// operand that isn't a register, condition or placeholder is data, so
/// `ld a, 5` finds `ld a, n8` and `ld [wFoo], a` finds `ld [a16], a`.
pub fn lookup(query: &str) -> Result<Vec<Form>, LookupError> {
    let query = query.trim();
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
    let Some(mnemonic) = by_name(word) else {
        return Err(LookupError::NoMnemonic {
            word: word.into(),
            near: near(word),
        });
    };
    let tokens: Vec<String> = if rest.trim().is_empty() {
        Vec::new()
    } else {
        rest.split(',').map(token).collect()
    };
    let mut forms = 0;
    let found: Vec<Form> = all()
        .filter(|&op| info(op).mnemonic == mnemonic)
        .inspect(|_| forms += 1)
        .filter(|&op| {
            let slots = slots(info(op));
            tokens.len() <= slots.len() && tokens.iter().zip(&slots).all(|(t, s)| s.matches(t))
        })
        .map(form)
        .collect();
    if found.is_empty() {
        return Err(LookupError::NoForm {
            query: query.into(),
            mnemonic: mnemonic_name(mnemonic),
            forms,
        });
    }
    Ok(found)
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

/// One query operand, spaces removed, with RGBDS's other spellings of the
/// same operand mapped to the table's.
fn token(text: &str) -> String {
    let t: String = text.chars().filter(|c| !c.is_whitespace()).collect();
    match t.as_str() {
        "[hli]" => "[hl+]".into(),
        "[hld]" => "[hl-]".into(),
        "[$ff00+c]" | "[0xff00+c]" => "[c]".into(),
        _ => t,
    }
}

/// An operand position of a form, and what a query may write there.
enum Slot {
    /// A register, a condition or `[hl+]`: only its own spelling.
    Fixed(String),
    /// Immediate data (`n8`, `n16`, `a16`, `e8`): any value or label.
    Value(&'static str),
    /// `[a16]` or `[a8]`: any value or label in brackets.
    Address(&'static str),
    /// `sp+e8`.
    SpOffset,
    /// An RST vector or a bit number: that number, in any base.
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
            Slot::Address(_) => token
                .strip_prefix('[')
                .and_then(|t| t.strip_suffix(']'))
                .is_some_and(is_value),
            Slot::SpOffset => {
                let offset = token.strip_prefix("sp+").or_else(|| {
                    // `sp-2`: the sign belongs to the offset.
                    token.strip_prefix("sp").filter(|t| t.starts_with('-'))
                });
                offset.is_some_and(is_value)
            }
            Slot::Number { value, .. } => number(token) == Some(*value),
        }
    }
}

const RESERVED: &[&str] = &[
    "a", "b", "c", "d", "e", "h", "l", "af", "bc", "de", "hl", "hl+", "hl-", "hli", "hld", "sp",
    "nz", "z", "nc", "n8", "n16", "a8", "a16", "e8",
];

/// Data or a label: not a register, condition or placeholder, not memory,
/// and not `sp±offset`.
fn is_value(token: &str) -> bool {
    !token.is_empty()
        && !RESERVED.contains(&token)
        && !token.contains(['[', ']'])
        && !token.starts_with("sp+")
        && !token.starts_with("sp-")
}

fn number(token: &str) -> Option<u8> {
    if let Some(hex) = token.strip_prefix('$').or_else(|| token.strip_prefix("0x")) {
        u8::from_str_radix(hex, 16).ok()
    } else {
        token.parse().ok()
    }
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
