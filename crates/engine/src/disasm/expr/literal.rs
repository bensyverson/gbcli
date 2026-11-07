//! rgbasm 1.0.4's literals, as its lexer reads them: numbers, fixed-point
//! numbers, graphics constants, character constants and strings. Each
//! function takes the text from the literal's first byte and returns its
//! value and its length in bytes, or `None` where rgbasm refuses it.

use super::Value;

/// A number at the start of `s`: every base rgbasm reads (`$`, `0x`, `%`,
/// `0b`, `&`, `0o`, decimal), with single `_`s between digits or after a
/// base prefix (`$_3`), never at the end; or a decimal fixed-point number
/// (`1.5`, `5.`, `1.5q8`), which is read but not valued.
pub(super) fn number(s: &[u8]) -> Option<(Value, usize)> {
    let (prefix, radix) = match s {
        [b'0', b'x' | b'X', ..] => (2, 16),
        [b'0', b'b' | b'B', ..] => (2, 2),
        [b'0', b'o' | b'O', ..] => (2, 8),
        [b'$', ..] => (1, 16),
        [b'%', ..] => (1, 2),
        [b'&', ..] => (1, 8),
        _ => (0, 10),
    };
    let len = prefix + word_len(&s[prefix..]);
    let value = digits(&s[prefix..len], radix)?;
    if radix != 10 || s.get(len) != Some(&b'.') {
        return Some((Value::Known(value), len));
    }
    // `5.`, `1.5`, `1.5_5`, `1.5q8`: never `_` first or last, and `q` takes
    // a precision.
    let fraction = &s[len + 1..];
    let flen = word_len(fraction);
    let (digits, precision) = match fraction[..flen]
        .iter()
        .position(|c| matches!(c, b'q' | b'Q'))
    {
        Some(q) => (&fraction[..q], Some(&fraction[q + 1..flen])),
        None => (&fraction[..flen], None),
    };
    let fine = |d: &[u8]| {
        d.first() != Some(&b'_')
            && d.last() != Some(&b'_')
            && !d.windows(2).any(|w| w == b"__")
            && d.iter().all(|c| c.is_ascii_digit() || *c == b'_')
    };
    let precision_fine =
        precision.is_none_or(|p| !p.is_empty() && p.iter().all(u8::is_ascii_digit));
    (fine(digits) && precision_fine).then_some((Value::Unknown, len + 1 + flen))
}

/// How many bytes of `s` belong to one number token: rgbasm's lexer reads
/// letters, digits and `_` into it, so `5x` is one bad number.
fn word_len(s: &[u8]) -> usize {
    s.iter()
        .take_while(|c| c.is_ascii_alphanumeric() || **c == b'_')
        .count()
}

/// The value of `text`'s digits in `radix`. A leading `_` makes a decimal an
/// identifier; elsewhere `_` may stand between digits or after a base
/// prefix, never doubled or last. Past 32 bits the value is 0, as rgbasm
/// 1.0.4 makes it (with a warning, -Wlarge-constant).
fn digits(text: &[u8], radix: u32) -> Option<i32> {
    if text.is_empty()
        || text.last() == Some(&b'_')
        || text.windows(2).any(|w| w == b"__")
        || (radix == 10 && text[0] == b'_')
    {
        return None;
    }
    let mut value: u64 = 0;
    let mut wide = false;
    let mut any = false;
    for &c in text {
        if c == b'_' {
            continue;
        }
        let digit = char::from(c).to_digit(radix)?;
        any = true;
        value = value * u64::from(radix) + u64::from(digit);
        if value > u64::from(u32::MAX) {
            wide = true;
            value = 0;
        }
    }
    any.then_some(if wide { 0 } else { value as u32 as i32 })
}

/// A graphics constant at the start of `s` (`` `01012323 ``): one to eight
/// pixels of 0 to 3, with `_` between them. Read but not valued.
pub(super) fn graphics(s: &[u8]) -> Option<(Value, usize)> {
    let len = 1 + word_len(&s[1..]);
    let body = &s[1..len];
    let pixels = body.iter().filter(|&&c| c != b'_').count();
    let fine = (1..=8).contains(&pixels)
        && body.iter().all(|c| matches!(c, b'0'..=b'3' | b'_'))
        && body.last() != Some(&b'_')
        && !body.windows(2).any(|w| w == b"__");
    fine.then_some((Value::Unknown, len))
}

/// A character constant (`'x'`) or a one-character string (`"x"`, which
/// rgbasm still reads as a number) at the start of `s`, quoted by `s[0]`:
/// its value in the default charmap and its length in bytes. One ASCII
/// character or escape; `{` starts an interpolation, which isn't read.
pub(super) fn character(s: &[u8]) -> Option<(Value, usize)> {
    let quote = s[0];
    let (value, len) = match *s.get(1)? {
        b'\\' => {
            let value = match *s.get(2)? {
                c @ (b'\\' | b'"' | b'\'' | b'{' | b'}') => c,
                b'n' => b'\n',
                b'r' => b'\r',
                b't' => b'\t',
                b'0' => 0,
                _ => return None,
            };
            (value, 2)
        }
        b'{' => return None,
        c if c == quote || !c.is_ascii() => return None,
        c => (c, 1),
    };
    (s.get(1 + len) == Some(&quote)).then_some((Value::Known(i32::from(value)), len + 2))
}

/// The length of the string literal at the start of `s` (`"…"`, a
/// backslash escaping the byte after it), or `None` if it never closes.
pub(super) fn string_len(s: &[u8]) -> Option<usize> {
    let mut i = 1;
    while i < s.len() {
        match s[i] {
            b'\\' => i += 2,
            b'"' => return Some(i + 1),
            _ => i += 1,
        }
    }
    None
}
