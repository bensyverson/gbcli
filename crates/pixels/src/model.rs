//! A picture as legend symbols, and as the RGBA pixels a PNG holds.

use std::collections::HashMap;

use crate::error::ParseError;

/// One pixel: red, green, blue, alpha.
pub type Rgba = [u8; 4];
/// A color: red, green, blue.
pub type Rgb = [u8; 3];

/// The pixel a transparent pixel is written as: a PNG's alpha 0 means
/// clear whatever its color, so every clear pixel is this one.
pub const CLEAR: Rgba = [0, 0, 0, 0];

/// What a legend character stands for, once painting is done (`keep` is
/// only a way to paint, so no picture holds it).
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub enum Value {
    /// `#rrggbb`, all 8 bits a channel.
    Color(Rgb),
    /// `clear`: alpha 0.
    Clear,
    /// `PALETTE N`: entry N (0-3) of a palette in `assets/palettes.txt`;
    /// `dmg N` is the dmg line's color N.
    Entry { palette: String, index: u8 },
}

/// A legend entry: a character and its value.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Symbol {
    pub symbol: char,
    pub value: Value,
    /// Where the value was written (line, column), in pixel text: what a
    /// value that doesn't resolve is located to.
    pub at: Option<(u32, u32)>,
}

/// An image as legend symbols: every pixel is an index into `legend`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Picture {
    pub width: u32,
    pub height: u32,
    /// In the order the text defined them, or a PNG's made order.
    pub legend: Vec<Symbol>,
    /// Row-major.
    pub pixels: Vec<u16>,
    /// From pixel text, the line that drew each pixel, row-major: a rows
    /// block's header, a map's row, or `fill`. Empty from a PNG.
    pub lines: Vec<u32>,
}

/// An image as RGBA pixels, row-major: what a PNG decodes to.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Raster {
    pub width: u32,
    pub height: u32,
    pub rgba: Vec<Rgba>,
}

/// The project's palettes, for `PALETTE N` and `dmg N`: each color at 8
/// bits a channel.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Palettes {
    /// The `dmg` line's colors, shade 0 first, when the file has one.
    pub dmg: Option<[Rgb; 4]>,
    pub named: Vec<(String, [Rgb; 4])>,
}

/// The reserved palette name whose entries are the DMG shades.
pub const DMG: &str = "dmg";

/// A tile's side in pixels.
pub const TILE: u32 = 8;

impl Picture {
    /// The legend entry of the pixel at (`x`, `y`).
    pub fn at(&self, x: u32, y: u32) -> &Symbol {
        &self.legend[usize::from(self.pixels[(y * self.width + x) as usize])]
    }

    /// The line that drew the pixel at (`x`, `y`), in pixel text.
    pub fn line_at(&self, x: u32, y: u32) -> Option<u32> {
        self.lines.get((y * self.width + x) as usize).copied()
    }

    /// The legend entries some pixel uses, other than clear ones: the
    /// image's colors, by legend identity.
    pub fn colors(&self) -> u32 {
        self.used()
            .iter()
            .filter(|&&i| self.legend[usize::from(i)].value != Value::Clear)
            .count() as u32
    }

    /// Whether some pixel is clear.
    pub fn has_clear(&self) -> bool {
        self.used()
            .iter()
            .any(|&i| self.legend[usize::from(i)].value == Value::Clear)
    }

    /// The distinct 8x8 tiles, by legend identity and with no flips; none
    /// when the size isn't whole tiles.
    pub fn tiles(&self) -> Option<u32> {
        Some(self.tile_grid()?.1.len() as u32)
    }

    /// Whether some value names a palette, so drawing it needs
    /// [`Palettes`].
    pub fn needs_palettes(&self) -> bool {
        self.legend
            .iter()
            .any(|s| matches!(s.value, Value::Entry { .. }))
    }

