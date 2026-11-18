//! An RGB raster with labels, and the geometry every view shares: crops,
//! nearest-neighbor integer scaling, blits and outlines.

use crate::ViewError;
use crate::encode;

/// The scale a screenshot gets when the caller names none.
pub const DEFAULT_SCALE: u32 = 4;
/// The largest integer scale: 16× a 256×256 map is already 4096 pixels wide.
pub const MAX_SCALE: u32 = 16;

/// A rectangle in an image's own pixels.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Rect {
    pub x: u32,
    pub y: u32,
    pub width: u32,
    pub height: u32,
}

impl Rect {
    /// The `gbcli:detail` label of an image cropped to this rectangle.
    pub fn crop_label(&self) -> String {
        format!(
            "crop x={} y={} w={} h={}",
            self.x, self.y, self.width, self.height
        )
    }
}

/// The pixels of a `width`×`height` row-major raster inside `rect`, which
/// must be non-empty and lie wholly inside it.
pub(crate) fn crop_rows<T: Copy>(
    pixels: &[T],
    width: u32,
    height: u32,
    rect: Rect,
) -> Result<Vec<T>, ViewError> {
    let inside = rect.width > 0
        && rect.height > 0
        && rect.x.checked_add(rect.width).is_some_and(|r| r <= width)
        && rect.y.checked_add(rect.height).is_some_and(|b| b <= height);
    if !inside {
        return Err(ViewError::CropOutside {
            rect,
            width,
            height,
        });
    }
    let mut out = Vec::with_capacity((rect.width * rect.height) as usize);
    for y in rect.y..rect.y + rect.height {
        let start = (y * width + rect.x) as usize;
        out.extend_from_slice(&pixels[start..start + rect.width as usize]);
    }
    Ok(out)
}

/// What a PNG's text chunk says about the image; written as `gbcli:<key>`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LabelKey {
    /// Which view drew it: `frame`, `sheet`, `diff`, `tileset`, `tilemap`, `oam`.
    View,
    /// Which frame (or frames) it shows: complete with its number and vblank
    /// kind, or live and partial with the line reached.
    Frame,
    /// Where a debug view's colors come from.
    Colors,
    /// What the view's parameters were (palette, map, viewport).
    Detail,
}

impl LabelKey {
    pub fn keyword(self) -> &'static str {
        match self {
            LabelKey::View => "gbcli:view",
            LabelKey::Frame => "gbcli:frame",
            LabelKey::Colors => "gbcli:colors",
            LabelKey::Detail => "gbcli:detail",
        }
    }
}

/// An image as 0x00RRGGBB pixels, row-major, with the labels its PNG carries.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Image {
    pub width: u32,
    pub height: u32,
    pub pixels: Vec<u32>,
    pub labels: Vec<(LabelKey, String)>,
}

impl Image {
    /// A `width`×`height` image of one color, unlabeled.
    pub fn filled(width: u32, height: u32, color: u32) -> Image {
        Image {
            width,
            height,
            pixels: vec![color; (width * height) as usize],
            labels: Vec::new(),
        }
    }

    /// The pixel at (`x`, `y`); panics outside the image.
    pub fn pixel(&self, x: u32, y: u32) -> u32 {
        assert!(x < self.width && y < self.height, "({x}, {y}) is outside");
        self.pixels[(y * self.width + x) as usize]
    }

    pub(crate) fn set(&mut self, x: u32, y: u32, color: u32) {
        if x < self.width && y < self.height {
            self.pixels[(y * self.width + x) as usize] = color;
        }
    }

    /// The image with one more label, which its PNG carries as a text chunk.
    pub fn label(mut self, key: LabelKey, text: impl Into<String>) -> Image {
        self.labels.push((key, text.into()));
        self
    }

    /// The text of the first label under `key`.
    pub fn label_text(&self, key: LabelKey) -> Option<&str> {
        self.labels
            .iter()
            .find(|(k, _)| *k == key)
            .map(|(_, t)| t.as_str())
    }

