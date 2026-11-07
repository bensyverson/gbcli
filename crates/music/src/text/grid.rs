//! The orders and the pattern blocks. A block shows up to four patterns
//! side by side, one per channel, as the orders play them together; a
//! pattern several order rows play appears in several blocks, and every
//! copy must read the same.

use std::collections::BTreeMap;

use super::lex::{Kind, Token};
use super::spell;
use super::values::{args, at, row};
use crate::error::ParseError;
use crate::model::{BLANK_ROWS, Cell, Pattern, ROWS, Rows};

/// A pattern named in the text, and where.
#[derive(Clone, Copy)]
struct Named {
    key: i32,
    line: u32,
    column: u32,
}

/// `P12` → 12; `-` → none.
fn name(line: u32, token: &Token) -> Result<Option<Named>, ParseError> {
    if token.text == "-" {
        return Ok(None);
    }
    let key = token
        .text
        .strip_prefix('P')
        .and_then(|k| k.parse::<i32>().ok())
        .ok_or_else(|| {
            at(
                line,
                token,
                format!("`{}` is not a pattern name", token.text),
                "a pattern is named P and its number (P0, P12), and - stands for none",
            )
        })?;
    Ok(Some(Named {
        key,
        line,
        column: token.column,
    }))
}

/// Each order row's four patterns.
#[derive(Default)]
pub struct Orders {
    rows: Vec<[Option<Named>; 4]>,
}

impl Orders {
    /// `NN P0 P1 P2 P3`.
    pub fn line(&mut self, line: u32, tokens: &[Token]) -> Result<(), ParseError> {
        let next = self.rows.len();
        if tokens[0].text.parse::<usize>().ok() != Some(next) {
            return Err(at(
                line,
                &tokens[0],
                format!(
                    "order `{}` is out of sequence; {next:02} comes next",
                    tokens[0].text
                ),
                "orders are numbered from 00 in steps of one",
            ));
        }
        let a = args(
            line,
            tokens,
            4,
            "NN P0 P1 P2 P3: the order's number, then the pattern each channel plays",
        )?;
        let mut entry = [None; 4];
        for (c, t) in a.iter().enumerate() {
            entry[c] = name(line, t)?;
            let ended = self.rows.last().is_some_and(|r| r[c].is_none());
            if entry[c].is_some() && ended {
                return Err(at(
                    line,
                    t,
                    format!("channel {}'s orders ended with - on the row above", c + 1),
                    "- marks the end of a channel's (shorter) orders; give the channel a \
                     pattern on every row up to here",
                ));
            }
        }
        self.rows.push(entry);
        Ok(())
    }
}

/// The first copy of each pattern the text shows.
struct Shown {
    rows: Rows,
    /// The block's `pattern` line.
    line: u32,
}

struct Open {
    line: u32,
    names: [Option<Named>; 4],
    rows: [Rows; 4],
    /// Each given row's line and its four cells' columns.
    lines: [Option<(u32, [u32; 4])>; ROWS],
    last_row: Option<usize>,
}

#[derive(Default)]
pub struct Blocks {
    open: Option<Open>,
    shown: BTreeMap<i32, Shown>,
    storage: Vec<Named>,
}

const CELL_SHAPE: &str = "a pattern row is the row number, then four cells (square 1, square \
                          2, wave, noise) separated by |, each a note, instrument, volume and \
                          effect: 00 C-5 01 ... 2FF | ... .. ... ... | ...";

impl Blocks {
    /// `pattern P0 P1 P2 P3`.
    pub fn start(&mut self, line: u32, tokens: &[Token]) -> Result<(), ParseError> {
        let a = args(
            line,
            tokens,
            4,
            "pattern P0 P1 P2 P3: the pattern each column shows, or - for none",
        )?;
        let mut names = [None; 4];
        for (c, t) in a.iter().enumerate() {
            names[c] = name(line, t)?;
        }
        self.open = Some(Open {
            line,
            names,
            rows: [BLANK_ROWS; 4],
            lines: [None; ROWS],
            last_row: None,
        });
        Ok(())
    }

    /// `NN cell | cell | cell | cell`.
    pub fn row(&mut self, line: u32, tokens: &[Token]) -> Result<(), ParseError> {
        let open = self
            .open
            .as_mut()
            .expect("a row comes inside a pattern block");
        let n = row(line, &tokens[0])?;
        if let Some(last) = open.last_row.filter(|last| n <= *last) {
            return Err(at(
                line,
                &tokens[0],
                format!("row {n:02} comes after row {last:02}"),
                "rows go in ascending order, 00-63; a row left out is blank",
            ));
        }
        let cells: Vec<&[Token]> = tokens[1..].split(|t| t.kind == Kind::Bar).collect();
        if cells.len() != 4 {
            return Err(at(
                line,
                &tokens[0],
                format!("this row has {} cells", cells.len()),
                CELL_SHAPE,
            ));
        }
        let mut columns = [0; 4];
        for (c, fields) in cells.iter().enumerate() {
            let Some(first) = fields.first() else {
                return Err(at(
                    line,
                    &tokens[0],
                    format!("cell {} is empty", c + 1),
                    CELL_SHAPE,
                ));
            };
            if fields.len() != 4 {
                return Err(at(
                    line,
                    first,
                    format!("cell {} has {} fields", c + 1, fields.len()),
                    CELL_SHAPE,
                ));
            }
            let cell = cell(line, fields)?;
            if open.names[c].is_none() && !cell.is_blank() {
                return Err(at(
                    line,
                    first,
                    format!("channel {} has no pattern in this block (-)", c + 1),
                    "name a pattern for the column on the pattern line, or leave its cells \
                     blank (... .. ... ...)",
                ));
            }
            open.rows[c][n] = cell;
            columns[c] = first.column;
        }
        open.lines[n] = Some((line, columns));
        open.last_row = Some(n);
        Ok(())
    }

