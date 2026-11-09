//! PNG to and from a [`Raster`], and a raster's made legend.

use std::collections::{HashMap, HashSet};

use crate::error::PngError;
use crate::model::{Picture, Raster, Rgb, Symbol, Value};

/// The legend a PNG's opaque colors get, lightest first: digits, letters,
/// then punctuation. `.` is clear's, and `#` and space are never legend
/// characters.
pub const MADE_SYMBOLS: &str = "0123456789abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQRSTUVWXYZ!\"$%&'()*+,-/:;<=>?@[\\]^_`{|}~";

/// Clear's character in a made legend.
const CLEAR_SYMBOL: char = '.';

const SIGNATURE: &[u8] = b"\x89PNG\r\n\x1a\n";

/// Whether `bytes` start with the PNG signature.
pub fn is_png(bytes: &[u8]) -> bool {
    bytes.starts_with(SIGNATURE)
}

/// Decodes a PNG of any color type and depth to 8-bit RGBA.
pub fn decode_png(bytes: &[u8]) -> Result<Raster, PngError> {
    let fail = |e: String| PngError {
        pixel: None,
        message: format!("the PNG doesn't decode: {e}"),
        remedy: "save it again as a PNG, or check the path".into(),
    };
    let mut decoder = png::Decoder::new(std::io::Cursor::new(bytes));
    // 16-bit samples are kept and scaled below, as rgbgfx scales them: the
    // png crate's STRIP_16 keeps the high byte, which can split a color
    // rgbgfx sees as one.
    decoder.set_transformations(png::Transformations::EXPAND);
    let mut reader = decoder.read_info().map_err(|e| fail(e.to_string()))?;
    let mut buf = vec![0; reader.output_buffer_size()];
    let info = reader
        .next_frame(&mut buf)
        .map_err(|e| fail(e.to_string()))?;
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
        png::ColorType::Indexed => return Err(fail("indexed color was not expanded".into())),
    };
    Ok(Raster {
        width: info.width,
        height: info.height,
        rgba,
    })
}

/// A 16-bit sample as 8 bits, as rgbgfx reads it: libpng's
/// `png_set_scale_16`.
pub(crate) fn scale_16(v: u16) -> u8 {
    // pngrtran.c, png_do_scale_16_to_8: a fast (v * 255 + 32767) / 65535.
    let [high, low] = v.to_be_bytes();
    let high = i32::from(high);
    (high + (((i32::from(low) - high + 128) * 65535) >> 24)) as u8
}

/// Encodes `raster` as an 8-bit PNG: RGB when nothing is transparent, else
/// RGBA. The same raster always gives the same bytes.
pub fn encode_png(raster: &Raster) -> Vec<u8> {
    let opaque = raster.rgba.iter().all(|p| p[3] == 0xFF);
    let (color, data): (png::ColorType, Vec<u8>) = if opaque {
        let data = raster
            .rgba
            .iter()
            .flat_map(|p| [p[0], p[1], p[2]])
            .collect();
        (png::ColorType::Rgb, data)
    } else {
        (png::ColorType::Rgba, raster.rgba.concat())
    };
    let mut out = Vec::new();
    let mut encoder = png::Encoder::new(&mut out, raster.width, raster.height);
    encoder.set_color(color);
    encoder.set_depth(png::BitDepth::Eight);
    encoder.set_compression(png::Compression::Best);
    // Writing to memory can't fail, and the header matches the data.
    let mut writer = encoder.write_header().expect("a valid header");
    writer
        .write_image_data(&data)
        .expect("data of the header's size");
    writer.finish().expect("written to memory");
    out
}

/// Lightness as rgbgfx weighs it: 2126 R + 7152 G + 722 B, the build's.
fn luminance([r, g, b]: Rgb) -> u32 {
    2126 * u32::from(r) + 7152 * u32::from(g) + 722 * u32::from(b)
}

/// A raster as a picture with a made legend: `.` for clear, then
/// [`MADE_SYMBOLS`] lightest first, ties by hex. A partly transparent
/// pixel, or more colors than the symbols, is an error at its pixel.
pub fn from_raster(raster: &Raster) -> Result<Picture, PngError> {
    let limit = MADE_SYMBOLS.chars().count();
    let mut colors: Vec<Rgb> = Vec::new();
    let mut seen: HashSet<Rgb> = HashSet::new();
    let mut clear = false;
    for (i, &[r, g, b, a]) in raster.rgba.iter().enumerate() {
        let pixel = Some((i as u32 % raster.width, i as u32 / raster.width));
        match a {
            0 => clear = true,
            0xFF => {
                if seen.insert([r, g, b]) {
                    if colors.len() == limit {
                        return Err(PngError {
                            pixel,
                            message: format!(
                                "this pixel is the image's {}rd color, and pixel text has {limit} \
                                 characters for colors",
                                limit + 1
                            ),
                            remedy: "use fewer colors: no Game Boy image needs this many (a \
                                     --gbc background holds 32)"
                                .into(),
                        });
                    }
                    colors.push([r, g, b]);
                }
            }
            _ => {
                return Err(PngError {
                    pixel,
                    message: format!(
                        "this pixel is partly transparent (alpha {a}), and pixel text has no \
                         spelling for that"
                    ),
                    remedy: "make every pixel opaque (alpha 255) or clear (alpha 0); the build \
                             refuses partial alpha too"
                        .into(),
                });
            }
        }
    }
    colors.sort_by_key(|&c| (std::cmp::Reverse(luminance(c)), c));
    let mut legend = Vec::new();
    if clear {
        legend.push(Symbol {
            symbol: CLEAR_SYMBOL,
            value: Value::Clear,
            at: None,
        });
    }
    let first = legend.len();
    legend.extend(
        colors
            .iter()
            .zip(MADE_SYMBOLS.chars())
            .map(|(&c, symbol)| Symbol {
                symbol,
                value: Value::Color(c),
                at: None,
            }),
    );
    let index: HashMap<Rgb, u16> = colors
        .iter()
        .enumerate()
        .map(|(i, &c)| (c, (first + i) as u16))
        .collect();
    let pixels = raster
        .rgba
        .iter()
        .map(|&p| {
            if p[3] == 0 {
                0
            } else {
                index[&[p[0], p[1], p[2]]]
            }
        })
        .collect();
    Ok(Picture {
        width: raster.width,
        height: raster.height,
        legend,
        pixels,
        lines: Vec::new(),
    })
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
        for v in 0..=u16::MAX {
            let rounded = ((u32::from(v) * 255 + 32767) / 65535) as u8;
            assert_eq!(scale_16(v), rounded, "{v:#06x}");
        }
    }
}
