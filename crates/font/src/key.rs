//! A glyph's key: one character, or a name for a symbol Unicode lacks. In
//! a `.yaff` file it is a label, `'é':`, `u+00E9:` or `"heart":`; on the
//! wire, the character itself or the name; in a charmap, the character or
//! `<name>`, so `db "<heart>"` assembles to the named glyph.

use std::fmt;

use gbcli_protocol::marks;

/// A glyph's key.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Key {
    /// One Unicode scalar value, in NFC.
    Char(char),
    /// Two or more ASCII letters, digits, `_` and `-`, starting with a
    /// letter: one character is always a [`Key::Char`].
    Name(String),
}

/// Why a label or a wire key isn't a key gbcli takes.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct KeyError {
    pub message: String,
    pub remedy: String,
}

fn bad(message: impl Into<String>, remedy: impl Into<String>) -> KeyError {
    KeyError {
        message: message.into(),
        remedy: remedy.into(),
    }
}

/// `'é' (u+00E9)`, how a message names a character.
pub(crate) fn said(c: char) -> String {
    if c.is_control() || c.is_whitespace() || is_combining(c) {
        format!("u+{:04X}", u32::from(c))
    } else {
        format!("'{c}' (u+{:04X})", u32::from(c))
    }
}

/// Whether `c` draws over the character before it, so a message, a label
/// or a sidecar writes its code point rather than the character: the mark
/// table's class, and the other blocks of combining marks.
pub(crate) fn is_combining(c: char) -> bool {
    marks::is_combining_mark(c)
        || matches!(
            c,
            '\u{1AB0}'..='\u{1AFF}'
                | '\u{1DC0}'..='\u{1DFF}'
                | '\u{20D0}'..='\u{20FF}'
                | '\u{FE20}'..='\u{FE2F}'
        )
}

const KEY_REMEDY: &str = "key a glyph by one character ('A':), a code point (u+0041:) or a \
                          name (\"heart\":); `gbcli help fonts` has the rules";

impl Key {
    /// The key a YAFF label (the text before its `:`) gives, under gbcli's
    /// rules: one character or code point, or a quoted name.
    pub fn from_label(label: &str) -> Result<Key, KeyError> {
        let label = label.trim();
        if label.is_empty() {
            return Err(bad(
                "this glyph has no key",
                "give it a label before its rows, such as 'A':",
            ));
        }
        if label.starts_with(|c: char| c.is_ascii_digit()) {
            return Err(bad(
                format!(
                    "`{label}` is a codepoint label, a number in some encoding; a gbcli font is \
                     keyed by character"
                ),
                "write the character's Unicode code point (u+0041:) or the character ('A':)",
            ));
        }
        if let Some(name) = label.strip_prefix('"') {
            let Some(name) = name.strip_suffix('"').filter(|_| label.len() >= 2) else {
                return Err(bad(
                    format!("`{label}` opens a name with `\"` but doesn't close it"),
                    KEY_REMEDY,
                ));
            };
            return Key::name(name);
        }
        let chars = elements(label)?;
        match chars.as_slice() {
            [c] => Key::char(*c),
            // NFC maps a singleton before composing (U+212A is K), so
            // KELVIN SIGN and a cedilla is Ķ, decomposed.
            [base, mark] => match marks::compose(canonical(*base), canonical(*mark)) {
                Some(letter) => Err(bad(
                    format!(
                        "`{label}` is {} followed by the combining mark u+{:04X}: a decomposed \
                         key, which gbcli refuses (keys are NFC)",
                        said(*base),
                        u32::from(*mark)
                    ),
                    format!(
                        "key it by its precomposed form, '{letter}': (u+{:04X})",
                        u32::from(letter)
                    ),
                )),
                None => Err(cluster(label)),
            },
            _ => Err(cluster(label)),
        }
    }

    /// The key a wire string names: one character, or a name.
    pub fn from_wire(text: &str) -> Result<Key, KeyError> {
        let mut chars = text.chars();
        match (chars.next(), chars.next()) {
            (None, _) => Err(bad(
                "an empty key names no glyph",
                "name a glyph by its character or its name",
            )),
            (Some(c), None) => Key::char(c),
            _ => Key::name(text),
        }
    }

    fn char(c: char) -> Result<Key, KeyError> {
        if c.is_control() {
            return Err(bad(
                format!(
                    "{} is a control character, which text doesn't draw",
                    said(c)
                ),
                "key the glyph by a character it draws; a font's charmap reserves $FE for a \
                 newline and $FF for the end of a string",
            ));
        }
        if let Some(canonical) = marks::singleton(c) {
            // A singleton onto a mark (U+0341 is U+0301) names a key a quote
            // would hide.
            let (said_as, key_by) = if quotable(canonical) {
                (
                    format!("'{canonical}' (u+{:04X})", u32::from(canonical)),
                    format!("'{canonical}': (u+{:04X})", u32::from(canonical)),
                )
            } else {
                let code = format!("u+{:04X}", u32::from(canonical));
                (code.clone(), format!("{code}:"))
            };
            return Err(bad(
                format!(
                    "u+{:04X} is not NFC: it is canonically {said_as}",
                    u32::from(c)
                ),
                format!("key it by {key_by}"),
            ));
        }
        Ok(Key::Char(c))
    }

