//! The legend's lines, `color` and `fill`, and looking a character up.

use super::{Def, Paint, Parser, VALUE_REMEDY, and_list, color, err, is_palette_name};
use crate::error::ParseError;
use crate::model::Value;
use crate::text::lex::Word;

impl Parser {
    pub(super) fn color(&mut self, line: u32, words: &[Word]) -> Result<(), ParseError> {
        if words.len() < 3 {
            return Err(err(
                line,
                words[0].column,
                "color takes a legend character and a value",
                VALUE_REMEDY,
            ));
        }
        let c = &words[1];
        let mut chars = c.text.chars();
        let symbol = match (chars.next(), chars.next()) {
            (Some(s), None) if s.is_ascii_graphic() && s != '#' => s,
            _ => {
                return Err(err(
                    line,
                    c.column,
                    format!("`{}` is not one legend character", c.text),
                    "a legend character is one printable ASCII character other than # and space",
                ));
            }
        };
        if let Some(&first) = self.by_symbol.get(&symbol) {
            return Err(err(
                line,
                c.column,
                format!(
                    "{symbol} is in the legend already (line {})",
                    self.defs[first].line
                ),
                "give each legend character one value; pick another character",
            ));
        }
        let value = &words[2..];
        let v = &value[0];
        let whole = match v.text {
            "clear" => Some(Paint::Value(Value::Clear)),
            "keep" => Some(Paint::Keep),
            t if t.starts_with('#') => match color(t) {
                Some(rgb) => Some(Paint::Value(Value::Color(rgb))),
                None => {
                    return Err(err(
                        line,
                        v.column,
                        format!("`{t}` is no #rrggbb or #rgb color"),
                        VALUE_REMEDY,
                    ));
                }
            },
            _ => None,
        };
        let (paint, used) = match (whole, value) {
            (Some(paint), _) => (paint, 1),
            (None, [one]) => {
                return Err(err(
                    line,
                    one.column,
                    format!("`{}` is not a value", one.text),
                    VALUE_REMEDY,
                ));
            }
            (None, [name, entry, ..]) => {
                if !is_palette_name(name.text) {
                    return Err(err(
                        line,
                        name.column,
                        format!(
                            "`{}` is no palette name, so `{} {}` is not a value",
                            name.text, name.text, entry.text
                        ),
                        VALUE_REMEDY,
                    ));
                }
                let index = match entry.text {
                    "0" | "1" | "2" | "3" => entry.text.as_bytes()[0] - b'0',
                    other => {
                        return Err(err(
                            line,
                            entry.column,
                            format!("`{other}` is no palette entry: entries are 0 to 3"),
                            VALUE_REMEDY,
                        ));
                    }
                };
                let palette = name.text.to_string();
                (Paint::Value(Value::Entry { palette, index }), 2)
            }
            (None, []) => unreachable!("three words at least"),
        };
        if let Some(extra) = value.get(used) {
            return Err(err(
                line,
                extra.column,
                format!(
                    "`{}` follows the value; color takes one character and one value",
                    extra.text
                ),
                "make the rest a comment with `# `",
            ));
        }
        self.by_symbol.insert(symbol, self.defs.len());
        self.defs.push(Def {
            symbol,
            paint,
            line,
            column: v.column,
        });
        Ok(())
    }

    /// The image's size, or the error for `word` coming before it.
    pub(super) fn sized(&self, line: u32, word: &Word) -> Result<(u32, u32), ParseError> {
        match &self.size {
            Some(s) => Ok((s.width, s.height)),
            None => Err(err(
                line,
                word.column,
                format!(
                    "{} comes before size, so the image has no size yet",
                    word.text
                ),
                "put `size W H` before the first fill, rows, tile or map",
            )),
        }
    }

    /// The def `symbol` names, or the error for a character not in the
    /// legend, at `column`.
    pub(super) fn def(&self, symbol: char, line: u32, column: u32) -> Result<usize, ParseError> {
        if symbol == '#' {
            return Err(err(
                line,
                column,
                "# isn't a legend character: it starts comments and colors",
                "draw with a legend character; # and space are never one",
            ));
        }
        self.by_symbol.get(&symbol).copied().ok_or_else(|| {
            let has: Vec<String> = self.defs.iter().map(|d| d.symbol.to_string()).collect();
            let has = match has.len() {
                0 => "it is empty".to_string(),
                _ => format!("it has {}", and_list(&has)),
            };
            err(
                line,
                column,
                format!("{symbol} isn't in the legend: {has}"),
                format!("add `color {symbol} VALUE`, or use a legend character"),
            )
        })
    }

    pub(super) fn fill(&mut self, line: u32, words: &[Word]) -> Result<(), ParseError> {
        self.sized(line, &words[0])?;
        let [_, c] = words else {
            return Err(err(
                line,
                words[0].column,
                "fill takes one legend character",
                "write `fill C` with C in the legend",
            ));
        };
        if let Some(first) = self.fill {
            return Err(err(
                line,
                words[0].column,
                format!("fill is given again (first on line {first})"),
                "fill once, before the blocks",
            ));
        }
        if self.blocks {
            return Err(err(
                line,
                words[0].column,
                "fill comes after a block, but it paints before every block",
                "move fill above the first rows, tile or map",
            ));
        }
        let mut chars = c.text.chars();
        let symbol = chars.next().expect("a word has a character");
        let def = self.def(symbol, line, c.column)?;
        if chars.next().is_some() {
            return Err(err(
                line,
                c.column,
                format!("`{}` is not one legend character", c.text),
                "write `fill C` with C in the legend",
            ));
        }
        if matches!(self.defs[def].paint, Paint::Keep) {
            return Err(err(
                line,
                c.column,
                "fill can't keep: it paints first, so there is nothing to keep",
                "fill with a color or clear",
            ));
        }
        self.fill = Some(line);
        self.canvas.fill(Some((def, line)));
        Ok(())
    }
}
