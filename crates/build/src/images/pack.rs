//! GBC palettes for color art, packed by gbcli (the GBC images plan, leaf
//! `pack`; research §3): which tiles force a ninth palette isn't unique, so
//! gbcli packs deterministically and indexes every tile into its result
//! (`gbc`).
//!
//! The census takes each tile's (or object's) set of RGB555 colors; the
//! packer drops duplicate sets and sets inside others, packs the rest first
//! fit decreasing, and only when that overflows the budget runs a small
//! bounded search for a packing that fits.

mod problems;

use std::collections::BTreeSet;

use gbcli_protocol::{GbcImage, GbcPalette, ImageArt, ImageProblem, PaletteSlot, Rgb555, TileAt};

use super::pixels::Pixels;
use problems::{Overflow, too_many_colors};

/// The GBC's palettes of each kind: 8 BG and 8 OBJ.
pub(crate) const PALETTES: usize = 8;

/// How many placements the search may try before giving up. A packing
/// that fits is usually found in a few hundred; a hopeless one is cut off
/// here, and first fit's overflow is reported.
const SEARCH_STEPS: u32 = 20_000;

/// A set of colors: one cell's, or one palette's.
type Colors = BTreeSet<Rgb555>;

/// Which palettes the colors go into.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Layer {
    /// Background tiles, 8x8, with 4 colors a palette.
    Bg,
    /// Objects, 8x8 or 8x16, with 3 opaque colors a palette (index 0 is
    /// transparent).
    Obj,
}

impl Layer {
    /// The colors one palette holds for this layer.
    pub fn colors(self) -> usize {
        match self {
            Layer::Bg => 4,
            Layer::Obj => 3,
        }
    }

    fn cell(self) -> &'static str {
        match self {
            Layer::Bg => "tile",
            Layer::Obj => "object",
        }
    }

    fn kind(self) -> &'static str {
        match self {
            Layer::Bg => "BG",
            Layer::Obj => "OBJ",
        }
    }
}

/// One image to pack: its path (for problems), pixels, and the height of
/// a cell in pixels: 8 for a background's tile, its objects' height for a
/// sheet.
pub(crate) struct Source<'a> {
    pub image: &'a str,
    pub pixels: &'a Pixels,
    pub cell_height: u32,
}

/// Palettes that fit, in slot order, and the slot of every cell.
#[derive(Debug, PartialEq, Eq)]
pub(crate) struct Packing {
    /// From slot `first_slot`, as palette RAM takes them. A BG palette's
    /// colors run lightest first, the darkest repeated to fill four; an OBJ
    /// palette's index 0 is never drawn, so it holds white, and its colors
    /// follow lightest first.
    pub palettes: Vec<GbcPalette>,
    /// The slot of `palettes[0]`: the reserved slots come first (the
    /// assigned slots of indexed sheets, `GbcImagesMade::obj_palettes`).
    pub first_slot: u8,
    /// Per source, the slot of each cell (tile or object, rows top to
    /// bottom); none for an object with no opaque pixel. The joint
    /// conversion puts each in its `TileEntry`.
    pub cell_palettes: Vec<Vec<Option<PaletteSlot>>>,
}

impl Packing {
    /// Source `source`'s GBC version as color art. A background lists its
    /// palettes; a sheet's are the shared OBJ palettes
    /// (`GbcImagesMade::obj_palettes`, this packing's `palettes` after the
    /// reserved ones), so it lists none. Each cell's slot is in
    /// `cell_palettes`.
    pub fn gbc_image(&self, layer: Layer) -> GbcImage {
        GbcImage {
            preview: String::new(),
            art: ImageArt::Color,
            palettes: match layer {
                Layer::Bg => self.palettes.clone(),
                Layer::Obj => Vec::new(),
            },
        }
    }
}

/// Each cell's opaque colors, cells `tall` pixels high, rows top to
/// bottom, left to right in each; none for a cell with no opaque pixel.
pub(crate) fn census(pixels: &Pixels, tall: u32) -> Vec<Option<Colors>> {
    let (w, h) = (pixels.width, pixels.height);
    let mut cells = Vec::new();
    for cy in (0..h).step_by(tall as usize) {
        for cx in (0..w).step_by(8) {
            let colors: Colors = (cy..(cy + tall).min(h))
                .flat_map(|y| (cx..(cx + 8).min(w)).map(move |x| (x, y)))
                .map(|(x, y)| pixels.at(x, y))
                .filter(|p| p[3] != 0)
                .map(|p| Rgb555::from_rgb888(p[0], p[1], p[2]))
                .collect();
            cells.push((!colors.is_empty()).then_some(colors));
        }
    }
    cells
}

/// A cell of one source: where it is and its colors.
struct Cell {
    source: usize,
    at: TileAt,
    colors: Colors,
}

