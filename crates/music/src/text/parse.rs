//! Song text → [`Song`]. Line by line: a line's first word says what it is;
//! `instrument`, `routine`, `orders` and `pattern` open a block that runs
//! until the next such word. Blank lines and lines starting with `#` are
//! skipped (outside a routine's `|` lines).

use std::collections::BTreeMap;

use super::FORMAT_LINE;
use super::grid::{Blocks, Orders};
use super::instrument::Block;
use super::lex::{Token, tokens};
use super::spell::read_bang;
use super::values::{args, at, int, number, string};
use crate::error::ParseError;
use crate::model::{INSTRUMENTS, ROUTINES, Song, WAVE_LEN, WAVES};

/// The words that start a top-level line.
const DIRECTIVES: &[&str] = &[
    "format",
    "name",
    "artist",
    "comment",
    "ticks_per_row",
    "timer",
    "wave",
    "instrument",
    "routine",
    "orders",
    "storage",
    "pattern",
];

enum Section {
    Top,
    Instrument(Box<Block>),
    Routine(Routine),
    Orders,
    Pattern,
}

struct Routine {
    index: usize,
    crlf: bool,
    lines: Vec<String>,
}

pub fn parse_text(text: &str) -> Result<Song, ParseError> {
    let mut p = Parser {
        song: Song::default(),
        section: Section::Top,
        format_seen: false,
        seen: BTreeMap::new(),
        instruments: [None; INSTRUMENTS],
        orders: Orders::default(),
        blocks: Blocks::default(),
        last_line: 0,
    };
    for (i, raw) in text.lines().enumerate() {
        let line = i as u32 + 1;
        p.last_line = line;
        p.line(line, raw)?;
    }
    p.finish()
}

struct Parser {
    song: Song,
    section: Section,
    format_seen: bool,
    /// Header words and indexed lines given so far (`name`, `wave 3`), by
    /// the line that gave them.
    seen: BTreeMap<String, u32>,
    instruments: [Option<u32>; INSTRUMENTS],
    orders: Orders,
    blocks: Blocks,
    last_line: u32,
}

impl Parser {
    fn line(&mut self, line: u32, raw: &str) -> Result<(), ParseError> {
        let trimmed = raw.trim_start();
        if let Section::Routine(r) = &mut self.section {
            if let Some(body) = trimmed.strip_prefix('|') {
                r.lines
                    .push(body.strip_prefix(' ').unwrap_or(body).to_string());
                return Ok(());
            }
            if trimmed.trim_end() == "end" {
                self.close_routine();
                return Ok(());
            }
            if trimmed.is_empty() || trimmed.starts_with('#') {
                return Ok(());
            }
            return Err(ParseError {
                line,
                column: (raw.len() - trimmed.len()) as u32 + 1,
                message: format!("routine {} needs `end` before this line", r.index),
                remedy: "every routine line starts with |, and the routine ends with a line \
                         reading end"
                    .into(),
            });
        }
        if trimmed.is_empty() || trimmed.starts_with('#') {
            return Ok(());
        }
        let tokens = tokens(raw, line)?;
        let word = tokens[0].text.as_str();
        if !self.format_seen {
            return self.format(line, &tokens);
        }
        if DIRECTIVES.contains(&word) {
            self.close()?;
            return self.directive(line, &tokens);
        }
        match &mut self.section {
            Section::Instrument(block) => block.line(line, &tokens),
            Section::Orders => self.orders.line(line, &tokens),
            Section::Pattern => self.blocks.row(line, &tokens),
            Section::Top | Section::Routine(_) => Err(self.unknown(line, &tokens[0])),
        }
    }

    fn unknown(&self, line: u32, token: &Token) -> ParseError {
        let what = if token.text.bytes().all(|b| b.is_ascii_digit()) {
            "a row outside a pattern block, an instrument or the orders".to_string()
        } else {
            format!("`{}` is not a song line", token.text)
        };
        at(
            line,
            token,
            what,
            format!(
                "a line starts with one of: {} (see `gbcli help music`)",
                DIRECTIVES[1..].join(", ")
            ),
        )
    }