    fn name(name: &str) -> Result<Key, KeyError> {
        let mut chars = name.chars();
        let first = chars.next();
        let ok = first.is_some_and(|c| c.is_ascii_alphabetic())
            && chars.all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-');
        if !ok {
            return Err(bad(
                format!(
                    "`{name}` isn't a glyph name: a name starts with an ASCII letter and holds \
                     only letters, digits, `_` and `-`"
                ),
                "rename it (\"btn_a\":), or key the glyph by its character",
            ));
        }
        if name.len() < 2 {
            return Err(bad(
                format!("\"{name}\" is a name of one character, which is that character's key"),
                format!("write it as '{name}':"),
            ));
        }
        Ok(Key::Name(name.to_string()))
    }

    /// The YAFF label gbcli writes for it: `'A'`, or `u+0020` for a
    /// character a quote would hide (space, a mark, `'`), or `"heart"`.
    pub fn label(&self) -> String {
        match self {
            Key::Char(c) if quotable(*c) => format!("'{c}'"),
            Key::Char(c) => format!("u+{:04X}", u32::from(*c)),
            Key::Name(n) => format!("\"{n}\""),
        }
    }

    /// The key on the wire: the character, or the name.
    pub fn text(&self) -> String {
        match self {
            Key::Char(c) => c.to_string(),
            Key::Name(n) => n.clone(),
        }
    }

    /// What a string writes for it, under the font's charmap: the
    /// character, or `<name>`.
    pub fn charmap(&self) -> String {
        match self {
            Key::Char(c) => c.to_string(),
            Key::Name(n) => format!("<{n}>"),
        }
    }
}

impl fmt::Display for Key {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Key::Char(c) => f.write_str(&said(*c)),
            Key::Name(n) => write!(f, "\"{n}\""),
        }
    }
}

/// Whether `'c'` reads as `c`: not a space, a mark, a quote or invisible.
fn quotable(c: char) -> bool {
    !(c.is_control()
        || c.is_whitespace()
        || c == '\''
        || is_combining(c)
        || matches!(c, '\u{200B}'..='\u{200F}' | '\u{2028}'..='\u{202E}' | '\u{FEFF}'))
}

fn cluster(label: &str) -> KeyError {
    bad(
        format!(
            "`{label}` is several characters; a gbcli glyph is keyed by one character (or a \
             name)"
        ),
        "key it by one character, or by a name (\"ff_ligature\":)",
    )
}

/// The characters a character label spells: `'…'` and `u+XXXX` elements,
/// separated by commas.
fn elements(label: &str) -> Result<Vec<char>, KeyError> {
    let mut out = Vec::new();
    let mut rest = label;
    loop {
        rest = rest.trim_start();
        if let Some(after) = rest.strip_prefix('\'') {
            // The element runs to the last quote before the next comma
            // after a quote, so `','` is a comma.
            let close = after
                .char_indices()
                .filter(|(_, c)| *c == '\'')
                .map(|(i, _)| i)
                .find(|&i| {
                    let tail = after[i + 1..].trim_start();
                    tail.is_empty() || tail.starts_with(',')
                })
                .ok_or_else(|| {
                    bad(
                        format!("`{label}` opens a character with `'` but doesn't close it"),
                        KEY_REMEDY,
                    )
                })?;
            out.extend(after[..close].chars());
            rest = &after[close + 1..];
        } else if let Some(hex) = rest.strip_prefix("u+").or_else(|| rest.strip_prefix("U+")) {
            let end = hex.find(',').unwrap_or(hex.len());
            let digits = hex[..end].trim();
            let c = u32::from_str_radix(digits, 16)
                .ok()
                .filter(|_| !digits.is_empty() && digits.len() <= 6)
                .and_then(char::from_u32)
                .ok_or_else(|| {
                    bad(
                        format!("`{label}`: u+{digits} isn't a Unicode character"),
                        "write a code point as u+ and 4 to 6 hex digits, such as u+00E9",
                    )
                })?;
            out.push(c);
            rest = &hex[end..];
        } else {
            return Err(bad(
                format!(
                    "`{label}` is an unquoted label, which YAFF 1.0 deprecates and gbcli refuses"
                ),
                "quote it: 'A': for a character, \"space\": for a name",
            ));
        }
        rest = rest.trim_start();
        match rest.strip_prefix(',') {
            Some(more) => rest = more,
            None if rest.is_empty() => break,
            None => {
                return Err(bad(
                    format!("`{label}`: `{rest}` follows a character"),
                    KEY_REMEDY,
                ));
            }
        }
    }
    if out.is_empty() {
        return Err(bad(format!("`{label}` names no character"), KEY_REMEDY));
    }
    Ok(out)
}

/// What `c` is canonically by itself, if it is a singleton the mark table
/// maps, or `c`.
fn canonical(c: char) -> char {
    marks::singleton(c).unwrap_or(c)
}

#[cfg(test)]
mod tests;
