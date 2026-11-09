//! The layout rules, which `gbcli help fonts` states normatively and
//! `gbcli font --sample` follows; a renderer (the starter's, LoadHigh's)
//! matches this one pixel for pixel.
//!
//! The rules are [`RULES`], word for word what `gbcli help fonts` prints.

use crate::model::{Font, Glyph, HEIGHT};

/// The layout rules, as `gbcli help fonts` states them (normative).
pub const RULES: &str = "  1. A line starts at the left edge of its first tile, with the pen at 0.
  2. A glyph's column 0 is drawn at the pen, and the pen then moves right
     by the glyph's width. Nothing else moves it: there is no kerning and
     no gap. A glyph's spacing is drawn into it, as paper columns inside
     its width, and a glyph starts at its cell's left edge.
  3. Pixel x of the line is in tile x / 8, at bit 7 - x % 8 of its row's
     byte: 1bpp, 8 bytes a tile, row 0 first, a set bit ink.
  4. A glyph replaces exactly its own columns and keeps every other pixel.
     With g its row's byte, w its width, o = x % 8 the pen's offset in its
     tile and m = $FF << (8 - w), every value kept to 8 bits:
       first tile:                   row = (row & ~(m >> o)) | ((g & m) >> o)
       next tile, when o + w > 8:    row = (row & ~(m << (8 - o)))
                                           | ((g & m) << (8 - o))
     The bits of g right of w are clear in every gbcli font, but a renderer
     that masks them as above never draws them anyway.
  5. A line touches (width + 7) / 8 tiles. On screen, ink and paper are two
     of the four shades: expand each 1bpp row into the two bit planes.
";

/// Text set in a font.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Line {
    /// The glyph numbers drawn, in order.
    pub glyphs: Vec<usize>,
    /// What the font has no glyph for, each once, in order of first use.
    pub missing: Vec<String>,
    /// Pixels from the line's left edge to the pen's last position.
    pub width: u32,
    /// The tiles the line touches: width / 8, rounded up.
    pub tiles: u32,
    /// The tiles, 1bpp: 8 bytes each, row 0 first, bit 7 the leftmost.
    pub data: Vec<u8>,
}

/// Sets `text` in `font`. A string names a glyph as its charmap does:
/// a character by itself, a named glyph as `<name>`, longest match first;
/// what matches no glyph is missing, and takes no room.
pub fn set(font: &Font, text: &str) -> Line {
    // Each glyph's charmap string, longest first, so `<heart>` wins over `<`.
    let mut strings: Vec<(String, usize)> = font
        .glyphs
        .iter()
        .enumerate()
        .map(|(i, g)| (g.key.charmap(), i))
        .collect();
    strings.sort_by(|a, b| b.0.len().cmp(&a.0.len()).then(a.1.cmp(&b.1)));
    let mut glyphs = Vec::new();
    let mut missing: Vec<String> = Vec::new();
    let mut rest = text;
    while let Some(c) = rest.chars().next() {
        match strings.iter().find(|(s, _)| rest.starts_with(s.as_str())) {
            Some((s, i)) => {
                glyphs.push(*i);
                rest = &rest[s.len()..];
            }
            None => {
                rest = &rest[c.len_utf8()..];
                let c = c.to_string();
                if !missing.contains(&c) {
                    missing.push(c);
                }
            }
        }
    }
    let width: u32 = glyphs
        .iter()
        .map(|&i| u32::from(font.glyphs[i].width))
        .sum();
    let tiles = width.div_ceil(8);
    let mut data = vec![0u8; tiles as usize * HEIGHT];
    let mut x = 0;
    for &i in &glyphs {
        draw(&mut data, x, &font.glyphs[i]);
        x += u32::from(font.glyphs[i].width);
    }
    Line {
        glyphs,
        missing,
        width,
        tiles,
        data,
    }
}

/// Draws `glyph` into `data` (tiles as [`Line::data`]) with its column 0
/// at pixel `x`, by rule 4.
pub fn draw(data: &mut [u8], x: u32, glyph: &Glyph) {
    let tile = (x / 8) as usize * HEIGHT;
    let o = x % 8;
    let width = u32::from(glyph.width);
    let mask = (0xFF00u16 >> width) as u8;
    for (r, &row) in glyph.rows.iter().enumerate() {
        let bits = row & mask;
        let first = &mut data[tile + r];
        *first = (*first & !(mask >> o)) | (bits >> o);
        if o + width > 8 {
            let next = &mut data[tile + HEIGHT + r];
            let back = 8 - o;
            *next = (*next & !(mask << back)) | (bits << back);
        }
    }
}

#[cfg(test)]
mod tests;
