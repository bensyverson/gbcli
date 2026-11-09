//! Color art's GBC tile data under `build --gbc` (the joint conversion,
//! plan question 1, leaf 0racny). The image's tiles were deduplicated on
//! the pair (DMG tile, GBC tile) when it was checked (`bg::check`,
//! `sheet::check`), so one tilemap or frame list serves both machines;
//! here rgbgfx converts the color image with gbcli's packed palettes
//! (`pack`, as `-c` in slot order) and an attribute map, without `-u`, and
//! gbcli keeps one tile per joint tile, in the DMG data's order.
//!
//! Every pixel rgbgfx drew is checked against the image's color first
//! (`agree`): by color, not by palette number, since rgbgfx may draw a tile
//! from another given palette that holds its colors (research §3), and it
//! chooses per 8x8 tile, so an 8x16 unit's halves can land in two. Each
//! joint tile takes the slot gbcli's packing gave its cell; where rgbgfx
//! chose that palette its bytes are kept, and elsewhere the tile is
//! indexed into gbcli's palette from the colors just checked equal.

use std::fs;
use std::path::Path;

use gbcli_protocol::{GbcPalette, ImageProblem, PaletteSlot, Rgb555, TileAt};

use super::agree::{self, ColorDisagreement};
use super::bg::BgPlan;
use super::pack::{self, Layer};
use super::pixels::Pixels;
use super::run::Rgbgfx;
use super::sheet::SheetPlan;
use crate::project::BuildError;

/// The packed palettes an image's GBC version draws from.
pub(crate) struct Palettes<'a> {
    /// In slot order from `first_slot`: a background's own BG palettes, or
    /// the packed OBJ palettes every color sheet shares.
    pub palettes: &'a [GbcPalette],
    pub first_slot: u8,
    pub layer: Layer,
}

/// A GBC version in the joint order: its tile data (2bpp) and the slot of
/// each joint tile (a background's) or kept unit (a sheet's).
pub(crate) struct Joint {
    pub tiles: Vec<u8>,
    pub slots: Vec<PaletteSlot>,
}

/// rgbgfx's GBC data as it wrote it: tiles in its order, an attribute
/// byte each (the palette in bits 0-2).
pub(crate) struct Raw {
    pub tiles: Vec<u8>,
    pub attrs: Vec<u8>,
}

/// An image's size in tiles, and the order rgbgfx wrote them: row by row,
/// or a sheet's (`-Z`) column by column, which keeps a unit's pair
/// adjacent.
#[derive(Clone, Copy)]
pub(crate) struct Grid {
    pub width: u32,
    pub height: u32,
}

impl Grid {
    fn at(self, layer: Layer, x: u32, y: u32) -> usize {
        match layer {
            Layer::Bg => (y * self.width + x) as usize,
            Layer::Obj => (x * self.height + y) as usize,
        }
    }
}

/// A color background's GBC version: each of `plan`'s unique tiles in its
/// first cell's slot (`slots`, one per cell). `None` when rgbgfx failed or
/// disagreed (its diagnostics or problems say why).
#[allow(clippy::too_many_arguments)]
pub(crate) fn background(
    rgbgfx: &mut Rgbgfx,
    image: &str,
    source: &Path,
    plan: &BgPlan,
    color: &Pixels,
    p: &Palettes,
    slots: &[Option<PaletteSlot>],
    out: &str,
) -> Result<Option<Joint>, BuildError> {
    let grid = Grid {
        width: plan.width,
        height: plan.height,
    };
    let Some(raw) = convert(rgbgfx, image, source, p, grid, out)? else {
        return Ok(None);
    };
    let mut firsts = vec![None; plan.unique.len()];
    for (cell, &u) in plan.cells.iter().enumerate() {
        firsts[u].get_or_insert(cell);
    }
    let cells = firsts
        .into_iter()
        .map(|c| c.expect("every unique tile has a cell"));
    let places = cells.map(|c| {
        let (x, y) = (c as u32 % plan.width, c as u32 / plan.width);
        (vec![TileAt { x, y }], slots[c])
    });
    joint(rgbgfx, image, color, p, grid, &raw, places)
}