    /// Each pixel as a `factor`×`factor` block (nearest neighbor; no smoothing).
    pub fn scaled(&self, factor: u32) -> Result<Image, ViewError> {
        if !(1..=MAX_SCALE).contains(&factor) {
            return Err(ViewError::BadScale { scale: factor });
        }
        let width = self.width * factor;
        let mut pixels = Vec::with_capacity((width * self.height * factor) as usize);
        for row in self.pixels.chunks(self.width.max(1) as usize) {
            let wide: Vec<u32> = row
                .iter()
                .flat_map(|&p| std::iter::repeat_n(p, factor as usize))
                .collect();
            for _ in 0..factor {
                pixels.extend_from_slice(&wide);
            }
        }
        Ok(Image {
            width,
            height: self.height * factor,
            pixels,
            labels: self.labels.clone(),
        })
    }

    /// The part of the image inside `rect`, which must be non-empty and lie
    /// wholly inside it.
    pub fn cropped(&self, rect: Rect) -> Result<Image, ViewError> {
        let pixels = crop_rows(&self.pixels, self.width, self.height, rect)?;
        Ok(Image {
            width: rect.width,
            height: rect.height,
            pixels,
            labels: self.labels.clone(),
        }
        .label(LabelKey::Detail, rect.crop_label()))
    }

    /// Copies `src` with its top-left corner at (`x`, `y`), clipping at the edges.
    pub(crate) fn blit(&mut self, src: &Image, x: u32, y: u32) {
        for sy in 0..src.height {
            for sx in 0..src.width {
                self.set(x + sx, y + sy, src.pixel(sx, sy));
            }
        }
    }

    /// A one-pixel outline of a `width`×`height` rectangle at (`x`, `y`) that
    /// wraps around the image's edges, as the PPU's viewport wraps the map.
    pub(crate) fn outline_wrapped(&mut self, x: u32, y: u32, width: u32, height: u32, color: u32) {
        let (w, h) = (self.width, self.height);
        for dx in 0..width {
            self.set((x + dx) % w, y % h, color);
            self.set((x + dx) % w, (y + height - 1) % h, color);
        }
        for dy in 0..height {
            self.set(x % w, (y + dy) % h, color);
            self.set((x + width - 1) % w, (y + dy) % h, color);
        }
    }

    /// The image as PNG bytes: palette-indexed when it has at most 256
    /// colors, else 8-bit RGB, with its labels as text chunks. The same image
    /// always gives the same bytes.
    pub fn png(&self) -> Result<Vec<u8>, ViewError> {
        encode::png(self)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ramp(width: u32, height: u32) -> Image {
        Image {
            width,
            height,
            pixels: (0..width * height).collect(),
            labels: Vec::new(),
        }
    }

    #[test]
    fn scaling_repeats_each_pixel_in_a_block() {
        let image = ramp(2, 2).scaled(3).unwrap();
        assert_eq!((image.width, image.height), (6, 6));
        assert_eq!(image.pixels[..6], [0, 0, 0, 1, 1, 1]);
        assert_eq!(image.pixels[12..18], [0, 0, 0, 1, 1, 1]);
        assert_eq!(image.pixels[18..24], [2, 2, 2, 3, 3, 3]);
        assert_eq!(ramp(2, 2).scaled(1).unwrap().pixels, [0, 1, 2, 3]);
    }

    #[test]
    fn a_crop_keeps_the_rectangle_and_says_so() {
        let rect = Rect {
            x: 1,
            y: 1,
            width: 2,
            height: 1,
        };
        let image = ramp(4, 3).cropped(rect).unwrap();
        assert_eq!(image.pixels, [5, 6]);
        assert_eq!(
            image.label_text(LabelKey::Detail),
            Some("crop x=1 y=1 w=2 h=1")
        );
        let past = Rect { x: 3, ..rect };
        assert!(ramp(4, 3).cropped(past).is_err());
    }

    #[test]
    fn an_outline_wraps_around_the_edges() {
        let mut image = Image::filled(4, 4, 0);
        image.outline_wrapped(3, 3, 2, 2, 9);
        let set: Vec<u32> = image.pixels.iter().map(|&p| p / 9).collect();
        #[rustfmt::skip]
        let expected = [
            1, 0, 0, 1,
            0, 0, 0, 0,
            0, 0, 0, 0,
            1, 0, 0, 1,
        ];
        assert_eq!(set, expected);
    }
}