    /// The pixels as RGBA, `PALETTE N` and `dmg N` resolved through
    /// `palettes`; a value that doesn't resolve is an error at its line
    /// and column. Clear pixels are [`CLEAR`].
    pub fn raster(&self, palettes: &Palettes) -> Result<Raster, ParseError> {
        // Only what is drawn must resolve, as only it is written.
        let mut colors = vec![CLEAR; self.legend.len()];
        for i in self.used() {
            let i = usize::from(i);
            colors[i] = resolve(&self.legend[i], palettes)?;
        }
        Ok(Raster {
            width: self.width,
            height: self.height,
            rgba: self
                .pixels
                .iter()
                .map(|&i| colors[usize::from(i)])
                .collect(),
        })
    }

    /// Legend indices some pixel uses, ascending.
    pub(crate) fn used(&self) -> Vec<u16> {
        let mut seen = vec![false; self.legend.len()];
        for &p in &self.pixels {
            seen[usize::from(p)] = true;
        }
        (0..self.legend.len() as u16)
            .filter(|&i| seen[usize::from(i)])
            .collect()
    }

    /// The tile grid when the size is whole tiles: each cell's tile number
    /// in raster order, and the distinct tiles in first-use order (each 64
    /// legend indices, rows top to bottom).
    pub(crate) fn tile_grid(&self) -> Option<(Vec<usize>, Vec<Vec<u16>>)> {
        if self.width == 0
            || self.height == 0
            || !self.width.is_multiple_of(TILE)
            || !self.height.is_multiple_of(TILE)
        {
            return None;
        }
        let mut cells = Vec::new();
        let mut tiles: Vec<Vec<u16>> = Vec::new();
        let mut known: HashMap<Vec<u16>, usize> = HashMap::new();
        for ty in 0..self.height / TILE {
            for tx in 0..self.width / TILE {
                let mut tile = Vec::with_capacity(64);
                for y in 0..TILE {
                    let row = (ty * TILE + y) * self.width + tx * TILE;
                    tile.extend_from_slice(&self.pixels[row as usize..(row + TILE) as usize]);
                }
                let n = *known.entry(tile.clone()).or_insert_with(|| {
                    tiles.push(tile);
                    tiles.len() - 1
                });
                cells.push(n);
            }
        }
        Some((cells, tiles))
    }
}

/// A legend entry's RGBA.
fn resolve(s: &Symbol, palettes: &Palettes) -> Result<Rgba, ParseError> {
    let (palette, index) = match &s.value {
        Value::Color([r, g, b]) => return Ok([*r, *g, *b, 0xFF]),
        Value::Clear => return Ok(CLEAR),
        Value::Entry { palette, index } => (palette.as_str(), usize::from(*index)),
    };
    let (line, column) = s.at.unwrap_or((1, 1));
    let colors = if palette == DMG {
        palettes.dmg.ok_or_else(|| ParseError {
            line,
            column,
            message: format!(
                "`{DMG} {index}` is shade {index}, but the palettes have no dmg line to say \
                 its color"
            ),
            remedy: "add `dmg: #ffffff #aaaaaa #555555 #000000` (shade 0 first) to \
                     assets/palettes.txt"
                .into(),
        })?
    } else {
        match palettes.named.iter().find(|(name, _)| name == palette) {
            Some((_, colors)) => *colors,
            None => {
                let known: Vec<&str> = palettes.named.iter().map(|(n, _)| n.as_str()).collect();
                let defined = match known.len() {
                    0 => "the palettes define none".to_string(),
                    _ => format!("the palettes define {}", known.join(", ")),
                };
                return Err(ParseError {
                    line,
                    column,
                    message: format!("no palette {palette}: {defined}"),
                    remedy: format!(
                        "define `{palette}: c0 c1 c2 c3` in assets/palettes.txt, or write the \
                         color as #rrggbb"
                    ),
                });
            }
        }
    };
    let [r, g, b] = colors[index];
    Ok([r, g, b, 0xFF])
}

impl Raster {
    /// The pixel at (`x`, `y`).
    pub fn at(&self, x: u32, y: u32) -> Rgba {
        self.rgba[(y * self.width + x) as usize]
    }

    /// The same image with every alpha-0 pixel [`CLEAR`]: equal rasters are
    /// then the same image as the build reads it.
    pub fn normalized(&self) -> Raster {
        Raster {
            rgba: self
                .rgba
                .iter()
                .map(|&p| if p[3] == 0 { CLEAR } else { p })
                .collect(),
            ..self.clone()
        }
    }
}
