//! Tokens of the condition language.

use super::ExprError;

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Tok {
    /// A number and the radix prefix it carried (`$`, `0x`, `%`).
    /// `value` is `None` for unprefixed digits that aren't decimal (`41AB`),
    /// which only a bank-qualified address (`03:41AB`, hex) accepts.
    Num {
        value: Option<i64>,
        raw: String,
        prefix: Prefix,
    },
    Ident(String),
    Op(&'static str),
    LBracket,
    RBracket,
    LParen,
    RParen,
    Colon,
}

/// The radix prefix a number was written with. A region offset or a
/// bank-qualified part takes hex alone, as the address grammar does.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Prefix {
    /// Bare digits: decimal in an expression, hex in an address part.
    None,
    /// `$` or `0x`.
    Hex,
    /// `%`.
    Binary,
}

const OPS: [&str; 20] = [
    "||", "&&", "==", "!=", "<=", ">=", "<<", ">>", "<", ">", "+", "-", "*", "/", "%", "&", "|",
    "^", "~", "!",
];

/// Tokens with their byte offsets.
pub fn lex(text: &str) -> Result<Vec<(usize, Tok)>, ExprError> {
    let bytes = text.as_bytes();
    let mut out: Vec<(usize, Tok)> = Vec::new();
    let mut i = 0;
    while i < bytes.len() {
        let c = bytes[i] as char;
        let start = i;
        if c.is_ascii_whitespace() {
            i += 1;
            continue;
        }
        let after_operand = matches!(
            out.last(),
            Some((
                _,
                Tok::Num { .. } | Tok::Ident(_) | Tok::RBracket | Tok::RParen
            ))
        );
        let take_while = |mut j: usize, f: fn(char) -> bool| {
            while j < bytes.len() && f(bytes[j] as char) {
                j += 1;
            }
            j
        };
        let tok = if c == '$' || (c == '%' && !after_operand) {
            let radix = if c == '$' { 16 } else { 2 };
            let end = take_while(i + 1, |c| c.is_ascii_alphanumeric());
            let digits = &text[i + 1..end];
            i = end;
            let prefix = if radix == 16 {
                Prefix::Hex
            } else {
                Prefix::Binary
            };
            number(text, start, digits, radix, prefix)?
        } else if c.is_ascii_digit() {
            let (radix, from) = if text[i..].starts_with("0x") || text[i..].starts_with("0X") {
                (16, i + 2)
            } else {
                (10, i)
            };
            let end = take_while(from, |c| c.is_ascii_alphanumeric());
            let digits = &text[from..end];
            i = end;
            let prefix = if radix == 16 {
                Prefix::Hex
            } else {
                Prefix::None
            };
            number(text, start, digits, radix, prefix)?
        } else if c.is_ascii_alphabetic() || c == '_' || c == '.' {
            let end = take_while(i, |c| {
                c.is_ascii_alphanumeric() || matches!(c, '_' | '.' | '#' | '@')
            });
            i = end;
            Tok::Ident(text[start..end].to_string())
        } else {
            i += 1;
            match c {
                '[' => Tok::LBracket,
                ']' => Tok::RBracket,
                '(' => Tok::LParen,
                ')' => Tok::RParen,
                ':' => Tok::Colon,
                _ => {
                    let op = OPS
                        .iter()
                        .find(|op| text[start..].starts_with(*op))
                        .ok_or_else(|| ExprError::syntax(start, format!("unexpected {c:?}")))?;
                    i = start + op.len();
                    Tok::Op(op)
                }
            }
        };
        out.push((start, tok));
    }
    Ok(out)
}

fn number(
    text: &str,
    at: usize,
    digits: &str,
    radix: u32,
    prefix: Prefix,
) -> Result<Tok, ExprError> {
    let value = i64::from_str_radix(digits, radix).ok();
    if value.is_none() && prefix != Prefix::None {
        let end = (at + digits.len() + 2).min(text.len());
        return Err(ExprError::syntax(
            at,
            format!("not a number: {:?}", &text[at..end]),
        ));
    }
    Ok(Tok::Num {
        value,
        raw: digits.to_string(),
        prefix,
    })
}
