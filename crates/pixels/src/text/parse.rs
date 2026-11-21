//! Pixel text into a [`Picture`]: each line read in order, each block
//! painted as it is read, every problem located to its line and column.

use std::collections::HashMap;

use super::FORMAT_LINE;
use super::lex::{Line, Word, words};
use crate::error::ParseError;
use crate::model::{Picture, Symbol, Value};

mod blocks;
mod legend;

/// The largest side of an image written as pixel text, in pixels: four
/// times a background map's.
pub const MAX_SIDE: u32 = 1024;

/// The largest side pixel text may have when read: twice [`MAX_SIDE`],
/// since padding a sheet's 8-pixel frames to 8x16 objects doubles its
/// height, so the padded text of a PNG of any size gbcli writes reads.
pub const MAX_TEXT_SIDE: u32 = 2 * MAX_SIDE;

/// The words that start a line.
const DIRECTIVES: &[&str] = &["format", "size", "color", "fill", "rows", "tile", "map"];

const VALUE_REMEDY: &str = "write `color C VALUE`: VALUE is #rrggbb, #rgb, clear, keep, or a \
                            palette and entry (`dmg 2`, `forest 1`)";
const SIZE_REMEDY: &str = "write `size W H`, e.g. `size 16 16`";
const AT_REMEDY: &str = "write `rows X Y` or `map X Y` (pixels from the top left), or leave the \
                         position out for 0 0";
const KEY_REMEDY: &str = "write `tile KEY`: KEY is letters, digits and _";
const TILE_REMEDY: &str = "a tile is exactly 8 rows of 8, then a blank line";
const SAME_WIDTH: &str = "make every row in a block the same width";

fn err(
    line: u32,
    column: u32,
    message: impl Into<String>,
    remedy: impl Into<String>,
) -> ParseError {
    ParseError {
        line,
        column,
        message: message.into(),
        remedy: remedy.into(),
    }
}

/// What a legend character paints.
#[derive(Clone, Debug)]
enum Paint {
    Value(Value),
    /// Leave the pixel as earlier blocks drew it.
    Keep,
}

#[derive(Debug)]
struct Def {
    symbol: char,
    paint: Paint,
    line: u32,
    /// Where the value starts.
    column: u32,
}

/// The block being read.
#[derive(Debug)]
enum Open {
    Rows {
        x: u32,
        y: u32,
    },
    Tile {
        key: String,
        column: u32,
        pixels: Vec<usize>,
    },
    Map {
        x: u32,
        y: u32,
    },
}

#[derive(Debug)]
struct Block {
    open: Open,
    line: u32,
    /// Where the header word starts.
    column: u32,
    rows: u32,
    /// The first row's width: pixels, or a map's tiles.
    width: Option<usize>,
}

struct Size {
    width: u32,
    height: u32,
    line: u32,
}

#[derive(Default)]
struct Parser {
    format: Option<u32>,
    size: Option<Size>,
    defs: Vec<Def>,
    by_symbol: HashMap<char, usize>,
    fill: Option<u32>,
    blocks: bool,
    tiles: Vec<(String, Vec<usize>, u32)>,
    by_key: HashMap<String, usize>,
    /// Each pixel's def, once painted, and the line that painted it.
    canvas: Vec<Option<(usize, u32)>>,
    block: Option<Block>,
}

/// Parses pixel text into a picture: every block painted, in file order.
pub fn parse_text(text: &str) -> Result<Picture, ParseError> {
    let text = text.strip_prefix('\u{FEFF}').unwrap_or(text);
    let mut p = Parser::default();
    for (raw, line) in text.split('\n').zip(1u32..) {
        let raw = raw.strip_suffix('\r').unwrap_or(raw);
        match words(raw) {
            Line::Blank => p.close()?,
            Line::Comment => {}
            Line::Words(words) => p.line(line, &words)?,
        }
    }
    p.close()?;
    p.finish()
}

fn is_directive(word: &str) -> bool {
    DIRECTIVES.contains(&word)
}

/// `a`, `a and b`, `a, b and c`.
fn and_list(items: &[String]) -> String {
    match items {
        [] => String::new(),
        [one] => one.clone(),
        [rest @ .., last] => format!("{} and {last}", rest.join(", ")),
    }
}

/// A lowercase letter, then lowercase letters, digits and `_`: a palette
/// name `palettes.txt` accepts, or `dmg`.
fn is_palette_name(word: &str) -> bool {
    let mut chars = word.chars();
    chars.next().is_some_and(|c| c.is_ascii_lowercase())
        && chars.all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '_')
}

fn is_tile_key(word: &str) -> bool {
    !word.is_empty() && word.chars().all(|c| c.is_ascii_alphanumeric() || c == '_')
}

/// `#rrggbb` or `#rgb` (either case), at 8 bits a channel.
fn color(word: &str) -> Option<[u8; 3]> {
    let digits = word.strip_prefix('#')?;
    if !digits.bytes().all(|b| b.is_ascii_hexdigit()) {
        return None;
    }
    let v = u32::from_str_radix(digits, 16).ok()?;
    match digits.len() {
        6 => Some([(v >> 16) as u8, (v >> 8) as u8, v as u8]),
        // CSS: each digit doubled, #8c4 = #88cc44.
        3 => Some([(v >> 8) & 0xF, (v >> 4) & 0xF, v & 0xF].map(|d| (d * 0x11) as u8)),
        _ => None,
    }
}

