//! How each value is spelled, both ways. A value outside its readable range
//! (one a file can hold but hUGETracker never writes) is spelled `!N`, its
//! stored number, so nothing a file holds is lost.

use crate::model::{Cell, MIDDLE_NOTE, NO_NOTE};
use crate::notes::{note_name, note_of};

/// The spellings of a choice field, readable word → stored value.
pub type Choices = &'static [(&'static str, i64)];

pub const DIRECTIONS: Choices = &[("up", 0), ("down", 1)];
pub const DUTIES: Choices = &[("12.5%", 0), ("25%", 1), ("50%", 2), ("75%", 3)];
pub const LEVELS: Choices = &[("0%", 0), ("100%", 1), ("50%", 2), ("25%", 3)];
pub const MODES: Choices = &[("15-bit", 0), ("7-bit", 1)];
pub const SWITCH: Choices = &[("off", 0), ("on", 1)];

/// `!N`.
pub fn bang(n: impl Into<i64>) -> String {
    format!("!{}", n.into())
}

/// `n` in decimal when it is in `min..=max`, else `!n`.
pub fn number(n: impl Into<i64>, min: i64, max: i64) -> String {
    let n = n.into();
    if (min..=max).contains(&n) {
        n.to_string()
    } else {
        bang(n)
    }
}

/// The word for `n`, or `!n`.
pub fn choice(n: impl Into<i64>, choices: Choices) -> String {
    let n = n.into();
    choices
        .iter()
        .find(|(_, v)| *v == n)
        .map_or_else(|| bang(n), |(word, _)| (*word).to_string())
}

/// The stored number a `!N` token holds.
pub fn read_bang(token: &str) -> Option<i64> {
    decimal(token.strip_prefix('!')?)
}

/// A decimal as the text spells one: an optional `-`, then digits. (`str::
/// parse` also takes a leading `+`, which the text never writes.)
pub fn decimal<T: std::str::FromStr>(text: &str) -> Option<T> {
    let digits = text.strip_prefix('-').unwrap_or(text);
    let plain = !digits.is_empty() && digits.bytes().all(|b| b.is_ascii_digit());
    plain.then(|| text.parse().ok()).flatten()
}

/// Two hex digits as a byte. (`from_str_radix` also takes a leading `+`.)
pub fn hex_byte(text: &str) -> Option<u8> {
    let plain = text.len() == 2 && text.bytes().all(|b| b.is_ascii_hexdigit());
    plain.then(|| u8::from_str_radix(text, 16).ok()).flatten()
}

pub fn note(n: i32) -> String {
    if n == NO_NOTE {
        return "...".into();
    }
    note_name(n).unwrap_or_else(|| bang(n))
}

pub fn read_note(token: &str) -> Option<i32> {
    match token {
        "..." => Some(NO_NOTE),
        _ => note_of(token).or_else(|| stored_i32(token)),
    }
}

/// A subpattern note: its offset from [`MIDDLE_NOTE`], as hUGETracker shows
/// it (`+00`, `-05`).
pub fn offset(n: i32) -> String {
    match n {
        NO_NOTE => "...".into(),
        0..=crate::model::HIGHEST_NOTE if n >= MIDDLE_NOTE => format!("+{:02}", n - MIDDLE_NOTE),
        0..=crate::model::HIGHEST_NOTE => format!("-{:02}", MIDDLE_NOTE - n),
        _ => bang(n),
    }
}

pub fn read_offset(token: &str) -> Option<i32> {
    if token == "..." {
        return Some(NO_NOTE);
    }
    let digits = |s: &str| {
        (s.len() == 2 && s.bytes().all(|b| b.is_ascii_digit()))
            .then(|| s.parse::<i32>().ok())
            .flatten()
    };
    let n = if let Some(up) = token.strip_prefix('+') {
        MIDDLE_NOTE + digits(up)?
    } else if let Some(down) = token.strip_prefix('-') {
        MIDDLE_NOTE - digits(down)?
    } else {
        return stored_i32(token);
    };
    (0..=crate::model::HIGHEST_NOTE).contains(&n).then_some(n)
}

