//! An instruction line's text as `lookup` reads it: its comment, its
//! operands split at commas, and each operand with its data evaluated once.

use super::expr::{self, Refused, Value};

/// Where an operand's data sits.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Shape {
    /// The whole operand: `5`, `Top`, `a`.
    Plain,
    /// Inside brackets: `[wFoo]`, `[hl]`.
    Memory,
    /// After `sp+` or `sp-`: `sp+5`, `sp - 2`.
    SpOffset,
}

/// One operand of a line.
#[derive(Clone, Debug)]
pub(super) struct Operand {
    /// Lowercase with whitespace removed, rgbasm's other spellings mapped
    /// to the table's: what registers and placeholders are matched against.
    pub norm: String,
    pub shape: Shape,
    /// The data the operand holds as an expression (the [`Shape`] says
    /// which part), evaluated from the text as written, since case matters
    /// inside a character constant (`'C'`) and whitespace separates
    /// operators (`& &` is not `&&`, `$ 38` is not `$38`). Evaluated once,
    /// however many forms the operand is matched against.
    pub data: Result<Value, Refused>,
    /// `[$ff00+c]`: in brackets, `+ c` after an expression worth $FF00, in
    /// any base or spelling rgbasm reads (`[255*256+c]`).
    pub high_c: bool,
}

impl Operand {
    pub fn written(raw: &str) -> Operand {
        let raw = raw.trim();
        let norm = raw
            .chars()
            .filter(|c| !c.is_whitespace())
            .collect::<String>()
            .to_ascii_lowercase();
        let (shape, text) =
            if let Some(inner) = raw.strip_prefix('[').and_then(|r| r.strip_suffix(']')) {
                (Shape::Memory, inner)
            } else if let Some(offset) = sp_offset(raw) {
                (Shape::SpOffset, offset)
            } else {
                (Shape::Plain, raw)
            };
        let high_c = shape == Shape::Memory
            && high_c_base(text).is_some_and(|base| expr::eval(base) == Ok(Value::Known(0xFF00)));
        Operand {
            norm,
            shape,
            data: expr::eval(text),
            high_c,
        }
    }

    /// One the line leaves out, or rgbasm's spelling implies: a register.
    pub fn implied(norm: &str) -> Operand {
        Operand {
            norm: norm.into(),
            shape: Shape::Plain,
            data: Err(Refused::Invalid),
            high_c: false,
        }
    }

    /// This operand matched against `norm` instead.
    pub fn renamed(&self, norm: String) -> Operand {
        Operand {
            norm,
            ..self.clone()
        }
    }
}

/// The offset in `sp+5` or `sp - 2` (with its minus sign), if `operand` is
/// one.
fn sp_offset(operand: &str) -> Option<&str> {
    if !operand.get(..2)?.eq_ignore_ascii_case("sp") {
        return None;
    }
    let rest = operand[2..].trim_start();
    rest.strip_prefix('+')
        .or_else(|| rest.starts_with('-').then_some(rest))
}

/// The base in `$ff00 + c`, the inside of `[$ff00+c]`.
fn high_c_base(inner: &str) -> Option<&str> {
    inner
        .trim_end()
        .strip_suffix(['c', 'C'])?
        .trim_end()
        .strip_suffix('+')
}

pub(super) fn norms(tokens: &[Operand]) -> Vec<&str> {
    tokens.iter().map(|t| t.norm.as_str()).collect()
}

/// Where the string or character literal opening at `s[i]` ends: just past
/// its closing quote (a backslash escapes the next byte), or the end of `s`.
fn quote_end(s: &[u8], i: usize) -> usize {
    let quote = s[i];
    let mut j = i + 1;
    while j < s.len() {
        match s[j] {
            b'\\' => j += 2,
            c if c == quote => return j + 1,
            _ => j += 1,
        }
    }
    s.len()
}

/// `line` in lowercase, but for its string and character literals, whose
/// case is part of their value.
pub(super) fn lowercase_outside_quotes(line: &str) -> String {
    let s = line.as_bytes();
    let (mut out, mut i) = (String::with_capacity(line.len()), 0);
    while i < s.len() {
        // Each step ends on an ASCII byte or the end: a character boundary.
        let end = match s[i] {
            b'"' | b'\'' => quote_end(s, i),
            _ => i + 1 + s[i + 1..].iter().take_while(|b| !b.is_ascii()).count(),
        };
        let part = &line[i..end];
        if s[i] == b'"' || s[i] == b'\'' {
            out.push_str(part);
        } else {
            out.extend(part.chars().map(|c| c.to_ascii_lowercase()));
        }
        i = end;
    }
    out
}

/// `line` up to its `;` comment, if any; a `;` inside quotes is not one.
pub(super) fn without_comment(line: &str) -> &str {
    let s = line.as_bytes();
    let mut i = 0;
    while i < s.len() {
        match s[i] {
            b'"' | b'\'' => i = quote_end(s, i),
            b';' => return &line[..i],
            _ => i += 1,
        }
    }
    line
}

/// The operands of `rest`, split at commas outside quotes and parentheses
/// (a function's arguments: `STRCMP("a", "b")`).
pub(super) fn operands(rest: &str) -> Vec<&str> {
    let s = rest.as_bytes();
    let (mut out, mut start, mut i, mut depth) = (Vec::new(), 0, 0, 0usize);
    while i < s.len() {
        match s[i] {
            b'"' | b'\'' => i = quote_end(s, i),
            b',' if depth == 0 => {
                out.push(&rest[start..i]);
                i += 1;
                start = i;
            }
            c => {
                depth = match c {
                    b'(' => depth + 1,
                    b')' => depth.saturating_sub(1),
                    _ => depth,
                };
                i += 1;
            }
        }
    }
    out.push(&rest[start..]);
    out
}

/// `query` with a comma where whitespace separates two operands
/// (`jr nz Top`), which rgbasm refuses: whitespace between two names or
/// numbers, with no operator, outside a string. `None` when there is none.
pub(super) fn missing_commas(query: &str) -> Option<String> {
    let (word, rest) = query.split_once(char::is_whitespace)?;
    let ends = |c: u8| c.is_ascii_alphanumeric() || b"_.#@'\")]".contains(&c);
    let starts = |c: u8| c.is_ascii_alphanumeric() || b"_.#@$'\"[`".contains(&c);
    let mut found = false;
    let operands: Vec<String> = operands(rest)
        .into_iter()
        .map(|operand| {
            let operand = operand.trim();
            let bytes = operand.as_bytes();
            let mut out = String::new();
            let (mut i, mut last) = (0, None);
            while i < bytes.len() {
                let c = bytes[i];
                if c.is_ascii_whitespace() {
                    let next = i + bytes[i..]
                        .iter()
                        .take_while(|b| b.is_ascii_whitespace())
                        .count();
                    let gap = last.is_some_and(ends) && bytes.get(next).is_some_and(|&n| starts(n));
                    found |= gap;
                    out.push_str(if gap { ", " } else { " " });
                    i = next;
                    continue;
                }
                // Each step ends on an ASCII byte or the end, so it slices
                // on a character boundary.
                let end = if c == b'"' || c == b'\'' {
                    quote_end(bytes, i)
                } else {
                    i + 1 + bytes[i + 1..].iter().take_while(|b| !b.is_ascii()).count()
                };
                out.push_str(&operand[i..end]);
                last = Some(c);
                i = end;
            }
            out
        })
        .collect();
    found.then(|| format!("{word} {}", operands.join(", ")))
}
