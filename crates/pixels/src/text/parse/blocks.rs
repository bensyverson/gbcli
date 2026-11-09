//! Blocks: `rows`, `tile` and `map` headers, the rows under them, and the
//! blank line that ends them.

use super::{
    AT_REMEDY, Block, KEY_REMEDY, Open, Paint, Parser, SAME_WIDTH, TILE_REMEDY, and_list, err,
    is_directive, is_tile_key, number,
};
use crate::error::ParseError;
use crate::model::TILE;
use crate::text::lex::Word;

impl Parser {
    pub(super) fn open(&mut self, line: u32, words: &[Word]) -> Result<(), ParseError> {
        let head = &words[0];
        let (width, height) = self.sized(line, head)?;
        self.blocks = true;
        let open = if head.text == "tile" {
            let [_, key] = words else {
                return Err(err(line, head.column, "tile takes one key", KEY_REMEDY));
            };
            if !is_tile_key(key.text) {
                return Err(err(
                    line,
                    key.column,
                    format!("`{}` is not a tile key", key.text),
                    KEY_REMEDY,
                ));
            }
            if let Some(&first) = self.by_key.get(key.text) {
                return Err(err(
                    line,
                    key.column,
                    format!(
                        "tile {} is defined already (line {})",
                        key.text, self.tiles[first].2
                    ),
                    "give each tile its own key",
                ));
            }
            Open::Tile {
                key: key.text.to_string(),
                column: key.column,
                pixels: Vec::new(),
            }
        } else {
            let (x, y) = match words {
                [_] => (0, 0),
                [_, x, y] => {
                    let at = |w: &Word, axis: &str, limit: u32, extent: &str| {
                        let n = number(w.text).ok_or_else(|| {
                            err(
                                line,
                                w.column,
                                format!("`{}` is not a number of pixels", w.text),
                                AT_REMEDY,
                            )
                        })?;
                        if n >= limit {
                            return Err(err(
                                line,
                                w.column,
                                format!(
                                    "{axis} {n} is outside the image, which is {limit} {extent}"
                                ),
                                format!(
                                    "start the block inside the image: {axis} is 0 to {}",
                                    limit - 1
                                ),
                            ));
                        }
                        Ok(n)
                    };
                    (at(x, "x", width, "wide")?, at(y, "y", height, "high")?)
                }
                _ => {
                    return Err(err(
                        line,
                        head.column,
                        format!("{} takes no position, or an X and a Y", head.text),
                        AT_REMEDY,
                    ));
                }
            };
            if head.text == "rows" {
                Open::Rows { x, y }
            } else {
                Open::Map { x, y }
            }
        };
        self.block = Some(Block {
            open,
            line,
            column: head.column,
            rows: 0,
            width: None,
        });
        Ok(())
    }

    /// A line inside the open block.
    pub(super) fn row(&mut self, line: u32, words: &[Word]) -> Result<(), ParseError> {
        let mut block = self.block.take().expect("a block is open");
        let result = match &block.open {
            Open::Map { .. } => self.map_row(&mut block, line, words),
            _ => self.pixel_row(&mut block, line, words),
        };
        self.block = Some(block);
        result
    }

    /// A directive inside a block that hasn't ended: a header, or a line of
    /// its own (`color`, `fill`, `size`, `format`).
    pub(super) fn unended(line: u32, word: &Word) -> ParseError {
        let message = if matches!(word.text, "rows" | "tile" | "map") {
            format!(
                "`{}` starts a block, but the block above hasn't ended",
                word.text
            )
        } else {
            format!(
                "`{}` is a line of its own, not a row, but the block above hasn't ended",
                word.text
            )
        };
        err(
            line,
            word.column,
            message,
            "put a blank line before it: a block ends at a blank line",
        )
    }

    pub(super) fn pixel_row(
        &mut self,
        block: &mut Block,
        line: u32,
        words: &[Word],
    ) -> Result<(), ParseError> {
        let (width, height) = self.sized(line, &words[0])?;
        if words.len() > 1 {
            if is_directive(words[0].text) {
                return Err(Self::unended(line, &words[0]));
            }
            return Err(err(
                line,
                words[1].column,
                "a row is one word, but this line has another",
                "draw a row without spaces; a block ends at a blank line",
            ));
        }
        let word = &words[0];
        let pixels: Vec<char> = word.text.chars().collect();
        let row = block.rows;
        block.rows += 1;
        if let Open::Tile {
            key, pixels: tile, ..
        } = &mut block.open
        {
            if row == TILE {
                return Err(err(
                    line,
                    word.column,
                    format!("tile {key} has a ninth row; a tile is 8 rows"),
                    TILE_REMEDY,
                ));
            }
            if pixels.len() != TILE as usize {
                return Err(err(
                    line,
                    word.column,
                    format!("a tile row is 8 pixels; this one is {}", pixels.len()),
                    TILE_REMEDY,
                ));
            }
            let mut drawn = Vec::with_capacity(pixels.len());
            for (&c, column) in pixels.iter().zip(word.column..) {
                drawn.push(self.def(c, line, column)?);
            }
            tile.extend(drawn);
            return Ok(());
        }
        let Open::Rows { x, y } = block.open else {
            unreachable!("map rows are read apart")
        };
        if y + row >= height {
            return Err(err(
                line,
                word.column,
                format!("this row is past the image's bottom edge: the image is {height} high"),
                "end the block sooner, or make the image taller with size",
            ));
        }
        let first = *block.width.get_or_insert(pixels.len());
        if pixels.len() != first {
            return Err(err(
                line,
                word.column,
                format!(
                    "this row is {} pixels wide, and the block's first is {first}",
                    pixels.len()
                ),
                SAME_WIDTH,
            ));
        }
        for ((&c, column), px) in pixels.iter().zip(word.column..).zip(x..) {
            let def = self.def(c, line, column)?;
            if px >= width {
                return Err(err(
                    line,
                    column,
                    format!("this row runs past the image's right edge: the image is {width} wide"),
                    "shorten the row, or move the block left",
                ));
            }
            self.paint(px, y + row, def, block.line);
        }
        Ok(())
    }

