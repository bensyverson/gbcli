//! The pixels crate's integration tests, one binary: parsing, writing and
//! converting images.

mod canonical;
mod errors;
mod examples;
mod fuzz;
mod png;
mod preview;
mod text;

use gbcli_pixels::{Palettes, Picture, Raster, parse_text};

/// Parses `text`, panicking with the error.
pub fn parse(text: &str) -> Picture {
    parse_text(text).unwrap_or_else(|e| panic!("{e} ({})\n{text}", e.remedy))
}

/// `text`'s pixels, its palette references resolved through `palettes`.
pub fn raster(text: &str, palettes: &Palettes) -> Raster {
    parse(text)
        .raster(palettes)
        .unwrap_or_else(|e| panic!("{e}"))
}

/// The pixels as `#rrggbb` words (`.` for clear), a row a line.
pub fn draw(r: &Raster) -> String {
    let mut out = String::new();
    for y in 0..r.height {
        let row: Vec<String> = (0..r.width)
            .map(|x| match r.at(x, y) {
                [_, _, _, 0] => ".".to_string(),
                [r, g, b, _] => format!("{r:02x}{g:02x}{b:02x}"),
            })
            .collect();
        out += &row.join(" ");
        out.push('\n');
    }
    out
}