pub fn instrument(n: i32) -> String {
    match n {
        0 => "..".into(),
        1..=15 => format!("{n:02}"),
        _ => bang(n),
    }
}

pub fn read_instrument(token: &str) -> Option<i32> {
    match token {
        // hUGETracker's clipboard writes no instrument as 00.
        ".." | "00" => Some(0),
        _ if token.len() == 2 && token.bytes().all(|b| b.is_ascii_digit()) => {
            let n: i32 = token.parse().ok()?;
            (1..=15).contains(&n).then_some(n)
        }
        _ => stored_i32(token),
    }
}

/// A pattern cell's volume, which hUGEDriver doesn't use: `...`.
pub fn volume(n: i32) -> String {
    if n == 0 { "...".into() } else { bang(n) }
}

pub fn read_volume(token: &str) -> Option<i32> {
    if token == "..." {
        Some(0)
    } else {
        stored_i32(token)
    }
}

/// A subpattern row's jump target (1-based), as hUGETracker shows it.
pub fn jump(n: i32) -> String {
    match n {
        0 => "...".into(),
        1..=99 => format!("J{n:02}"),
        _ => bang(n),
    }
}

pub fn read_jump(token: &str) -> Option<i32> {
    if token == "..." {
        return Some(0);
    }
    if let Some(d) = token.strip_prefix('J') {
        let n: i32 = (d.len() == 2).then(|| decimal(d)).flatten()?;
        return (1..=99).contains(&n).then_some(n);
    }
    stored_i32(token)
}

/// An effect: `...` for none, three hex digits, or `!CODE:PARAM` for a
/// code past F.
pub fn effect(code: i32, param: u8) -> String {
    match code {
        0 if param == 0 => "...".into(),
        0..=15 => format!("{code:X}{param:02X}"),
        _ => format!("!{code}:{param}"),
    }
}

pub fn read_effect(token: &str) -> Option<(i32, u8)> {
    if token == "..." {
        return Some((0, 0));
    }
    if let Some(rest) = token.strip_prefix('!') {
        let (code, param) = rest.split_once(':')?;
        return Some((decimal(code)?, decimal(param)?));
    }
    if token.len() != 3 || !token.bytes().all(|b| b.is_ascii_hexdigit()) {
        return None;
    }
    let code = i32::from_str_radix(&token[..1], 16).ok()?;
    let param = u8::from_str_radix(&token[1..], 16).ok()?;
    Some((code, param))
}

fn stored_i32(token: &str) -> Option<i32> {
    read_bang(token).and_then(|n| i32::try_from(n).ok())
}

/// A pattern cell: note, instrument, volume, effect.
pub fn cell(c: &Cell) -> [String; 4] {
    [
        note(c.note),
        instrument(c.instrument),
        volume(c.volume),
        effect(c.effect_code, c.effect_param),
    ]
}

/// A subpattern cell: offset, instrument, jump, effect.
pub fn sub_cell(c: &Cell) -> [String; 4] {
    [
        offset(c.note),
        instrument(c.instrument),
        jump(c.volume),
        effect(c.effect_code, c.effect_param),
    ]
}

/// `bytes` as a quoted string: UTF-8 as is, except `"`, `\` and control
/// characters, which are escaped, and bytes that aren't UTF-8, as `\xNN`.
pub fn quote(bytes: &[u8]) -> String {
    let mut out = String::from("\"");
    for chunk in bytes.utf8_chunks() {
        for c in chunk.valid().chars() {
            match c {
                '"' => out += "\\\"",
                '\\' => out += "\\\\",
                '\n' => out += "\\n",
                '\r' => out += "\\r",
                '\t' => out += "\\t",
                c if c.is_control() => {
                    let mut buf = [0; 4];
                    for b in c.encode_utf8(&mut buf).bytes() {
                        out += &format!("\\x{b:02X}");
                    }
                }
                c => out.push(c),
            }
        }
        for b in chunk.invalid() {
            out += &format!("\\x{b:02X}");
        }
    }
    out.push('"');
    out
}
