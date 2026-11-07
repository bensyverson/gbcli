//! The preview: an image's converted data drawn as the DMG shows it (not
//! the source PNG), then its unique tiles (or 8x16 units) by use, most used
//! first, so the costly one-off tiles sit at the end. Drawn through
//! `gbcli-view`, so it is a deterministic, labeled PNG like every view.

use gbcli_view::colors::{GAP, TRANSPARENT};
use gbcli_view::{DEFAULT_SCALE, DMG_GRAYS, Image, LabelKey};

use super::sheet::{SheetPlan, X_FLIP, Y_FLIP};

/// Unique tiles per row of the by-use grid.
const GRID_COLUMNS: u32 = 16;
/// The widest preview before it is scaled down from [`DEFAULT_SCALE`].
const MAX_WIDTH: u32 = 2048;

/// Color index `x`, `y` of a 2bpp tile.
fn index(tile: &[u8], x: u32, y: u32) -> u8 {
    let (lo, hi) = (tile[2 * y as usize], tile[2 * y as usize + 1]);
    let bit = 7 - x;
    (((hi >> bit) & 1) << 1) | ((lo >> bit) & 1)
}

/// The order of `uses`' indices, most used first, ties by index.
fn by_use(uses: &[u32]) -> Vec<usize> {
    let mut order: Vec<usize> = (0..uses.len()).collect();
    order.sort_by_key(|&i| (std::cmp::Reverse(uses[i]), i));
    order
}

/// A canvas holding `art` (w x h) on the left and a grid of `cells`
/// cells (cw x ch) beside it.
fn canvas(w: u32, h: u32, cells: u32, cw: u32, ch: u32) -> (Image, u32) {
    let rows = cells.div_ceil(GRID_COLUMNS).max(1);
    let columns = cells.clamp(1, GRID_COLUMNS);
    let grid_x = w + 8;
    let width = grid_x + columns * (cw + 1) - 1;
    let height = h.max(rows * (ch + 1) - 1);
    (Image::filled(width, height, GAP), grid_x)
}

fn put(image: &mut Image, x: u32, y: u32, color: u32) {
    let i = (y * image.width + x) as usize;
    image.pixels[i] = color;
}

fn finish(image: Image, detail: String) -> Image {
    let scale = (MAX_WIDTH / image.width).clamp(1, DEFAULT_SCALE);
    let mut out = image.scaled(scale).expect("the scale is in range");
    out.labels = vec![
        (LabelKey::View, "image".into()),
        (
            LabelKey::Colors,
            "DMG grays; each color index is its shade (BGP $E4), objects through their OBP".into(),
        ),
        (LabelKey::Detail, detail),
    ];
    out
}

/// A background from its tiles and map (`width` x `height` tiles).
pub(crate) fn background(source: &str, tiles: &[u8], map: &[u8], width: u32, height: u32) -> Image {
    let count = (tiles.len() / 16) as u32;
    let (mut image, grid_x) = canvas(width * 8, height * 8, count, 8, 8);
    let tile = |id: u8| &tiles[usize::from(id) * 16..][..16];
    for ty in 0..height {
        for tx in 0..width {
            let id = map[(ty * width + tx) as usize];
            for y in 0..8 {
                for x in 0..8 {
                    let shade = index(tile(id), x, y);
                    put(
                        &mut image,
                        tx * 8 + x,
                        ty * 8 + y,
                        DMG_GRAYS[usize::from(shade)],
                    );
                }
            }
        }
    }
    let mut uses = vec![0u32; count as usize];
    for &id in map {
        uses[usize::from(id)] += 1;
    }
    for (slot, id) in by_use(&uses).into_iter().enumerate() {
        let (cx, cy) = (slot as u32 % GRID_COLUMNS, slot as u32 / GRID_COLUMNS);
        for y in 0..8 {
            for x in 0..8 {
                let shade = index(tile(id as u8), x, y);
                put(
                    &mut image,
                    grid_x + cx * 9 + x,
                    cy * 9 + y,
                    DMG_GRAYS[usize::from(shade)],
                );
            }
        }
    }
    let once = uses.iter().filter(|&&n| n == 1).count();
    finish(
        image,
        format!(
            "{source}: {width}x{height} tiles as shown, then its {count} unique tiles by use, \
             most used first ({once} used once)"
        ),
    )
}

/// A sprite sheet rebuilt from its kept units and frames, transparent as
/// magenta, then its units by use.
pub(crate) fn sheet(source: &str, units: &[u8], plan: &SheetPlan) -> Image {
    let (w, h) = (plan.width * 8, plan.height * 8);
    let count = plan.kept.len() as u32;
    let (mut image, grid_x) = canvas(w, h, count, 8, 16);
    for y in 0..h {
        for x in 0..w {
            put(&mut image, x, y, TRANSPARENT);
        }
    }
    let shade = |i: u8| DMG_GRAYS[usize::from((plan.obp >> (2 * i)) & 3)];
    let unit_pixel = |unit: usize, x: u32, y: u32| -> u8 {
        let data = &units[unit * 32..][..32];
        index(&data[(y / 8) as usize * 16..][..16], x, y % 8)
    };
    let (fw, fh) = plan.frame;
    let per_row = (w / fw).max(1);
    for (f, objects) in plan.frames.iter().enumerate() {
        let (fx, fy) = ((f as u32 % per_row) * fw, (f as u32 / per_row) * fh);
        for o in objects {
            for y in 0..16 {
                for x in 0..8 {
                    let sx = if o.flip & X_FLIP != 0 { 7 - x } else { x };
                    let sy = if o.flip & Y_FLIP != 0 { 15 - y } else { y };
                    let i = unit_pixel(o.unit, sx, sy);
                    if i != 0 {
                        put(&mut image, fx + o.x + x, fy + o.y + y, shade(i));
                    }
                }
            }
        }
    }
    for (slot, unit) in by_use(&plan.uses).into_iter().enumerate() {
        let (cx, cy) = (slot as u32 % GRID_COLUMNS, slot as u32 / GRID_COLUMNS);
        for y in 0..16 {
            for x in 0..8 {
                let i = unit_pixel(unit, x, y);
                let color = if i == 0 { TRANSPARENT } else { shade(i) };
                put(&mut image, grid_x + cx * 9 + x, cy * 17 + y, color);
            }
        }
    }
    finish(
        image,
        format!(
            "{source}: {} frame(s) of {fw}x{fh} as shown, then its {count} unique 8x16 objects \
             by use, most used first",
            plan.frames.len()
        ),
    )
}
