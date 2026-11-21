//! A picture as canonical pixel text: the legend entries some pixel uses,
//! in legend order, then rows by frame or band, or tiles and a map.

use std::fmt::Write;

use super::{FORMAT_LINE, Layout, MAX_SIDE};
use crate::error::LayoutError;
use crate::model::{Picture, TILE, Value};

/// The canonical text of `picture` in `layout`.
pub fn to_text(picture: &Picture, layout: Layout) -> Result<String, LayoutError> {
    // Text past the parser's size would be written and then refused.
    if picture.width > MAX_SIDE || picture.height > MAX_SIDE {
        return Err(LayoutError {
            message: format!(
                "the image is {}x{}, and gbcli writes pixel text for 1 to {MAX_SIDE} pixels a side",
                picture.width, picture.height
            ),
            remedy: format!(
                "keep it a PNG, or split it into images of at most {MAX_SIDE} pixels a side"
            ),
        });
    }
    let mut out = format!("{FORMAT_LINE}\nsize {} {}\n", picture.width, picture.height);
    for i in picture.used() {
        let s = &picture.legend[usize::from(i)];
        let _ = writeln!(out, "color {} {}", s.symbol, value(&s.value));
    }
    match layout {
        Layout::Rows { frame } => rows(picture, frame, &mut out),
        Layout::Tiles => tiles(picture, &mut out)?,
    }
    Ok(out)
}

fn value(v: &Value) -> String {
    match v {
        Value::Color([r, g, b]) => format!("#{r:02x}{g:02x}{b:02x}"),
        Value::Clear => "clear".into(),
        Value::Entry { palette, index } => format!("{palette} {index}"),
    }
}

/// The pixels from (`x`, `y`), `width` wide and `height` high, a row a line.
fn block(picture: &Picture, (x, y): (u32, u32), (width, height): (u32, u32), out: &mut String) {
    for row in y..y + height {
        out.extend((x..x + width).map(|col| picture.at(col, row).symbol));
        out.push('\n');
    }
}

fn rows(picture: &Picture, frame: Option<(u32, u32)>, out: &mut String) {
    let (w, h) = (picture.width, picture.height);
    let whole =
        frame.filter(|&(fw, fh)| fw > 0 && fh > 0 && w.is_multiple_of(fw) && h.is_multiple_of(fh));
    if let Some((fw, fh)) = whole {
        let mut k = 0;
        for y in (0..h).step_by(fh as usize) {
            for x in (0..w).step_by(fw as usize) {
                let _ = writeln!(out, "\nrows {x} {y} # frame {k}");
                block(picture, (x, y), (fw, fh), out);
                k += 1;
            }
        }
        return;
    }
    for y in (0..h).step_by(TILE as usize) {
        let _ = writeln!(out, "\nrows 0 {y}");
        block(picture, (0, y), (w, TILE.min(h - y)), out);
    }
}

fn tiles(picture: &Picture, out: &mut String) -> Result<(), LayoutError> {
    let Some((cells, tiles)) = picture.tile_grid() else {
        return Err(LayoutError {
            message: format!(
                "the image is {}x{}, which isn't whole 8x8 tiles, so it can't be written as tiles",
                picture.width, picture.height
            ),
            remedy: "write it as rows: drop --tiles".into(),
        });
    };
    // Every key as wide as the last, two hex digits at least.
    let digits = format!("{:x}", tiles.len().saturating_sub(1)).len().max(2);
    let key = |n: usize| format!("{n:0digits$x}");
    for (n, tile) in tiles.iter().enumerate() {
        let _ = writeln!(out, "\ntile {}", key(n));
        for row in tile.chunks(TILE as usize) {
            out.extend(row.iter().map(|&i| picture.legend[usize::from(i)].symbol));
            out.push('\n');
        }
    }
    out.push_str("\nmap 0 0\n");
    for row in cells.chunks((picture.width / TILE) as usize) {
        let keys: Vec<String> = row.iter().map(|&n| key(n)).collect();
        out.push_str(&keys.join(" "));
        out.push('\n');
    }
    Ok(())
}
