//! A preview: the image enlarged with its 8x8 grid drawn, for looking only.

use gbcli_view::Image;
use gbcli_view::colors::{GAP, TRANSPARENT};

use crate::model::{Raster, TILE};

/// How many times larger a preview is than the image.
pub const PREVIEW_SCALE: u32 = 4;

/// `raster` at [`PREVIEW_SCALE`], clear pixels in the views' transparent
/// magenta, with a line on every edge between tiles, as PNG bytes. The
/// line covers a quarter of the pixels beside it, so the art still shows.
pub fn preview_png(raster: &Raster) -> Vec<u8> {
    let (w, h) = (raster.width * PREVIEW_SCALE, raster.height * PREVIEW_SCALE);
    let cell = TILE * PREVIEW_SCALE;
    let mut pixels = Vec::with_capacity((w * h) as usize);
    for y in 0..h {
        for x in 0..w {
            let edge = (x > 0 && x % cell == 0) || (y > 0 && y % cell == 0);
            pixels.push(if edge {
                GAP
            } else {
                match raster.at(x / PREVIEW_SCALE, y / PREVIEW_SCALE) {
                    [_, _, _, 0] => TRANSPARENT,
                    [r, g, b, _] => u32::from(r) << 16 | u32::from(g) << 8 | u32::from(b),
                }
            });
        }
    }
    let image = Image {
        width: w,
        height: h,
        pixels,
        labels: Vec::new(),
    };
    // An image of a valid size always encodes.
    image.png().expect("a preview encodes")
}