/// Packs the colors of `sources` into palettes of `layer`, at most 8 less
/// `reserved`, the slots already taken (the assigned slots of indexed
/// sheets, one per palette and OBP, `indexed::ObjSlots::reserved`): those are slots 0 to `reserved - 1`, so
/// the packed palettes take the slots after them.
/// The problems are `gbc_tile_colors` (a cell over a palette's colors,
/// which stops the packing) or `gbc_palettes` (one per image with cells
/// that landed past the budget).
pub(crate) fn pack(
    sources: &[Source],
    layer: Layer,
    reserved: usize,
) -> Result<Packing, Vec<ImageProblem>> {
    let cap = layer.colors();
    let per_source: Vec<Vec<Option<Colors>>> = sources
        .iter()
        .map(|s| census(s.pixels, s.cell_height))
        .collect();
    let mut cells = Vec::new();
    for (source, (s, found)) in sources.iter().zip(&per_source).enumerate() {
        let columns = s.pixels.width.div_ceil(8).max(1);
        for (i, colors) in found.iter().enumerate() {
            let Some(colors) = colors else { continue };
            let (col, row) = (i as u32 % columns, i as u32 / columns);
            let at = TileAt {
                x: col,
                y: row * s.cell_height / 8,
            };
            cells.push(Cell {
                source,
                at,
                colors: colors.clone(),
            });
        }
    }
    let crowded = too_many_colors(sources, &cells, layer);
    if !crowded.is_empty() {
        return Err(crowded);
    }

    let sets = maximal_sets(&cells);
    let colors = sets.iter().flatten().collect::<BTreeSet<_>>().len();
    let bound = colors.div_ceil(cap);
    let budget = PALETTES.saturating_sub(reserved);
    let first = first_fit(&sets, cap);
    let palettes = if first.len() <= budget {
        first
    } else if let Some(found) = (bound <= budget)
        .then(|| search(&sets, cap, budget))
        .flatten()
    {
        found
    } else {
        let over = Overflow {
            layer,
            reserved,
            colors,
            bound,
            placed: first.len(),
        };
        return Err(over.problems(sources, &cells, &first[..budget]));
    };

    // The budget keeps reserved + placed within 8, so every slot is one.
    let first_slot = reserved.min(PALETTES) as u8;
    let slot = |set: &Colors| {
        palettes
            .iter()
            .position(|p| set.is_subset(p))
            .and_then(|i| PaletteSlot::new(first_slot + i as u8))
    };
    let cell_palettes = per_source
        .iter()
        .map(|found| found.iter().map(|c| c.as_ref().and_then(slot)).collect())
        .collect();
    Ok(Packing {
        palettes: palettes.iter().map(|p| palette(p, layer)).collect(),
        first_slot,
        cell_palettes,
    })
}

/// The distinct color sets that no other set contains, largest first, ties
/// in set order: first fit decreasing's order, fixed by the colors alone.
fn maximal_sets(cells: &[Cell]) -> Vec<Colors> {
    let distinct: BTreeSet<&Colors> = cells.iter().map(|c| &c.colors).collect();
    let mut kept: Vec<Colors> = distinct
        .iter()
        .filter(|s| !distinct.iter().any(|t| t != *s && s.is_subset(t)))
        .map(|s| (*s).clone())
        .collect();
    kept.sort_by(|a, b| b.len().cmp(&a.len()).then_with(|| a.cmp(b)));
    kept
}

/// Each set into the first palette it fits, or a new one.
fn first_fit(sets: &[Colors], cap: usize) -> Vec<Colors> {
    let mut palettes: Vec<Colors> = Vec::new();
    for set in sets {
        match palettes.iter_mut().find(|p| p.union(set).count() <= cap) {
            Some(p) => p.extend(set),
            None => palettes.push(set.clone()),
        }
    }
    palettes
}

/// A packing into at most `target` palettes by depth-first search, in a
/// fixed order, within `SEARCH_STEPS`; none if it finds none in time.
fn search(sets: &[Colors], cap: usize, target: usize) -> Option<Vec<Colors>> {
    fn place(
        sets: &[Colors],
        palettes: &mut Vec<Colors>,
        cap: usize,
        target: usize,
        steps: &mut u32,
    ) -> bool {
        let Some((set, rest)) = sets.split_first() else {
            return true;
        };
        if *steps == 0 {
            return false;
        }
        *steps -= 1;
        // A set some palette already holds goes nowhere better.
        if palettes.iter().any(|p| set.is_subset(p)) {
            return place(rest, palettes, cap, target, steps);
        }
        for i in 0..palettes.len() {
            let added: Vec<Rgb555> = set.difference(&palettes[i]).copied().collect();
            if palettes[i].len() + added.len() > cap {
                continue;
            }
            palettes[i].extend(&added);
            if place(rest, palettes, cap, target, steps) {
                return true;
            }
            for c in &added {
                palettes[i].remove(c);
            }
        }
        if palettes.len() < target {
            palettes.push(set.clone());
            if place(rest, palettes, cap, target, steps) {
                return true;
            }
            palettes.pop();
        }
        false
    }
    let mut palettes = Vec::new();
    let mut steps = SEARCH_STEPS;
    place(sets, &mut palettes, cap, target, &mut steps).then_some(palettes)
}

/// A packed palette in slot order: lightest first by rgbgfx's weights, so
/// the order is fixed by the colors; an OBJ palette's index 0 is white.
fn palette(colors: &Colors, layer: Layer) -> GbcPalette {
    let lightness = |c: &Rgb555| {
        let [r, g, b] = c.channels().map(u32::from);
        2126 * r + 7152 * g + 722 * b
    };
    let mut sorted: Vec<Rgb555> = colors.iter().copied().collect();
    sorted.sort_by(|a, b| lightness(b).cmp(&lightness(a)).then(a.cmp(b)));
    let mut slots = Vec::with_capacity(4);
    if layer == Layer::Obj {
        slots.push(Rgb555::WHITE);
    }
    slots.extend(&sorted);
    let last = *slots.last().unwrap_or(&Rgb555::BLACK);
    slots.resize(4, last);
    GbcPalette([slots[0], slots[1], slots[2], slots[3]])
}

#[cfg(test)]
mod tests;
