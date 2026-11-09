//! The grid read back: the image and its `.chars` text to glyphs, or every
//! cell and line at fault.

use std::collections::BTreeMap;

use gbcli_pixels::{Raster, Rgba};

use super::GRID_WIDTH;
use crate::chars::{GRID_COLUMNS, Slot, named, read_chars};
use crate::key::Key;
use crate::model::{Glyph, HEIGHT};
use crate::problem::Problem;

const EMPTY_REMEDY: &str = "draw it (a space is columns of opaque paper), or write <> in its \
                            place in the .chars file to leave the cell empty";

/// The glyphs `image` and `chars` hold, in cell order, or every problem.
pub fn from_grid(image: &Raster, chars: &str) -> Result<Vec<Glyph>, Vec<Problem>> {
    let mut problems = Vec::new();
    if image.width != GRID_WIDTH || !image.height.is_multiple_of(8) || image.height == 0 {
        problems.push(Problem::new(
            format!(
                "the image is {}x{}; a font's grid is {GRID_WIDTH} pixels wide (16 cells of 8x8) \
                 and a whole number of 8-pixel rows tall",
                image.width, image.height
            ),
            "keep the grid's size: add or remove whole rows of cells at the bottom",
        ));
        return Err(problems);
    }
    let rows = read_chars(chars)?;
    let cell_rows = (image.height / 8) as usize;
    let mut slots: Vec<Slot> = vec![None; cell_rows * GRID_COLUMNS];
    let mut seen: BTreeMap<Key, (u32, u32)> = BTreeMap::new();
    for (r, row) in rows.iter().enumerate() {
        if row.len() > GRID_COLUMNS {
            problems.push(
                Problem::new(
                    format!(
                        "this line lists {} cells; a row of the grid has 16",
                        row.len()
                    ),
                    "move the extra characters to the next line, under a new row of cells",
                )
                .at_line(r + 1, Some(GRID_COLUMNS + 1)),
            );
        }
        if r >= cell_rows && row.iter().any(Option::is_some) {
            problems.push(
                Problem::new(
                    format!(
                        "this line names cells in row {r}, but the image has {cell_rows} rows of \
                         cells"
                    ),
                    "add a row of cells to the image for them, or remove the line",
                )
                .at_line(r + 1, None),
            );
            continue;
        }
        for (i, slot) in row.iter().enumerate().take(GRID_COLUMNS) {
            let Some(key) = slot else { continue };
            let cell = (i as u32, r as u32);
            if let Some(first) = seen.get(key) {
                problems.push(
                    Problem::new(
                        format!(
                            "{} is listed twice: here and at cell {},{}",
                            named(key),
                            first.0,
                            first.1
                        ),
                        "keep one; each glyph has one cell",
                    )
                    .at_cell(cell.0, cell.1)
                    .at_line(r + 1, Some(i + 1))
                    .of(key),
                );
                continue;
            }
            seen.insert(key.clone(), cell);
            slots[r * GRID_COLUMNS + i] = Some(key.clone());
        }
    }
    let (ink, third) = colors(image);
    if let Some((x, y)) = third {
        problems.push(
            Problem::new(
                format!("pixel {x},{y} is a third opaque color; a font is two, ink and paper"),
                "draw every opaque pixel in the darker color (ink) or the lighter (paper)",
            )
            .at_cell(x / 8, y / 8),
        );
    }
    let mut glyphs = Vec::new();
    for (n, slot) in slots.iter().enumerate() {
        let (cx, cy) = ((n % GRID_COLUMNS) as u32, (n / GRID_COLUMNS) as u32);
        let at = |x: u32, y: u32| image.rgba[((cy * 8 + y) * image.width + cx * 8 + x) as usize];
        let opaque = |x: u32, y: u32| at(x, y)[3] == 0xFF;
        let width = (0..8u32)
            .rev()
            .find(|&x| (0..8).any(|y| opaque(x, y)))
            .map_or(0, |x| x + 1);
        let Some(key) = slot else {
            if width > 0 {
                problems.push(
                    Problem::new(
                        format!("cell {cx},{cy} is drawn but has no character in the .chars file"),
                        "name it on the .chars line for its row, or clear the cell",
                    )
                    .at_cell(cx, cy),
                );
            }
            continue;
        };
        if width == 0 {
            problems.push(
                Problem::new(
                    format!(
                        "{} is listed over cell {cx},{cy}, which is empty (only clear and guide \
                         pixels)",
                        named(key)
                    ),
                    EMPTY_REMEDY,
                )
                .at_cell(cx, cy)
                .of(key),
            );
            continue;
        }
        let hole = (0..HEIGHT as u32)
            .flat_map(|y| (0..width).map(move |x| (x, y)))
            .find(|&(x, y)| !opaque(x, y));
        if let Some((x, y)) = hole {
            problems.push(
                Problem::new(
                    format!(
                        "{}'s cell {cx},{cy} has a clear pixel at {x},{y}, left of its last \
                         opaque column ({}): a glyph is opaque ink or paper across its width",
                        named(key),
                        width - 1
                    ),
                    "paint it ink or paper; only the columns past a glyph's width are clear",
                )
                .at_cell(cx, cy)
                .of(key),
            );
            continue;
        }
        let mut rows = [0u8; HEIGHT];
        for (y, row) in rows.iter_mut().enumerate() {
            for x in 0..width {
                if Some(at(x, y as u32)) == ink {
                    *row |= 0x80 >> x;
                }
            }
        }
        glyphs.push(Glyph {
            comment: Vec::new(),
            key: key.clone(),
            width: width as u8,
            rows,
        });
    }
    if problems.is_empty() {
        Ok(glyphs)
    } else {
        Err(problems)
    }
}

/// The image's ink color (the darker of its two opaque colors, or its one
/// when that is dark), and where a third opaque color first appears.
fn colors(image: &Raster) -> (Option<Rgba>, Option<(u32, u32)>) {
    let mut found: Vec<Rgba> = Vec::new();
    let mut third = None;
    for (i, &p) in image.rgba.iter().enumerate() {
        if p[3] != 0xFF || found.contains(&p) {
            continue;
        }
        if found.len() == 2 {
            third.get_or_insert((i as u32 % image.width, i as u32 / image.width));
            continue;
        }
        found.push(p);
    }
    let luma = |p: &Rgba| 2126 * u32::from(p[0]) + 7152 * u32::from(p[1]) + 722 * u32::from(p[2]);
    let ink = match found.as_slice() {
        [one] if luma(one) < 10_000 * 128 => Some(*one),
        [a, b] => Some(if luma(a) <= luma(b) { *a } else { *b }),
        _ => None,
    };
    (ink, third)
}
