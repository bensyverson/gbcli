//! Reference screenshots and frame comparison.

use std::collections::HashMap;
use std::path::Path;

use gbcli_core::{FrameRef, Pixels};

use crate::manifest::Palette;

/// The Game Boy screen.
pub const WIDTH: u32 = 160;
pub const HEIGHT: u32 = 144;
/// Where the game screen sits inside SameBoy's 256×224 SGB frame.
const SGB_BORDER: (u32, u32, u32) = (256, 48, 40);

/// An RGB image, row-major.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Image {
    pub width: u32,
    pub height: u32,
    pub rgb: Vec<[u8; 3]>,
}

/// How a frame compares with its reference.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Match {
    Exact,
    /// Not equal, but a one-to-one color map makes it equal: the same picture
    /// in other colors.
    Recolored,
    Differs {
        pixels: usize,
        first: (u32, u32),
    },
    WrongSize {
        got: (u32, u32),
    },
}

/// The four DMG shades as 8-bit gray, lightest first (`dmg_gray`).
const DMG_GRAY: [u8; 4] = [0xFF, 0xAA, 0x55, 0x00];

/// Decodes a PNG to 8-bit RGB, whatever its color type.
pub fn load_png(path: &Path) -> Result<Image, String> {
    let file = std::fs::File::open(path).map_err(|e| format!("{}: {e}", path.display()))?;
    let mut decoder = png::Decoder::new(std::io::BufReader::new(file));
    decoder.set_transformations(png::Transformations::EXPAND | png::Transformations::STRIP_16);
    let mut reader = decoder
        .read_info()
        .map_err(|e| format!("{}: {e}", path.display()))?;
    let mut buf = vec![0; reader.output_buffer_size()];
    let info = reader
        .next_frame(&mut buf)
        .map_err(|e| format!("{}: {e}", path.display()))?;
    let channels = info.color_type.samples();
    let rgb = buf[..info.buffer_size()]
        .chunks_exact(channels)
        .map(|px| match channels {
            1 | 2 => [px[0]; 3],
            _ => [px[0], px[1], px[2]],
        })
        .collect();
    Ok(Image {
        width: info.width,
        height: info.height,
        rgb,
    })
}

/// Writes an image as an 8-bit RGB PNG.
pub fn save_png(path: &Path, image: &Image) -> Result<(), String> {
    let file = std::fs::File::create(path).map_err(|e| format!("{}: {e}", path.display()))?;
    let mut encoder = png::Encoder::new(std::io::BufWriter::new(file), image.width, image.height);
    encoder.set_color(png::ColorType::Rgb);
    encoder.set_depth(png::BitDepth::Eight);
    let data: Vec<u8> = image.rgb.iter().flatten().copied().collect();
    encoder
        .write_header()
        .and_then(|mut w| w.write_image_data(&data))
        .map_err(|e| format!("{}: {e}", path.display()))
}

/// A core's frame as RGB under a manifest palette, cropped to the game screen.
pub fn render(frame: &FrameRef<'_>, palette: Palette) -> Result<Image, String> {
    let rgb: Vec<[u8; 3]> = match (frame.pixels, palette) {
        (Pixels::Shades(shades), Palette::DmgGray) => shades
            .iter()
            .map(|&s| [DMG_GRAY[usize::from(s & 3)]; 3])
            .collect(),
        (Pixels::Rgb(rgb), Palette::CgbRgb555 | Palette::CgbCompatCsp) => rgb
            .iter()
            .map(|&p| [(p >> 16) as u8, (p >> 8) as u8, p as u8])
            .collect(),
        (Pixels::Rgb(rgb), Palette::CgbRgb555Shift) => rgb
            .iter()
            .map(|&p| {
                [
                    (p >> 16) as u8 & 0xF8,
                    (p >> 8) as u8 & 0xF8,
                    p as u8 & 0xF8,
                ]
            })
            .collect(),
        (pixels, palette) => {
            let kind = match pixels {
                Pixels::Shades(_) => "DMG shades",
                Pixels::Rgb(_) => "RGB",
            };
            return Err(format!(
                "a frame of {kind} can't be shown in palette {palette:?}"
            ));
        }
    };
    Ok(crop(Image {
        width: frame.width,
        height: frame.height,
        rgb,
    }))
}

/// Cuts SameBoy's SGB border away; other sizes pass through.
fn crop(image: Image) -> Image {
    let (border_width, x0, y0) = SGB_BORDER;
    if image.width != border_width {
        return image;
    }
    let rgb = (0..HEIGHT)
        .flat_map(|y| {
            let start = ((y0 + y) * border_width + x0) as usize;
            image.rgb[start..start + WIDTH as usize].iter().copied()
        })
        .collect();
    Image {
        width: WIDTH,
        height: HEIGHT,
        rgb,
    }
}

