//! gbcli's pixel text: images an agent can write and read. A `.pixels`
//! file (`format gbcli-pixels 1`) draws an image with a legend of
//! characters, in rows, or in 8x8 tiles placed by a map; this crate parses
//! it, writes it canonically, reads and writes PNG, and converts both ways
//! losslessly. It is to images what song text is to music.
//!
//! The format is specified in project/2026-11-08-pixel-text-design.md and
//! taught by `gbcli help pixels`; [`EXAMPLE`] is a complete file in it.

mod error;
mod example;
mod model;
mod name;
mod png;
mod preview;
mod text;

pub use error::{LayoutError, ParseError, PixelsError, PngError};
pub use example::EXAMPLE;
pub use model::{CLEAR, DMG, Palettes, Picture, Raster, Rgb, Rgba, Symbol, TILE, Value};
pub use name::{DMG_SUFFIX, extension_of, frame_size, is_text_name, size_word};
pub use png::{MADE_SYMBOLS, decode_png, encode_png, from_raster, is_png};
pub use preview::{PREVIEW_SCALE, preview_png};
pub use text::{EXTENSION, FORMAT_LINE, Layout, MAX_SIDE, MAX_TEXT_SIDE, parse_text, to_text};

/// Which form an image file is in.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Form {
    Png,
    Text,
}

impl Form {
    pub fn name(self) -> &'static str {
        match self {
            Form::Png => "png",
            Form::Text => "text",
        }
    }
}

/// An image file, read in whichever form it is.
#[derive(Clone, Debug)]
pub struct Loaded {
    pub form: Form,
    pub picture: Picture,
}

/// Reads `bytes` as a PNG when they start with its signature, and as pixel
/// text otherwise.
pub fn read(bytes: &[u8]) -> Result<Loaded, PixelsError> {
    if is_png(bytes) {
        let raster = decode_png(bytes)?;
        return Ok(Loaded {
            form: Form::Png,
            picture: from_raster(&raster)?,
        });
    }
    let text = std::str::from_utf8(bytes).map_err(|e| {
        let valid = &bytes[..e.valid_up_to()];
        let line = valid.iter().filter(|b| **b == b'\n').count() as u32 + 1;
        ParseError {
            line,
            column: 1,
            message: "this line is not UTF-8, so the file is neither a PNG nor pixel text".into(),
            remedy: format!("an image file is a PNG, or UTF-8 text starting with `{FORMAT_LINE}`"),
        }
    })?;
    Ok(Loaded {
        form: Form::Text,
        picture: parse_text(text)?,
    })
}
