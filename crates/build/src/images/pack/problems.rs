//! The packer's problems: a cell over a palette's colors
//! (`gbc_tile_colors`), and cells past the palette budget (`gbc_palettes`).

use std::collections::BTreeMap;

use gbcli_protocol::{ImageLimit, ImageProblem, Rgb555};

use super::{Cell, Colors, Layer, PALETTES, Source};
use crate::images::census;
use crate::images::pixels::{Pixels, Rgba, hex};

/// A `gbc_tile_colors` error per source with cells over a palette's colors.
pub(super) fn too_many_colors(
    sources: &[Source],
    cells: &[Cell],
    layer: Layer,
) -> Vec<ImageProblem> {
    let cap = layer.colors();
    let mut problems = Vec::new();
    for (source, s) in sources.iter().enumerate() {
        let crowded: Vec<&Cell> = cells
            .iter()
            .filter(|c| c.source == source && c.colors.len() > cap)
            .collect();
        let Some(most) = crowded.iter().map(|c| c.colors.len()).max() else {
            continue;
        };
        let (what, remedy) = match layer {
            Layer::Bg => (
                "a GBC BG palette holds 4 colors",
                "redraw each named tile in at most 4 colors",
            ),
            Layer::Obj => (
                "a GBC OBJ palette holds 3 colors, since index 0 is transparent",
                "redraw each named object in at most 3 opaque colors",
            ),
        };
        let named: Vec<String> = crowded
            .iter()
            .map(|c| format!("({},{}) has {}", c.at.x, c.at.y, c.colors.len()))
            .collect();
        let message = format!(
            "{} has {} {}(s) with more than {cap} colors, and {what}: {}",
            s.image,
            crowded.len(),
            layer.cell(),
            named.join(", ")
        );
        problems.push(ImageProblem {
            tiles: crowded.iter().map(|c| c.at).collect(),
            ..census::error(
                s.image,
                ImageLimit::GbcTileColors,
                most,
                cap,
                message,
                remedy,
            )
        });
    }
    problems
}

/// The cells a `gbc_palettes` message names; `tiles` lists every one.
const NAMED: usize = 8;

/// Why first fit's palettes went past the budget, for the problems.
pub(super) struct Overflow {
    pub layer: Layer,
    pub reserved: usize,
    /// Distinct colors across the cells.
    pub colors: usize,
    /// ⌈colors / colors a palette⌉.
    pub bound: usize,
    /// The palettes first fit placed.
    pub placed: usize,
}

impl Overflow {
    /// A `gbc_palettes` error for each source with cells that `fitted`
    /// (first fit's palettes within the budget) doesn't hold. `count` is
    /// the reserved slots plus the palettes placed (never fewer than the
    /// bound, since a palette holds at most `cap` colors).
    pub fn problems(
        &self,
        sources: &[Source],
        cells: &[Cell],
        fitted: &[Colors],
    ) -> Vec<ImageProblem> {
        let Overflow {
            layer,
            reserved,
            colors,
            bound,
            placed,
        } = *self;
        let needed = reserved + placed;
        let assigned = if reserved > 0 {
            format!(" plus {reserved} assigned")
        } else {
            String::new()
        };
        let mut problems = Vec::new();
        for (source, s) in sources.iter().enumerate() {
            let past: Vec<&Cell> = cells
                .iter()
                .filter(|c| c.source == source && !fitted.iter().any(|p| c.colors.is_subset(p)))
                .collect();
            if past.is_empty() {
                continue;
            }
            let shown = representatives(s.pixels);
            let mut named: Vec<String> = past
                .iter()
                .take(NAMED)
                .map(|c| {
                    let hexes: Vec<String> = c.colors.iter().map(|k| shown(*k)).collect();
                    format!("({},{}) {}", c.at.x, c.at.y, hexes.join(" "))
                })
                .collect();
            if past.len() > NAMED {
                named.push(format!("and {} more (listed)", past.len() - NAMED));
            }
            let whose = match layer {
                Layer::Bg => format!("{}'s tiles", s.image),
                Layer::Obj => "the objects of every sheet".to_string(),
            };
            let message = format!(
                "packed, {whose} take {placed} {kind} palettes{assigned}, and the GBC has \
                 {PALETTES}; their {colors} colors need at least {bound} at {cap} a palette. \
                 These {cell}s of {image} landed past the {PALETTES}th: {}",
                named.join(", "),
                cell = layer.cell(),
                kind = layer.kind(),
                cap = layer.colors(),
                image = s.image,
            );
            let remedy = format!(
                "share colors across {cell}s: recolor the named ones with colors other {cell}s \
                 already use, so their sets fit fewer palettes",
                cell = layer.cell(),
            );
            problems.push(ImageProblem {
                tiles: past.iter().map(|c| c.at).collect(),
                ..census::error(
                    s.image,
                    ImageLimit::GbcPalettes,
                    needed,
                    PALETTES,
                    message,
                    &remedy,
                )
            });
        }
        problems
    }
}

/// How a color reads in a message: the source's most used `#rrggbb` that
/// truncates to it, as the other image checks name colors.
fn representatives(pixels: &Pixels) -> impl Fn(Rgb555) -> String {
    let by_key: BTreeMap<Rgb555, Rgba> = census::colors(pixels)
        .into_iter()
        .map(|(c, _)| (Rgb555::from_rgb888(c[0], c[1], c[2]), c))
        .collect();
    move |k| {
        by_key
            .get(&k)
            .map_or_else(|| format!("${:04x}", k.word()), |c| hex(*c))
    }
}