    /// Ends the open block: each pattern it shows becomes known, or is
    /// checked against the copy shown before.
    pub fn close(&mut self) -> Result<(), ParseError> {
        let Some(open) = self.open.take() else {
            return Ok(());
        };
        for (c, named) in open.names.iter().enumerate() {
            let Some(named) = named else { continue };
            let rows = open.rows[c];
            let Some(first) = self.shown.get(&named.key) else {
                self.shown.insert(
                    named.key,
                    Shown {
                        rows,
                        line: open.line,
                    },
                );
                continue;
            };
            let Some(r) = (0..ROWS).find(|r| first.rows[*r] != rows[*r]) else {
                continue;
            };
            let (line, column) = match open.lines[r] {
                Some((line, columns)) => (line, columns[c]),
                None => (named.line, named.column),
            };
            return Err(ParseError {
                line,
                column,
                message: format!(
                    "pattern P{} differs from its copy in the block at line {}, at row {r:02}",
                    named.key, first.line
                ),
                remedy: "a pattern shown in several blocks must read the same in each: edit \
                         every copy, or give this column a new pattern name here and in the \
                         orders"
                    .into(),
            });
        }
        Ok(())
    }

    /// `storage P3 P1 ...`: the order the `.uge` stores patterns in.
    pub fn storage(&mut self, line: u32, tokens: &[Token]) -> Result<(), ParseError> {
        for t in &tokens[1..] {
            let named = name(line, t)?.ok_or_else(|| {
                at(
                    line,
                    t,
                    "storage lists patterns, not -",
                    "list pattern names: P0 P1 ...",
                )
            })?;
            if self.storage.iter().any(|s| s.key == named.key) {
                return Err(at(
                    line,
                    t,
                    format!("P{} is listed twice", named.key),
                    "list each pattern once",
                ));
            }
            self.storage.push(named);
        }
        Ok(())
    }

    /// The patterns in storage order (the `storage` line's, then the rest by
    /// number) and each channel's orders.
    pub fn finish(mut self, orders: Orders) -> Result<(Vec<Pattern>, [Vec<i32>; 4]), ParseError> {
        self.close()?;
        let missing = |named: &Named, what: String| ParseError {
            line: named.line,
            column: named.column,
            message: format!("{what} P{}, which no pattern block shows", named.key),
            remedy: format!(
                "add a pattern block that shows P{} (a `pattern` line naming it, then its \
                 rows), or name a pattern that has one",
                named.key
            ),
        };
        let mut patterns = Vec::with_capacity(self.shown.len());
        for named in &self.storage {
            let shown = self
                .shown
                .remove(&named.key)
                .ok_or_else(|| missing(named, "storage lists".into()))?;
            patterns.push(Pattern {
                key: named.key,
                rows: shown.rows,
            });
        }
        let stored: Vec<i32> = self.storage.iter().map(|s| s.key).collect();
        let mut all_keys: Vec<i32> = stored.clone();
        for (key, shown) in std::mem::take(&mut self.shown) {
            all_keys.push(key);
            patterns.push(Pattern {
                key,
                rows: shown.rows,
            });
        }
        let mut out: [Vec<i32>; 4] = Default::default();
        for (i, entry) in orders.rows.iter().enumerate() {
            for (c, named) in entry.iter().enumerate() {
                let Some(named) = named else { continue };
                if !all_keys.contains(&named.key) {
                    return Err(missing(named, format!("order {i:02} plays")));
                }
                out[c].push(named.key);
            }
        }
        Ok((patterns, out))
    }
}

/// Note, instrument, volume, effect.
fn cell(line: u32, f: &[Token]) -> Result<Cell, ParseError> {
    let bad = |t: &Token, what: &str, remedy: &str| {
        at(line, t, format!("`{}` is not {what}", t.text), remedy)
    };
    let note = spell::read_note(&f[0].text).ok_or_else(|| {
        bad(
            &f[0],
            "a note",
            "notes are C-3 to B-8 (C-, C#, D-, D#, E-, F-, F#, G-, G#, A-, A#, B-, then the \
             octave), or ... for none",
        )
    })?;
    let instrument = spell::read_instrument(&f[1].text).ok_or_else(|| {
        bad(
            &f[1],
            "an instrument",
            "an instrument is 01-15 from the channel's bank, or .. to keep the last one",
        )
    })?;
    let volume = spell::read_volume(&f[2].text).ok_or_else(|| {
        bad(
            &f[2],
            "a volume",
            "the volume column is unused: write ... (set a volume with effect Cxy)",
        )
    })?;
    let (effect_code, effect_param) = spell::read_effect(&f[3].text).ok_or_else(|| {
        bad(
            &f[3],
            "an effect",
            "an effect is three hex digits, the code then the parameter (C08), or ... for \
             none; `gbcli help music` lists the codes",
        )
    })?;
    Ok(Cell {
        note,
        instrument,
        volume,
        effect_code,
        effect_param,
    })
}