    fn format(&mut self, line: u32, tokens: &[Token]) -> Result<(), ParseError> {
        let words: Vec<&str> = tokens.iter().map(|t| t.text.as_str()).collect();
        if words.join(" ") != FORMAT_LINE {
            let message = if words[0] == "format" {
                format!(
                    "`{}` is not a format this gbcli reads",
                    words[1..].join(" ")
                )
            } else {
                "a song text starts with its format line".to_string()
            };
            return Err(at(
                line,
                &tokens[0],
                message,
                format!("make the first line `{FORMAT_LINE}`"),
            ));
        }
        self.format_seen = true;
        Ok(())
    }

    /// Refuses a second `name`, `wave 3`, ... naming where the first was.
    fn once(&mut self, key: String, line: u32, token: &Token) -> Result<(), ParseError> {
        if let Some(first) = self.seen.insert(key.clone(), line) {
            return Err(at(
                line,
                token,
                format!("`{key}` is given twice (first at line {first})"),
                "keep one",
            ));
        }
        Ok(())
    }

    fn directive(&mut self, line: u32, tokens: &[Token]) -> Result<(), ParseError> {
        let word = tokens[0].text.as_str();
        match word {
            "format" => Err(at(
                line,
                &tokens[0],
                "a second format line",
                "keep the first",
            )),
            "name" | "artist" | "comment" => {
                self.once(word.into(), line, &tokens[0])?;
                let a = args(line, tokens, 1, &format!("{word} \"TEXT\""))?;
                let s = string(line, &a[0], &format!("the {word}"))?;
                match word {
                    "name" => self.song.name = s,
                    "artist" => self.song.artist = s,
                    _ => self.song.comment = s,
                }
                Ok(())
            }
            "ticks_per_row" => {
                self.once(word.into(), line, &tokens[0])?;
                let a = args(line, tokens, 1, "ticks_per_row N (1-255)")?;
                let n = number(line, &a[0], 1, 255, "the ticks per row")?;
                self.song.ticks_per_row = int(line, &a[0], n, "the ticks per row")?;
                Ok(())
            }
            "timer" => {
                self.once(word.into(), line, &tokens[0])?;
                self.timer(line, tokens)
            }
            "wave" => self.wave(line, tokens),
            "instrument" => {
                let block = Block::start(line, tokens)?;
                if let Some(first) = self.instruments[block.index].replace(line) {
                    return Err(at(
                        line,
                        &tokens[0],
                        format!("this instrument is given twice (first at line {first})"),
                        "keep one",
                    ));
                }
                self.section = Section::Instrument(Box::new(block));
                Ok(())
            }
            "routine" => self.routine(line, tokens),
            "orders" => {
                self.once(word.into(), line, &tokens[0])?;
                args(
                    line,
                    tokens,
                    0,
                    "orders, then one line per order: NN P0 P1 P2 P3",
                )?;
                self.section = Section::Orders;
                Ok(())
            }
            "storage" => {
                self.once(word.into(), line, &tokens[0])?;
                self.blocks.storage(line, tokens)
            }
            _ => {
                self.blocks.start(line, tokens)?;
                self.section = Section::Pattern;
                Ok(())
            }
        }
    }

    fn timer(&mut self, line: u32, tokens: &[Token]) -> Result<(), ParseError> {
        let shape = "timer off, timer on DIVIDER, or timer off DIVIDER (0-255)";
        let a = &tokens[1..];
        let divider = |t: &Token| number(line, t, 0, 255, "the timer divider");
        let (enabled, divider) = match a.iter().map(|t| t.text.as_str()).collect::<Vec<_>>()[..] {
            ["off"] => (0, 0),
            ["off", _] => (0, divider(&a[1])?),
            ["on", _] => (1, divider(&a[1])?),
            [e, _] if read_bang(e).is_some() => (read_bang(e).unwrap_or(0), divider(&a[1])?),
            _ => {
                return Err(at(
                    line,
                    &tokens[0],
                    "this timer line has the wrong shape",
                    shape,
                ));
            }
        };
        let last = &tokens[tokens.len() - 1];
        self.song.timer_enabled = super::values::byte(line, last, enabled, "the timer switch")?;
        self.song.timer_divider = int(line, last, divider, "the timer divider")?;
        Ok(())
    }

