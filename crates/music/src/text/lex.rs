//! A line into tokens: words, quoted strings and the `|` between channels,
//! each with the column it starts at.

use crate::error::ParseError;

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Kind {
    Word,
    /// A quoted string, its escapes decoded.
    Str(Vec<u8>),
    Bar,
}

#[derive(Clone, Debug)]
pub struct Token {
    pub kind: Kind,
    /// The source text (for a string, as written, quotes included).
    pub text: String,
    /// 1-based, in characters.
    pub column: u32,
}

pub fn tokens(line: &str, line_no: u32) -> Result<Vec<Token>, ParseError> {
    let chars: Vec<char> = line.chars().collect();
    let mut out = Vec::new();
    let mut i = 0;
    while i < chars.len() {
        let c = chars[i];
        let column = i as u32 + 1;
        if c.is_whitespace() {
            i += 1;
        } else if c == '|' {
            out.push(Token {
                kind: Kind::Bar,
                text: "|".into(),
                column,
            });
            i += 1;
        } else if c == '"' {
            let (bytes, end) = string(&chars, i, line_no)?;
            out.push(Token {
                kind: Kind::Str(bytes),
                text: chars[i..end].iter().collect(),
                column,
            });
            i = end;
        } else {
            let start = i;
            while i < chars.len() && !chars[i].is_whitespace() && chars[i] != '|' && chars[i] != '"'
            {
                i += 1;
            }
            out.push(Token {
                kind: Kind::Word,
                text: chars[start..i].iter().collect(),
                column,
            });
        }
    }
    Ok(out)
}

/// The string starting at `chars[start]` (a quote): its bytes and the index
/// past its closing quote.
fn string(chars: &[char], start: usize, line_no: u32) -> Result<(Vec<u8>, usize), ParseError> {
    let err = |i: usize, message: String| ParseError {
        line: line_no,
        column: i as u32 + 1,
        message,
        remedy: "close the string with \"; inside one, write \\\" for a quote, \\\\ for a \
                 backslash, \\n, \\r, \\t, or \\xNN for any byte"
            .into(),
    };
    let mut bytes = Vec::new();
    let mut i = start + 1;
    loop {
        let Some(&c) = chars.get(i) else {
            return Err(err(start, "this string has no closing quote".into()));
        };
        i += 1;
        match c {
            '"' => return Ok((bytes, i)),
            '\\' => {
                let Some(&e) = chars.get(i) else {
                    return Err(err(start, "this string has no closing quote".into()));
                };
                i += 1;
                match e {
                    '"' => bytes.push(b'"'),
                    '\\' => bytes.push(b'\\'),
                    'n' => bytes.push(b'\n'),
                    'r' => bytes.push(b'\r'),
                    't' => bytes.push(b'\t'),
                    'x' => {
                        let hex: String = chars.get(i..i + 2).unwrap_or(&[]).iter().collect();
                        let Ok(b) = u8::from_str_radix(&hex, 16) else {
                            return Err(err(i - 2, "\\x needs two hex digits".into()));
                        };
                        bytes.push(b);
                        i += 2;
                    }
                    other => return Err(err(i - 2, format!("\\{other} is not an escape"))),
                }
            }
            c => {
                let mut buf = [0; 4];
                bytes.extend(c.encode_utf8(&mut buf).bytes());
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn words_strings_and_bars_carry_their_columns() {
        let t = tokens("00 C-5 01|  \"a \\\"b\\\"\" x", 1).unwrap();
        let texts: Vec<(&str, u32)> = t.iter().map(|t| (t.text.as_str(), t.column)).collect();
        assert_eq!(
            texts,
            [
                ("00", 1),
                ("C-5", 4),
                ("01", 8),
                ("|", 10),
                ("\"a \\\"b\\\"\"", 13),
                ("x", 23)
            ]
        );
        assert_eq!(t[4].kind, Kind::Str(b"a \"b\"".to_vec()));
    }
}