/// A decimal number, or none.
fn number(word: &str) -> Option<u32> {
    if word.is_empty() || !word.bytes().all(|b| b.is_ascii_digit()) {
        return None;
    }
    // Too many digits for a u32 is past every limit anyway.
    Some(word.parse().unwrap_or(u32::MAX))
}

impl Parser {
    fn line(&mut self, line: u32, words: &[Word]) -> Result<(), ParseError> {
        if self.block.is_some() {
            return self.row(line, words);
        }
        let first = &words[0];
        if self.format.is_none() {
            return self.format(line, words);
        }
        match first.text {
            "format" => Err(err(
                line,
                first.column,
                format!(
                    "the format line is given again (first on line {})",
                    self.format.unwrap_or(1)
                ),
                "give the format line once, first",
            )),
            "size" => self.size(line, words),
            "color" => self.color(line, words),
            "fill" => self.fill(line, words),
            "rows" | "map" | "tile" => self.open(line, words),
            other => Err(err(
                line,
                first.column,
                format!(
                    "`{other}` doesn't start a line: a line starts with size, color, fill, \
                     rows, tile or map"
                ),
                "start the line with one of those, or make it a comment with `# `",
            )),
        }
    }

    fn format(&mut self, line: u32, words: &[Word]) -> Result<(), ParseError> {
        let joined: Vec<&str> = words.iter().map(|w| w.text).collect();
        if joined.join(" ") != FORMAT_LINE {
            let message = if words[0].text == "format" {
                format!(
                    "`{}` is not a format this gbcli reads",
                    joined[1..].join(" ")
                )
            } else {
                "pixel text starts with its format line".to_string()
            };
            return Err(err(
                line,
                words[0].column,
                message,
                format!("make the first line `{FORMAT_LINE}`"),
            ));
        }
        self.format = Some(line);
        Ok(())
    }

    fn size(&mut self, line: u32, words: &[Word]) -> Result<(), ParseError> {
        if let Some(first) = &self.size {
            return Err(err(
                line,
                words[0].column,
                format!("size is given again (first on line {})", first.line),
                "give the size once",
            ));
        }
        let [_, w, h] = words else {
            return Err(err(
                line,
                words[0].column,
                "size takes a width and a height in pixels",
                SIZE_REMEDY,
            ));
        };
        let side = |word: &Word| match number(word.text) {
            None => Err(err(
                line,
                word.column,
                format!("`{}` is not a number of pixels", word.text),
                SIZE_REMEDY,
            )),
            Some(n) if n == 0 || n > MAX_TEXT_SIDE => Err(err(
                line,
                word.column,
                format!("an image is 1 to {MAX_TEXT_SIDE} pixels a side"),
                SIZE_REMEDY,
            )),
            Some(n) => Ok(n),
        };
        let (width, height) = (side(w)?, side(h)?);
        self.size = Some(Size {
            width,
            height,
            line,
        });
        self.canvas = vec![None; (width * height) as usize];
        Ok(())
    }

    fn finish(self) -> Result<Picture, ParseError> {
        let Some(format) = self.format else {
            return Err(err(
                1,
                1,
                "pixel text starts with its format line",
                format!("make the first line `{FORMAT_LINE}`"),
            ));
        };
        let Some(size) = self.size else {
            return Err(err(
                format,
                1,
                "the file has no size line",
                "add `size W H` after the format line",
            ));
        };
        // Keep is only a way to paint: the picture's legend is the rest.
        let mut legend = Vec::new();
        let mut index = vec![0u16; self.defs.len()];
        for (i, d) in self.defs.iter().enumerate() {
            if let Paint::Value(value) = &d.paint {
                index[i] = legend.len() as u16;
                legend.push(Symbol {
                    symbol: d.symbol,
                    value: value.clone(),
                    at: Some((d.line, d.column)),
                });
            }
        }
        let mut pixels = Vec::with_capacity(self.canvas.len());
        let mut lines = Vec::with_capacity(self.canvas.len());
        for (i, painted) in self.canvas.iter().enumerate() {
            let Some((def, line)) = painted else {
                let (x, y) = (i as u32 % size.width, i as u32 / size.width);
                return Err(err(
                    size.line,
                    1,
                    format!("pixel {x},{y} is never painted: no fill or block covers it"),
                    "draw it in a rows or map block, or `fill C` first",
                ));
            };
            pixels.push(index[*def]);
            lines.push(*line);
        }
        Ok(Picture {
            width: size.width,
            height: size.height,
            legend,
            pixels,
            lines,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::DMG;

    #[test]
    fn dmg_is_a_palette_name_here_and_names_follow_palettes_txt() {
        assert!(is_palette_name(DMG));
        assert!(is_palette_name("forest_2"));
        assert!(!is_palette_name("Forest"));
        assert!(!is_palette_name("2x"));
    }
}
