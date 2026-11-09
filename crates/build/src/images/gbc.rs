//! Color art's GBC tile data under `build --gbc` (the joint conversion,
//! plan question 1, leaf 0racny). The image's tiles were deduplicated on
//! the pair (DMG tile, GBC tile) when it was checked (`bg::check`,
//! `sheet::check`), so one tilemap or frame list serves both machines; here
//! each color tile is indexed into the palette its cell was packed into
//! (`pack`), and the attribute map names that slot (`output`).
//!
//! rgbgfx drew this data until 2026-11-09; it is now the differential test's
//! oracle only (`tests/it/images_differential.rs`), which is why nothing
//! here compares palettes any more: gbcli's own slot is the one each tile
//! uses.

use gbcli_protocol::{GbcPalette, PaletteSlot, Rgb555, TileAt};

use super::bg::BgPlan;
use super::encode;
use super::pack::Layer;
use super::pixels::Pixels;
use super::sheet::SheetPlan;

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

/// A color background's GBC version: each of `plan`'s unique tiles in its
/// first cell's slot (`slots`, one per cell).
pub(crate) fn background(
    plan: &BgPlan,
    color: &Pixels,
    p: &Palettes,
    slots: &[Option<PaletteSlot>],
) -> Joint {
    let mut firsts = vec![None; plan.unique.len()];
    for (cell, &unique) in plan.cells.iter().enumerate() {
        firsts[unique].get_or_insert(cell);
    }
    let mut joint = Joint {
        tiles: Vec::with_capacity(plan.unique.len() * 16),
        slots: Vec::with_capacity(plan.unique.len()),
    };
    for cell in firsts {
        let cell = cell.expect("every unique tile has a cell");
        let (x, y) = (cell as u32 % plan.width, cell as u32 / plan.width);
        let slot = slot(slots, cell);
        joint.tiles.extend(tile(color, p, slot, TileAt { x, y }));
        joint.slots.push(slot);
    }
    joint
}

/// A color sheet's GBC version: each of `plan`'s kept units in its slot
/// (`slots`, one per cell of the sheet, as `pack` gives them).
pub(crate) fn sheet(
    plan: &SheetPlan,
    color: &Pixels,
    p: &Palettes,
    slots: &[Option<PaletteSlot>],
) -> Joint {
    let per = plan.object.tiles();
    let mut joint = Joint {
        tiles: Vec::with_capacity(plan.kept.len() * 16 * per as usize),
        slots: Vec::with_capacity(plan.kept.len()),
    };
    for &(ux, uy) in &plan.kept {
        let (x, y) = (ux / 8, uy / 8);
        let cell = (y / per * plan.width + x) as usize;
        let slot = slot(slots, cell);
        for t in 0..per {
            joint
                .tiles
                .extend(tile(color, p, slot, TileAt { x, y: y + t }));
        }
        joint.slots.push(slot);
    }
    joint
}

/// `slots[cell]`: the packing gives every cell with an opaque pixel one, and
/// the checks refused an image whose kept tile has none.
fn slot(slots: &[Option<PaletteSlot>], cell: usize) -> PaletteSlot {
    slots[cell].expect("the packing gives every opaque cell a slot")
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

/// Tile `at`'s GBC data in `slot`: each pixel's color's index in the slot's
/// palette, 8 a row.
fn tile(color: &Pixels, p: &Palettes, slot: PaletteSlot, at: TileAt) -> Vec<u8> {
    let index = usize::from(
        slot.get()
            .checked_sub(p.first_slot)
            .expect("a slot from the packing's first"),
    );
    let palette = &p.palettes[index];
    let mut indices = [0u8; 64];
    for n in 0..64 {
        let (x, y) = (n % 8, n / 8);
        let want = planned(color, at.x * 8 + x, at.y * 8 + y);
        indices[n as usize] = (0..4u8)
            .find(|&k| drawn(palette, p.layer, k) == want)
            .expect("the packed palette holds every color of its cell");
    }
    encode::encode(&indices)
}

#[cfg(test)]
mod tests;
