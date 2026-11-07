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

impl Pixels {
    pub fn at(&self, x: u32, y: u32) -> Rgba {
        self.rgba[(y * self.width + x) as usize]
    }

    /// Decodes the PNG at `path`; the error is the decoder's, for a message.
    pub fn read(path: &Path) -> Result<Pixels, String> {
        let file = fs::File::open(path).map_err(|e| e.to_string())?;
        let mut decoder = png::Decoder::new(std::io::BufReader::new(file));
        decoder.set_transformations(png::Transformations::EXPAND | png::Transformations::STRIP_16);
        let mut reader = decoder.read_info().map_err(|e| e.to_string())?;
        let mut buf = vec![0; reader.output_buffer_size()];
        let info = reader.next_frame(&mut buf).map_err(|e| e.to_string())?;
        let bytes = &buf[..info.buffer_size()];
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
