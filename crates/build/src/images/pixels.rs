//! A PNG as RGBA pixels, whatever its color type and depth.

use std::fs;
use std::path::Path;

/// One pixel: red, green, blue, alpha.
pub(crate) type Rgba = [u8; 4];

/// An image's pixels, row-major.
pub(crate) struct Pixels {
    pub width: u32,
    pub height: u32,
    pub rgba: Vec<Rgba>,
}

/// Under `--gbc`, the color image beside the DMG version a check reads:
/// its name in messages, since a limit counted on joint tiles is the
/// color image's to meet, and its pixels.
#[derive(Clone, Copy)]
pub(crate) struct Color<'a> {
    pub image: &'a str,
    pub pixels: &'a Pixels,
}

impl Pixels {
    pub fn at(&self, x: u32, y: u32) -> Rgba {
        self.rgba[(y * self.width + x) as usize]
    }

    /// Decodes the PNG at `path`; the error is the decoder's, for a message.
    pub fn read(path: &Path) -> Result<Pixels, String> {
        let file = fs::File::open(path).map_err(|e| e.to_string())?;
        let mut decoder = png::Decoder::new(std::io::BufReader::new(file));
        // 16-bit samples are kept and scaled below, as rgbgfx scales them:
        // the png crate's STRIP_16 keeps the high byte, which can split a
        // color rgbgfx sees as one.
        decoder.set_transformations(png::Transformations::EXPAND);
        let mut reader = decoder.read_info().map_err(|e| e.to_string())?;
        let mut buf = vec![0; reader.output_buffer_size()];
        let info = reader.next_frame(&mut buf).map_err(|e| e.to_string())?;
        let bytes = &buf[..info.buffer_size()];
        let scaled: Vec<u8>;
        let bytes = if info.bit_depth == png::BitDepth::Sixteen {
            scaled = bytes
                .chunks(2)
                .map(|b| scale_16(u16::from_be_bytes([b[0], b[1]])))
                .collect();
            &scaled[..]
        } else {
            bytes
        };
        let rgba = match info.color_type {
            png::ColorType::Grayscale => bytes.iter().map(|&g| [g, g, g, 0xFF]).collect(),
            png::ColorType::GrayscaleAlpha => {
                bytes.chunks(2).map(|p| [p[0], p[0], p[0], p[1]]).collect()
            }
            png::ColorType::Rgb => bytes.chunks(3).map(|p| [p[0], p[1], p[2], 0xFF]).collect(),
            png::ColorType::Rgba => bytes.chunks(4).map(|p| [p[0], p[1], p[2], p[3]]).collect(),
            png::ColorType::Indexed => return Err("indexed color was not expanded".into()),
        };
        Ok(Pixels {
            width: info.width,
            height: info.height,
            rgba,
        })
    }
}

/// `#rrggbb`.
pub(crate) fn hex(c: Rgba) -> String {
    format!("#{:02x}{:02x}{:02x}", c[0], c[1], c[2])
}

/// A 16-bit sample as 8 bits, as rgbgfx reads it: libpng's
/// `png_set_scale_16`.
pub(crate) fn scale_16(v: u16) -> u8 {
    // pngrtran.c, png_do_scale_16_to_8: a fast (v * 255 + 32767) / 65535.
    let [high, low] = v.to_be_bytes();
    let high = i32::from(high);
    (high + (((i32::from(low) - high + 128) * 65535) >> 24)) as u8
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_16_bit_sample_rounds_as_libpng_scales_it() {
        assert_eq!(scale_16(0x0000), 0x00);
        assert_eq!(scale_16(0xFFFF), 0xFF);
        // The high byte alone would give $07 and $08: these differ in RGB555.
        assert_eq!(scale_16(0x07FF), 0x08);
        assert_eq!(scale_16(0x0800), 0x08);
        assert_eq!(scale_16(0x807F), 0x80);
        assert_eq!(scale_16(0x8080), 0x80);
        for v in 0..=u16::MAX {
            let rounded = ((u32::from(v) * 255 + 32767) / 65535) as u8;
            assert_eq!(scale_16(v), rounded, "{v:#06x}");
        }
    }
}