    pub(super) fn map_row(
        &mut self,
        block: &mut Block,
        line: u32,
        words: &[Word],
    ) -> Result<(), ParseError> {
        let (width, height) = self.sized(line, &words[0])?;
        let first = &words[0];
        if is_directive(first.text) && !self.by_key.contains_key(first.text) {
            return Err(Self::unended(line, first));
        }
        let Open::Map { x, y } = block.open else {
            unreachable!("only maps")
        };
        let top = y + block.rows * TILE;
        block.rows += 1;
        let count = *block.width.get_or_insert(words.len());
        if words.len() != count {
            let tiles = |n: usize| {
                if n == 1 {
                    "1 tile".into()
                } else {
                    format!("{n} tiles")
                }
            };
            return Err(err(
                line,
                first.column,
                format!(
                    "this row names {}, and the block's first names {count}",
                    tiles(words.len())
                ),
                SAME_WIDTH,
            ));
        }
        let mut tiles = Vec::with_capacity(words.len());
        for word in words {
            let Some(&tile) = self.by_key.get(word.text) else {
                let keys: Vec<String> = self.tiles.iter().map(|t| t.0.clone()).collect();
                let known = match keys.len() {
                    0 => "no tile is defined yet".to_string(),
                    _ => format!("the tiles are {}", and_list(&keys)),
                };
                return Err(err(
                    line,
                    word.column,
                    format!("no tile {}: {known}", word.text),
                    format!(
                        "name a tile defined with `tile KEY`, or define {}",
                        word.text
                    ),
                ));
            };
            tiles.push(tile);
        }
        if top + TILE > height {
            return Err(err(
                line,
                first.column,
                format!(
                    "this row of tiles runs past the image's bottom edge: the image is {height} \
                     high"
                ),
                "map fewer rows, or move the block up",
            ));
        }
        let lefts = (x..).step_by(TILE as usize);
        for ((word, tile), left) in words.iter().zip(tiles).zip(lefts) {
            if left + TILE > width {
                return Err(err(
                    line,
                    word.column,
                    format!(
                        "this tile runs past the image's right edge: the image is {width} wide"
                    ),
                    "map fewer tiles, or move the block left",
                ));
            }
            for i in 0..(TILE * TILE) as usize {
                let def = self.tiles[tile].1[i];
                self.paint(left + i as u32 % TILE, top + i as u32 / TILE, def, line);
            }
        }
        Ok(())
    }

    pub(super) fn paint(&mut self, x: u32, y: u32, def: usize, line: u32) {
        if matches!(self.defs[def].paint, Paint::Keep) {
            return;
        }
        let width = self.size.as_ref().map_or(0, |s| s.width);
        self.canvas[(y * width + x) as usize] = Some((def, line));
    }

    /// Ends the open block, if any: a blank line or the file's end.
    pub(super) fn close(&mut self) -> Result<(), ParseError> {
        let Some(block) = self.block.take() else {
            return Ok(());
        };
        match block.open {
            Open::Tile {
                key,
                column,
                pixels,
            } => {
                if block.rows != TILE {
                    let rows = match block.rows {
                        1 => "1 row".to_string(),
                        n => format!("{n} rows"),
                    };
                    return Err(err(
                        block.line,
                        column,
                        format!("tile {key} has {rows}; a tile is 8"),
                        TILE_REMEDY,
                    ));
                }
                self.by_key.insert(key.clone(), self.tiles.len());
                self.tiles.push((key, pixels, block.line));
            }
            Open::Rows { .. } | Open::Map { .. } if block.rows == 0 => {
                let kind = if matches!(block.open, Open::Rows { .. }) {
                    "rows"
                } else {
                    "map"
                };
                return Err(err(
                    block.line,
                    block.column,
                    format!("this {kind} block has no rows"),
                    "draw its rows under it, before a blank line",
                ));
            }
            Open::Rows { .. } | Open::Map { .. } => {}
        }
        Ok(())
    }
}
