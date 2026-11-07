//! Reading one token as a value, with an error that says what was wanted.

use super::lex::{Kind, Token};
use super::spell::{Choices, decimal, read_bang};
use crate::error::ParseError;
use crate::model::ShortString;

const BANG: &str = "a value a file stores outside that range is written !N (e.g. !20) and kept \
                    as stored";

/// An error at `token` on `line`.
pub fn at(
    line: u32,
    token: &Token,
    message: impl Into<String>,
    remedy: impl Into<String>,
) -> ParseError {
    ParseError {
        line,
        column: token.column,
        message: message.into(),
        remedy: remedy.into(),
    }
}

/// A decimal in `min..=max`, or `!N` (any i64; the caller checks it fits).
pub fn number(line: u32, token: &Token, min: i64, max: i64, what: &str) -> Result<i64, ParseError> {
    if let Some(n) = read_bang(&token.text) {
        return Ok(n);
    }
    match decimal::<i64>(&token.text) {
        Some(n) if (min..=max).contains(&n) => Ok(n),
        Some(n) => Err(at(
            line,
            token,
            format!("{n} is out of range for {what} ({min}-{max})"),
            format!("use {min}-{max}; {BANG}"),
        )),
        None => Err(at(
            line,
            token,
            format!("`{}` is not a number, and {what} is one", token.text),
            format!("write {what} as a decimal, {min}-{max}"),
        )),
    }
}

/// One of `choices`' words, or `!N`.
pub fn choice(line: u32, token: &Token, choices: Choices, what: &str) -> Result<i64, ParseError> {
    if let Some(n) = read_bang(&token.text) {
        return Ok(n);
    }
    choices
        .iter()
        .find(|(word, _)| *word == token.text)
        .map(|(_, v)| *v)
        .ok_or_else(|| {
            let words: Vec<&str> = choices.iter().map(|(w, _)| *w).collect();
            at(
                line,
                token,
                format!("`{}` is not {what}", token.text),
                format!("use one of: {}; {BANG}", words.join(", ")),
            )
        })
}

/// `n` as a stored byte.
pub fn byte(line: u32, token: &Token, n: i64, what: &str) -> Result<u8, ParseError> {
    u8::try_from(n).map_err(|_| {
        at(
            line,
            token,
            format!("{n} doesn't fit {what}, which is stored as a byte"),
            "a stored byte is 0-255",
        )
    })
}

/// `n` as a stored 32-bit integer.
pub fn int(line: u32, token: &Token, n: i64, what: &str) -> Result<i32, ParseError> {
    i32::try_from(n).map_err(|_| {
        at(
            line,
            token,
            format!("{n} doesn't fit {what}, which is stored as 32 bits"),
            "a stored integer is -2147483648 to 2147483647",
        )
    })
}

/// A quoted string of at most 255 bytes.
pub fn string(line: u32, token: &Token, what: &str) -> Result<ShortString, ParseError> {
    let Kind::Str(bytes) = &token.kind else {
        return Err(at(
            line,
            token,
            format!("{what} is a quoted string, not `{}`", token.text),
            format!("write it in double quotes, e.g. \"{}\"", token.text),
        ));
    };
    let len = bytes.len();
    ShortString::new(bytes.clone()).ok_or_else(|| {
        at(
            line,
            token,
            format!("{what} is {len} bytes; hUGETracker keeps at most 255"),
            "shorten it to 255 bytes or fewer",
        )
    })
}

/// Exactly `n` arguments after the line's first word, else an error that
/// gives the line's shape.
pub fn args<'a>(
    line: u32,
    tokens: &'a [Token],
    n: usize,
    shape: &str,
) -> Result<&'a [Token], ParseError> {
    let rest = &tokens[1..];
    if rest.len() == n {
        Ok(rest)
    } else {
        Err(at(
            line,
            &tokens[0],
            format!(
                "`{}` takes {n} values here, not {}",
                tokens[0].text,
                rest.len()
            ),
            format!("write it as: {shape}"),
        ))
    }
}

/// A row number, 00-63.
pub fn row(line: u32, token: &Token) -> Result<usize, ParseError> {
    let n = token
        .text
        .bytes()
        .all(|b| b.is_ascii_digit())
        .then(|| token.text.parse::<usize>().ok())
        .flatten();
    match n {
        Some(n) if n < crate::model::ROWS => Ok(n),
        Some(n) => Err(at(
            line,
            token,
            format!("row {n} is past the last row, 63"),
            "rows are numbered 00-63",
        )),
        None => Err(at(
            line,
            token,
            format!("`{}` is not a row number", token.text),
            "a row line starts with its number, 00-63",
        )),
    }
}