/// A color sheet's GBC version: each of `plan`'s kept units in its slot
/// (`slots`, one per unit of the sheet, as `pack` gives them).
#[allow(clippy::too_many_arguments)]
pub(crate) fn sheet(
    rgbgfx: &mut Rgbgfx,
    image: &str,
    source: &Path,
    plan: &SheetPlan,
    color: &Pixels,
    p: &Palettes,
    slots: &[Option<PaletteSlot>],
    out: &str,
) -> Result<Option<Joint>, BuildError> {
    if plan.kept.is_empty() {
        return Ok(Some(Joint {
            tiles: Vec::new(),
            slots: Vec::new(),
        }));
    }
    let grid = Grid {
        width: plan.width,
        height: plan.height,
    };
    let Some(raw) = convert(rgbgfx, image, source, p, grid, out)? else {
        return Ok(None);
    };
    let per = plan.object.tiles();
    let places = plan.kept.iter().map(|&(ux, uy)| {
        let (x, y) = (ux / 8, uy / 8);
        let unit = (y / per * plan.width + x) as usize;
        let tiles = (0..per).map(|t| TileAt { x, y: y + t }).collect();
        (tiles, slots[unit])
    });
    joint(rgbgfx, image, color, p, grid, &raw, places)
}

/// Runs rgbgfx on the color image and reads what it wrote.
fn convert(
    rgbgfx: &mut Rgbgfx,
    image: &str,
    source: &Path,
    p: &Palettes,
    grid: Grid,
    out: &str,
) -> Result<Option<Raw>, BuildError> {
    let (tiles, attrs) = (format!("{out}.gbc.raw.2bpp"), format!("{out}.gbc.attrmap"));
    let spec = pack::spec(p.palettes, p.layer);
    let mut args = vec!["-c", &spec, "-o", &tiles, "-a", &attrs];
    if p.layer == Layer::Obj {
        args.insert(0, "-Z");
    }
    let ran = rgbgfx.run(image, &args, source)?;
    let read = |rel: &str| {
        let read = ran.then(|| rgbgfx.read(rel)).transpose();
        let _ = fs::remove_file(rgbgfx.root.join(rel));
        read
    };
    let (Some(tiles), Some(attrs)) = (read(&tiles)?, read(&attrs)?) else {
        return Ok(None);
    };
    let n = (grid.width * grid.height) as usize;
    if (tiles.len(), attrs.len()) != (n * 16, n) {
        rgbgfx.error(
            image,
            format!(
                "rgbgfx wrote {} GBC tiles and {} attribute bytes for a {}x{}-tile image",
                tiles.len() / 16,
                attrs.len(),
                grid.width,
                grid.height
            ),
        );
        return Ok(None);
    }
    Ok(Some(Raw { tiles, attrs }))
}

/// Checks `raw` against the image, then keeps each joint tile: `places`
/// gives its tiles (a unit's two) and its slot.
fn joint(
    rgbgfx: &mut Rgbgfx,
    image: &str,
    color: &Pixels,
    p: &Palettes,
    grid: Grid,
    raw: &Raw,
    places: impl Iterator<Item = (Vec<TileAt>, Option<PaletteSlot>)>,
) -> Result<Option<Joint>, BuildError> {
    if let Err(problem) = check(image, color, p, grid, raw) {
        rgbgfx.problems.push(problem);
        return Ok(None);
    }
    let mut kept = Joint {
        tiles: Vec::new(),
        slots: Vec::new(),
    };
    let mut off = Vec::new();
    for (tiles, slot) in places {
        let Some(slot) = slot else {
            // The packing gives every opaque cell a slot; one without is
            // gbcli's mistake, stopped like any disagreement.
            off.push(missing(tiles[0], None));
            continue;
        };
        for at in tiles {
            match tile(color, p, grid, raw, at, slot) {
                Ok(bytes) => kept.tiles.extend(bytes),
                Err(d) => off.push(d),
            }
        }
        kept.slots.push(slot);
    }
    if !off.is_empty() {
        rgbgfx.problems.push(agree::color_problem(image, &off));
        return Ok(None);
    }
    Ok(Some(kept))
}

