//! An image as RGBA pixels: a PNG of any color type and depth, or pixel
//! text, both read by `gbcli-pixels`.

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

    /// Reads the image at `path`, a PNG or pixel text (told apart by
    /// content, as `gbcli-pixels` reads them), pixel text's palette entries
    /// resolved through `palettes`. Pixel text comes back with its picture,
    /// whose lines locate a problem in it.
    pub fn read(path: &Path, palettes: &gbcli_pixels::Palettes) -> Result<Read, Unread> {
        let bytes = fs::read(path).map_err(|e| Unread::Png(e.to_string()))?;
        // A file named as pixel text, or one that starts as it does (a
        // manifest's file may be named anything), is read as pixel text,
        // so its problems are located; anything else is a PNG.
        let named = path
            .file_name()
            .is_some_and(|n| gbcli_pixels::is_text_name(&n.to_string_lossy()));
        let text = bytes
            .strip_prefix("\u{FEFF}".as_bytes())
            .unwrap_or(&bytes)
            .starts_with(b"format ");
        if gbcli_pixels::is_png(&bytes) || !(named || text) {
            let r = gbcli_pixels::decode_png(&bytes).map_err(|e| {
                let decoder = e.message.strip_prefix("the PNG doesn't decode: ");
                Unread::Png(decoder.unwrap_or(&e.message).to_string())
            })?;
            return Ok(Read {
                pixels: r.into(),
                text: None,
            });
        }
        let picture = match gbcli_pixels::read(&bytes) {
            Ok(loaded) => loaded.picture,
            Err(gbcli_pixels::PixelsError::Text(e)) => return Err(Unread::Text(e)),
            Err(e) => return Err(Unread::Png(e.to_string())),
        };
        let raster = picture.raster(palettes).map_err(Unread::Text)?;
        Ok(Read {
            pixels: raster.into(),
            text: Some(picture),
        })
    }
}

impl From<gbcli_pixels::Raster> for Pixels {
    fn from(r: gbcli_pixels::Raster) -> Pixels {
        Pixels {
            width: r.width,
            height: r.height,
            rgba: r.rgba,
        }
    }
}

/// An image read: its pixels, and its picture when it is pixel text.
pub(crate) struct Read {
    pub pixels: Pixels,
    pub text: Option<gbcli_pixels::Picture>,
}

/// Why an image didn't read.
pub(crate) enum Unread {
    /// Not a PNG gbcli can read (nor pixel text): the decoder's message.
    Png(String),
    /// Pixel text that doesn't parse, or names a palette entry the
    /// palettes don't have.
    Text(gbcli_pixels::ParseError),
}

/// `#rrggbb`.
pub(crate) fn hex(c: Rgba) -> String {
    format!("#{:02x}{:02x}{:02x}", c[0], c[1], c[2])
}
