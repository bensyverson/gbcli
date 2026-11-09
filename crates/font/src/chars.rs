//! The `.chars` sidecar of a font's PNG working copy: one line per row of
//! cells, each character the key of the cell under it, left to right.
//! Besides a character as itself, a line writes `<name>` for a named
//! glyph, `<u+XXXX>` for any character by code point (the writer uses it
//! for a space, a combining mark and an ambiguous `<`), and `<>` for a
//! cell left empty. A line shorter than the row leaves the rest empty.
//!
//! A character list (`--chars FILE` on `import` and `new`) is the same
//! text with its line breaks ignored.

use crate::key::{Key, said};
use crate::marks;
use crate::problem::Problem;

/// A `.chars` file's extension.
pub const CHARS_EXTENSION: &str = "chars";
/// Cells in a row of a font's grid: GB Studio's 16, a 128-pixel image.
pub const GRID_COLUMNS: usize = 16;

/// A cell of the grid: the key of the glyph drawn in it, or none.
pub type Slot = Option<Key>;

const REMEDY: &str = "write a character as itself, a named glyph as <name>, any character as \
                      <u+XXXX>, and an empty cell as <>; `gbcli help fonts` has the rules";

/// The rows of slots `text` lists, or every line and column at fault.
pub fn read_chars(text: &str) -> Result<Vec<Vec<Slot>>, Vec<Problem>> {
    let text = text.strip_prefix('\u{FEFF}').unwrap_or(text);
    let mut rows = Vec::new();
    let mut problems = Vec::new();
    for (n, line) in text.lines().enumerate() {
        let line = line.strip_suffix('\r').unwrap_or(line);
        let mut row = Vec::new();
        let mut rest = line;
        let mut column = 1;
        while let Some(c) = rest.chars().next() {
            let (slot, len) = match escape(rest) {
                Some(Ok((slot, len))) => (slot, len),
                Some(Err(e)) => {
                    problems.push(Problem::new(e, REMEDY).at_line(n + 1, Some(column)));
                    let len = rest.find('>').map_or(rest.len(), |i| i + 1);
                    (None, len)
                }
                None => match Key::from_wire(&c.to_string()) {
                    Ok(key) => (Some(key), c.len_utf8()),
                    Err(e) => {
                        problems
                            .push(Problem::new(e.message, e.remedy).at_line(n + 1, Some(column)));
                        (None, c.len_utf8())
                    }
                },
            };
            column += rest[..len].chars().count();
            rest = &rest[len..];
            row.push(slot);
        }
        rows.push(row);
    }
    if problems.is_empty() {
        Ok(rows)
    } else {
        Err(problems)
    }
}

/// The escape `rest` starts with, and its length in bytes: `None` when it
/// starts with none (a `<` that opens no escape is itself).
fn escape(rest: &str) -> Option<Result<(Slot, usize), String>> {
    let after = rest.strip_prefix('<')?;
    if after.starts_with('>') {
        return Some(Ok((None, 2)));
    }
    let close = after.find('>')?;
    let inner = &after[..close];
    let len = close + 2;
    if let Some(hex) = inner
        .strip_prefix("u+")
        .or_else(|| inner.strip_prefix("U+"))
    {
        let c = u32::from_str_radix(hex, 16)
            .ok()
            .filter(|_| (1..=6).contains(&hex.len()))
            .and_then(char::from_u32);
        return Some(match c {
            Some(c) => Key::from_wire(&c.to_string())
                .map(|k| (Some(k), len))
                .map_err(|e| e.message),
            None => Err(format!("<{inner}> isn't a Unicode character")),
        });
    }
    let named = inner.len() >= 2
        && inner.starts_with(|c: char| c.is_ascii_alphabetic())
        && inner
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-');
    named.then(|| Ok((Some(Key::Name(inner.to_string())), len)))
}

/// `rows` as `.chars` text, which [`read_chars`] reads back as `rows`
/// (with trailing empty cells dropped).
pub fn write_chars(rows: &[Vec<Slot>]) -> String {
    let mut out = String::new();
    for row in rows {
        let last = row.iter().rposition(Option::is_some).map_or(0, |i| i + 1);
        for (i, slot) in row[..last].iter().enumerate() {
            match slot {
                None => out.push_str("<>"),
                Some(Key::Name(n)) => {
                    out.push('<');
                    out.push_str(n);
                    out.push('>');
                }
                Some(Key::Char(c)) => {
                    let next = row[..last].get(i + 1);
                    let ambiguous = *c == '<'
                        && !matches!(next, Some(Some(Key::Char(n))) if !(n.is_ascii_alphabetic() || *n == '>'))
                        && next.is_some();
                    if c.is_whitespace() || marks::is_combining(*c) || ambiguous {
                        out.push_str(&format!("<u+{:04X}>", u32::from(*c)));
                    } else {
                        out.push(*c);
                    }
                }
            }
        }
        out.push('\n');
    }
    out
}

/// The keys a character list names, in order, each once; a cell left
/// empty (`<>`) names none.
pub fn read_list(text: &str) -> Result<Vec<Key>, Vec<Problem>> {
    let mut keys: Vec<Key> = Vec::new();
    for key in read_chars(text)?.into_iter().flatten().flatten() {
        if !keys.contains(&key) {
            keys.push(key);
        }
    }
    Ok(keys)
}

/// A key as a problem names it in a `.chars` file.
pub(crate) fn named(key: &Key) -> String {
    match key {
        Key::Char(c) => said(*c),
        Key::Name(n) => format!("<{n}>"),
    }
}

#[cfg(test)]
mod tests;