/// The disagreement for a tile gbcli can't index: its first pixel.
fn missing(tile: TileAt, planned: Option<Rgb555>) -> ColorDisagreement {
    ColorDisagreement {
        tile,
        x: 0,
        y: 0,
        planned,
        wrote: None,
    }
}

/// Pixel (`x`, `y`) of the image as the GBC draws it: `None` where it is
/// transparent.
fn planned(color: &Pixels, x: u32, y: u32) -> Option<Rgb555> {
    let c = color.at(x, y);
    (c[3] != 0).then(|| Rgb555::from_rgb888(c[0], c[1], c[2]))
}

/// The color index `i` of `palette` draws; an object's index 0 is
/// transparent.
fn drawn(palette: &GbcPalette, layer: Layer, i: u8) -> Option<Rgb555> {
    match (layer, i) {
        (Layer::Obj, 0) => None,
        _ => Some(palette.0[usize::from(i)]),
    }
}

/// `rgbgfx_agrees` when any pixel of `raw` draws another color than
/// `color` holds there: one disagreement per tile, its first pixel.
pub(crate) fn check(
    image: &str,
    color: &Pixels,
    p: &Palettes,
    grid: Grid,
    raw: &Raw,
) -> Result<(), ImageProblem> {
    let mut off = Vec::new();
    for ty in 0..grid.height {
        for tx in 0..grid.width {
            let i = grid.at(p.layer, tx, ty);
            let tile = TileAt { x: tx, y: ty };
            let palette = p.palettes.get(usize::from(raw.attrs[i] & 7));
            let indices = agree::indices(&raw.tiles[i * 16..i * 16 + 16]);
            let first = (0..64u32).find_map(|n| {
                let (x, y) = (n % 8, n / 8);
                let planned = planned(color, tx * 8 + x, ty * 8 + y);
                let wrote = palette.and_then(|pal| drawn(pal, p.layer, indices[n as usize]));
                let differs = planned != wrote || palette.is_none();
                differs.then_some(ColorDisagreement {
                    tile,
                    x,
                    y,
                    planned,
                    wrote,
                })
            });
            off.extend(first);
        }
    }
    if off.is_empty() {
        Ok(())
    } else {
        Err(agree::color_problem(image, &off))
    }
}

/// Tile `at`'s GBC data in `slot`: rgbgfx's bytes when it chose that
/// palette, else the tile indexed into it from the image's colors.
fn tile(
    color: &Pixels,
    p: &Palettes,
    grid: Grid,
    raw: &Raw,
    at: TileAt,
    slot: PaletteSlot,
) -> Result<Vec<u8>, ColorDisagreement> {
    let i = grid.at(p.layer, at.x, at.y);
    let chosen = slot.get().checked_sub(p.first_slot);
    let palette = chosen.and_then(|c| p.palettes.get(usize::from(c)));
    let Some(palette) = palette else {
        return Err(missing(at, planned(color, at.x * 8, at.y * 8)));
    };
    if chosen == Some(raw.attrs[i] & 7) {
        return Ok(raw.tiles[i * 16..i * 16 + 16].to_vec());
    }
    let mut indices = Vec::with_capacity(64);
    for n in 0..64u32 {
        let (x, y) = (n % 8, n / 8);
        let want = planned(color, at.x * 8 + x, at.y * 8 + y);
        let found = (0..4u8).find(|&k| drawn(palette, p.layer, k) == want);
        match found {
            Some(k) => indices.push(k),
            None => {
                return Err(ColorDisagreement {
                    tile: at,
                    x,
                    y,
                    planned: want,
                    wrote: None,
                });
            }
        }
    }
    Ok(agree::encode(&indices))
}

#[cfg(test)]
mod tests;