    fn wave(&mut self, line: u32, tokens: &[Token]) -> Result<(), ParseError> {
        let shape = "wave N SAMPLES: N is 0-15, SAMPLES 32 hex digits (one per sample, 0-F)";
        if tokens.len() < 3 {
            return Err(at(
                line,
                &tokens[0],
                "a wave line needs its number and samples",
                shape,
            ));
        }
        let n = number(line, &tokens[1], 0, WAVES as i64 - 1, "the wave number")?;
        if read_bang(&tokens[1].text).is_some() {
            return Err(at(line, &tokens[1], "a wave number is 0-15", shape));
        }
        self.once(format!("wave {n}"), line, &tokens[0])?;
        let samples: Option<Vec<u8>> = match &tokens[2..] {
            [one] if one.text.len() == WAVE_LEN => one
                .text
                .chars()
                .map(|c| c.to_digit(16).map(|d| d as u8))
                .collect(),
            many if many.len() == WAVE_LEN => many
                .iter()
                .map(|t| {
                    (t.text.len() == 2)
                        .then(|| u8::from_str_radix(&t.text, 16).ok())
                        .flatten()
                })
                .collect(),
            _ => None,
        };
        let samples = samples
            .ok_or_else(|| at(line, &tokens[2], "these are not a wave's 32 samples", shape))?;
        self.song.waves[n as usize].copy_from_slice(&samples);
        Ok(())
    }

    fn routine(&mut self, line: u32, tokens: &[Token]) -> Result<(), ParseError> {
        let shape = "routine N, then | lines and end (routine N crlf for CRLF line ends), or \
                     routine N \"TEXT\"";
        if tokens.len() < 2 {
            return Err(at(
                line,
                &tokens[0],
                "a routine line needs its number",
                shape,
            ));
        }
        let index = tokens[1]
            .text
            .parse::<usize>()
            .ok()
            .filter(|n| *n < ROUTINES)
            .ok_or_else(|| {
                at(
                    line,
                    &tokens[1],
                    format!("routine `{}` is not 0-15", tokens[1].text),
                    shape,
                )
            })?;
        self.once(format!("routine {index}"), line, &tokens[0])?;
        match tokens[2..]
            .iter()
            .map(|t| t.text.as_str())
            .collect::<Vec<_>>()[..]
        {
            [] | ["crlf"] => {
                self.section = Section::Routine(Routine {
                    index,
                    crlf: tokens.len() == 3,
                    lines: Vec::new(),
                });
                Ok(())
            }
            [_] if tokens[2].text.starts_with('"') => {
                let super::lex::Kind::Str(bytes) = &tokens[2].kind else {
                    unreachable!("a token starting with a quote is a string")
                };
                self.song.routines[index] = bytes.clone();
                Ok(())
            }
            _ => Err(at(
                line,
                &tokens[2],
                "this routine line has the wrong shape",
                shape,
            )),
        }
    }

    fn close_routine(&mut self) {
        if let Section::Routine(r) = std::mem::replace(&mut self.section, Section::Top) {
            let newline = if r.crlf { "\r\n" } else { "\n" };
            self.song.routines[r.index] = r.lines.join(newline).into_bytes();
        }
    }

    /// Ends the open block.
    fn close(&mut self) -> Result<(), ParseError> {
        match std::mem::replace(&mut self.section, Section::Top) {
            Section::Instrument(block) => {
                self.song.instruments[block.index] = block.ins;
            }
            Section::Pattern => self.blocks.close()?,
            Section::Routine(_) | Section::Orders | Section::Top => {}
        }
        Ok(())
    }

    fn finish(mut self) -> Result<Song, ParseError> {
        if let Section::Routine(r) = &self.section {
            return Err(ParseError {
                line: self.last_line,
                column: 1,
                message: format!("the file ends inside routine {}", r.index),
                remedy: "end the routine with a line reading end".into(),
            });
        }
        if !self.format_seen {
            return Err(ParseError {
                line: 1,
                column: 1,
                message: "a song text starts with its format line".into(),
                remedy: format!("make the first line `{FORMAT_LINE}`"),
            });
        }
        self.close()?;
        let (patterns, orders) = self.blocks.finish(self.orders)?;
        self.song.patterns = patterns;
        self.song.orders = orders;
        Ok(self.song)
    }
}
