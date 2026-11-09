//! A font's PNG working copy: GB Studio's grid of 8x8 cells, 16 to a row
//! (128 pixels wide), glyphs in glyph-number order, each at its cell's
//! left edge, with its `.chars` sidecar naming the cells.
//!
//! In the image, an opaque pixel is ink (the darker of the image's two
//! opaque colors) or paper, and a glyph's width runs to its cell's last
//! column holding an opaque pixel; every pixel left of that is opaque.
//! Any other pixel is clear: alpha 0, or alpha 1-254, a guide, which the
//! conversion drops. The writer's guides tint every other cell and the
//! rows below the baseline, in clear pixels only, so they never touch a
//! glyph or its width.

mod read;
mod sheet;

use gbcli_pixels::{Raster, Rgba};

use crate::chars::{GRID_COLUMNS, Slot, write_chars};
use crate::key::Key;
use crate::model::{DEFAULT_ASCENT, Font, HEIGHT};

pub use read::from_grid;
pub use sheet::guide_sheet;

/// The paper the writer draws: the DMG's lightest shade.
pub const PAPER: Rgba = [0xFF, 0xFF, 0xFF, 0xFF];
/// The ink the writer draws: the DMG's darkest shade.
pub const INK: Rgba = [0x00, 0x00, 0x00, 0xFF];
/// The guide tinting every other cell.
pub const CELL_GUIDE: Rgba = [0x40, 0x80, 0xFF, 0x40];
/// The guide tinting the rows below the baseline.
pub const BASELINE_GUIDE: Rgba = [0xFF, 0x60, 0x20, 0x50];
/// The image's width: 16 cells.
pub const GRID_WIDTH: u32 = GRID_COLUMNS as u32 * 8;

/// A PNG working copy, before encoding: the image and its `.chars` text.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Grid {
    pub image: Raster,
    pub chars: String,
}

/// `font` as a grid, with guides when `guides`.
pub fn to_grid(font: &Font, guides: bool) -> Grid {
    let slots: Vec<Slot> = font.glyphs.iter().map(|g| Some(g.key.clone())).collect();
    let mut grid = blank(&slots, guides.then(|| font.ascent()));
    for (i, g) in font.glyphs.iter().enumerate() {
        let (cx, cy) = ((i % GRID_COLUMNS) as u32 * 8, (i / GRID_COLUMNS) as u32 * 8);
        for row in 0..HEIGHT {
            for column in 0..g.width {
                let p = if g.ink(column, row) { INK } else { PAPER };
                put(&mut grid.image, cx + u32::from(column), cy + row as u32, p);
            }
        }
    }
    grid
}

/// An empty grid for `keys`, in order, with guides: where to draw a new
/// font, its baseline at [`DEFAULT_ASCENT`].
pub fn new_grid(keys: &[Key]) -> Grid {
    let slots: Vec<Slot> = keys.iter().cloned().map(Some).collect();
    blank(&slots, Some(DEFAULT_ASCENT))
}

/// A grid of clear cells for `slots`, guided below `ascent` when given.
fn blank(slots: &[Slot], ascent: Option<u8>) -> Grid {
    let rows = slots.len().div_ceil(GRID_COLUMNS).max(1);
    let height = rows as u32 * 8;
    let mut image = Raster {
        width: GRID_WIDTH,
        height,
        rgba: vec![[0, 0, 0, 0]; (GRID_WIDTH * height) as usize],
    };
    if let Some(ascent) = ascent {
        for y in 0..height {
            for x in 0..GRID_WIDTH {
                let guide = if (y % 8) as u8 >= ascent {
                    Some(BASELINE_GUIDE)
                } else if (x / 8 + y / 8) % 2 == 1 {
                    Some(CELL_GUIDE)
                } else {
                    None
                };
                if let Some(p) = guide {
                    put(&mut image, x, y, p);
                }
            }
        }
    }
    let chars: Vec<Vec<Slot>> = slots.chunks(GRID_COLUMNS).map(<[Slot]>::to_vec).collect();
    Grid {
        image,
        chars: write_chars(&chars),
    }
}

fn put(image: &mut Raster, x: u32, y: u32, p: Rgba) {
    let w = image.width;
    image.rgba[(y * w + x) as usize] = p;
}

#[cfg(test)]
mod tests;
