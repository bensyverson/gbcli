//! The guide sheet (`gbcli font new --guide-sheet`): the grid at 4x, its
//! guides shown over white, each cell over a strip naming its key (a code
//! point in hex, or a name), to keep open beside the editor.

use gbcli_pixels::{Raster, Rgba, encode_png};

use crate::chars::{GRID_COLUMNS, read_chars};
use crate::labels::{self, ADVANCE};

const SCALE: u32 = 4;
const CELL: u32 = 8 * SCALE;
const STRIP: u32 = 8;
const STRIP_PAPER: Rgba = [0xE8, 0xE8, 0xE8, 0xFF];
const LABEL_INK: Rgba = [0x20, 0x20, 0x40, 0xFF];

/// `image` (a grid) at 4x, each cell labeled from `chars`, as PNG bytes.
/// A `.chars` that doesn't read labels nothing.
pub fn guide_sheet(image: &Raster, chars: &str) -> Vec<u8> {
    let rows = (image.height / 8).max(1);
    let width = image.width * SCALE;
    let height = rows * (CELL + STRIP);
    let mut sheet = Raster {
        width,
        height,
        rgba: vec![STRIP_PAPER; (width * height) as usize],
    };
    for y in 0..image.height {
        for x in 0..image.width {
            let p = over_white(image.rgba[(y * image.width + x) as usize]);
            let top = (y / 8) * (CELL + STRIP) + (y % 8) * SCALE;
            for dy in 0..SCALE {
                for dx in 0..SCALE {
                    let i = ((top + dy) * width + x * SCALE + dx) as usize;
                    sheet.rgba[i] = p;
                }
            }
        }
    }
    let slots = read_chars(chars).unwrap_or_default();
    for (r, row) in slots.iter().enumerate().take(rows as usize) {
        for (c, slot) in row.iter().enumerate().take(GRID_COLUMNS) {
            let Some(key) = slot else { continue };
            let (left, top) = (c as u32 * CELL + 1, r as u32 * (CELL + STRIP) + CELL + 2);
            labels::draw(&labels::label(key), (CELL / ADVANCE) as usize, |x, y| {
                sheet.rgba[((top + y) * width + left + x) as usize] = LABEL_INK;
            });
        }
    }
    encode_png(&sheet)
}

/// `p` laid over white.
fn over_white(p: Rgba) -> Rgba {
    let a = u32::from(p[3]);
    let mix = |c: u8| ((u32::from(c) * a + 255 * (255 - a)) / 255) as u8;
    [mix(p[0]), mix(p[1]), mix(p[2]), 0xFF]
}