/// Compares a frame with its reference pixel by pixel.
pub fn compare(actual: &Image, reference: &Image) -> Match {
    if (actual.width, actual.height) != (reference.width, reference.height) {
        return Match::WrongSize {
            got: (actual.width, actual.height),
        };
    }
    let mut differing = actual
        .rgb
        .iter()
        .zip(&reference.rgb)
        .enumerate()
        .filter(|(_, (a, r))| a != r);
    let Some((first, _)) = differing.next() else {
        return Match::Exact;
    };
    let pixels = 1 + differing.count();
    if recolorable(actual, reference) {
        return Match::Recolored;
    }
    let first = first as u32;
    Match::Differs {
        pixels,
        first: (first % actual.width, first / actual.width),
    }
}

/// Whether one consistent one-to-one color map turns `a` into `b`.
fn recolorable(a: &Image, b: &Image) -> bool {
    let mut forward = HashMap::new();
    let mut backward = HashMap::new();
    a.rgb
        .iter()
        .zip(&b.rgb)
        .all(|(x, y)| *forward.entry(x).or_insert(y) == y && *backward.entry(y).or_insert(x) == x)
}

#[cfg(test)]
mod tests {
    use super::*;
    use gbcli_core::{FrameTag, Ticks, VblankKind};

    const TAG: FrameTag = FrameTag {
        number: 1,
        kind: VblankKind::Normal,
        at: Ticks(0),
    };

    fn image(rgb: Vec<[u8; 3]>, width: u32) -> Image {
        Image {
            width,
            height: rgb.len() as u32 / width,
            rgb,
        }
    }

    #[test]
    fn dmg_shades_render_as_gray_lightest_first() {
        let frame = FrameRef {
            tag: TAG,
            width: 4,
            height: 1,
            pixels: Pixels::Shades(&[0, 1, 2, 3]),
        };
        let got = render(&frame, Palette::DmgGray).unwrap();
        assert_eq!(got.rgb, vec![[0xFF; 3], [0xAA; 3], [0x55; 3], [0; 3]]);
    }

    #[test]
    fn rgb_frames_render_channel_by_channel() {
        let frame = FrameRef {
            tag: TAG,
            width: 1,
            height: 1,
            pixels: Pixels::Rgb(&[0x00_12_34_56]),
        };
        let got = render(&frame, Palette::CgbRgb555).unwrap();
        assert_eq!(got.rgb, vec![[0x12, 0x34, 0x56]]);
    }

    #[test]
    fn rgb_frames_in_the_shift_palette_drop_the_low_bit_fill() {
        // SameBoy emits (x << 3) | (x >> 2); the shift convention is x << 3.
        let frame = FrameRef {
            tag: TAG,
            width: 2,
            height: 1,
            pixels: Pixels::Rgb(&[0x00_FF_FF_FF, 0x00_84_00_08]),
        };
        let got = render(&frame, Palette::CgbRgb555Shift).unwrap();
        assert_eq!(got.rgb, vec![[0xF8; 3], [0x80, 0x00, 0x08]]);
    }

    #[test]
    fn a_frame_in_the_wrong_palette_kind_is_an_error() {
        let frame = FrameRef {
            tag: TAG,
            width: 1,
            height: 1,
            pixels: Pixels::Shades(&[0]),
        };
        assert!(render(&frame, Palette::CgbRgb555).is_err());
    }

    #[test]
    fn an_sgb_frame_is_cropped_to_the_game_screen() {
        let mut pixels = vec![0u32; 256 * 224];
        pixels[40 * 256 + 48] = 0x00FF_FFFF; // the game screen's top-left pixel
        let frame = FrameRef {
            tag: TAG,
            width: 256,
            height: 224,
            pixels: Pixels::Rgb(&pixels),
        };
        let got = render(&frame, Palette::CgbRgb555).unwrap();
        assert_eq!((got.width, got.height), (160, 144));
        assert_eq!(got.rgb[0], [0xFF; 3]);
        assert_eq!(got.rgb[1], [0; 3]);
    }

    #[test]
    fn compare_reports_exact_recolored_differs_and_size() {
        let (w, k, r) = ([0xFF; 3], [0; 3], [0xFF, 0, 0]);
        let reference = image(vec![w, k, k, w], 2);
        assert_eq!(
            compare(&image(vec![w, k, k, w], 2), &reference),
            Match::Exact
        );
        assert_eq!(
            compare(&image(vec![r, k, k, r], 2), &reference),
            Match::Recolored
        );
        assert_eq!(
            compare(&image(vec![w, k, w, w], 2), &reference),
            Match::Differs {
                pixels: 1,
                first: (0, 1)
            }
        );
        assert_eq!(
            compare(&image(vec![w, w, w, w], 2), &reference),
            Match::Differs {
                pixels: 2,
                first: (1, 0)
            },
            "two colors merged into one is not a recoloring"
        );
        assert_eq!(
            compare(&image(vec![w, k], 2), &reference),
            Match::WrongSize { got: (2, 1) }
        );
    }
}
