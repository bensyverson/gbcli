//! Previews, for looking only: a sample line or the whole font, enlarged,
//! with tile boundaries marked and, for the whole font, the columns past
//! each glyph's width tinted.

use gbcli_view::Image;
use gbcli_view::colors::{DMG_GRAYS, GAP};

use crate::layout::Line;
use crate::model::{Font, HEIGHT};

/// How many times larger a preview is than the pixels.
pub const PREVIEW_SCALE: u32 = 4;

const PAPER: u32 = DMG_GRAYS[0];
const INK: u32 = DMG_GRAYS[3];
/// A cell's columns past its glyph's width.
const PAST: u32 = 0xF0_C8_F0;
/// Glyphs to a row in a font's preview.
const PER_ROW: u32 = 16;

/// Draws a `w`x`h`-pixel picture at [`PREVIEW_SCALE`], a line on every
/// tile boundary, from `color(x, y)`.
fn png(w: u32, h: u32, color: impl Fn(u32, u32) -> u32) -> Vec<u8> {
    let (sw, sh) = ((w * PREVIEW_SCALE).max(1), (h * PREVIEW_SCALE).max(1));
    let cell = 8 * PREVIEW_SCALE;
    let mut pixels = Vec::with_capacity((sw * sh) as usize);
    for y in 0..sh {
        for x in 0..sw {
            let edge = (x > 0 && x % cell == 0) || (y > 0 && y % cell == 0);
            pixels.push(if edge || w == 0 {
                GAP
            } else {
                color(x / PREVIEW_SCALE, y / PREVIEW_SCALE)
            });
        }
    }
    let image = Image {
        width: sw,
        height: sh,
        pixels,
        labels: Vec::new(),
    };
    image.png().expect("a preview encodes")
}

/// `line` at [`PREVIEW_SCALE`], ink dark on light paper, across all its
/// tiles, a line on every tile boundary, as PNG bytes.
pub fn preview_line(line: &Line) -> Vec<u8> {
    png(line.tiles * 8, HEIGHT as u32, |x, y| {
        let byte = line.data[(x / 8) as usize * HEIGHT + y as usize];
        if byte & (0x80 >> (x % 8)) != 0 {
            INK
        } else {
            PAPER
        }
    })
}

/// Every glyph of `font` in glyph-number order, 16 to a row, each in its
/// own 8x8 cell from the cell's left edge, the columns past its width
/// tinted, as PNG bytes.
pub fn preview_font(font: &Font) -> Vec<u8> {
    let n = font.glyphs.len() as u32;
    let rows = n.div_ceil(PER_ROW);
    let cols = if rows > 1 { PER_ROW } else { n };
    png(cols * 8, rows * 8, |x, y| {
        let i = (y / 8) * PER_ROW + x / 8;
        let Some(g) = font.glyphs.get(i as usize) else {
            return PAST;
        };
        let column = (x % 8) as u8;
        if column >= g.width {
            PAST
        } else if g.ink(column, (y % 8) as usize) {
            INK
        } else {
            PAPER
        }
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::key::Key;
    use crate::model::Glyph;

    fn decode(png: &[u8]) -> (u32, u32) {
        // Width and height from the IHDR chunk.
        let be = |b: &[u8]| u32::from_be_bytes([b[0], b[1], b[2], b[3]]);
        (be(&png[16..20]), be(&png[20..24]))
    }

    #[test]
    fn previews_are_four_times_the_tiles_they_show() {
        let font = Font {
            glyphs: (0..17)
                .map(|i| Glyph {
                    comment: vec![],
                    key: Key::Char(char::from(b'A' + i)),
                    width: 5,
                    rows: [0xF8; 8],
                })
                .collect(),
            ..Font::default()
        };
        assert_eq!(decode(&preview_font(&font)), (16 * 32, 2 * 32));
        let line = crate::layout::set(&font, "ABC");
        assert_eq!(decode(&preview_line(&line)), (2 * 32, 32));
    }
}
