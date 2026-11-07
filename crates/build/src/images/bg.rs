//! A background on the DMG: one palette of four shades for the whole
//! screen, and at most 256 unique tiles for one map to address.

use std::collections::HashMap;

use gbcli_protocol::{ImageLimit, ImageProblem, TileAt};

use super::census::{self, Key, error, key};
use super::pixels::{Pixels, Rgba, hex};
use super::shades::Shading;
use super::shared;

/// Tiles one background map addresses (Pan Docs, Tile Maps).
pub(crate) const MAP_TILES: usize = 256;
/// Shades in BGP.
const BG_SHADES: usize = 4;

/// A background that fits, ready for rgbgfx.
#[derive(Debug)]
pub(crate) struct BgPlan {
    /// rgbgfx's `-c`: each color at the index of its shade, so BGP $E4
    /// shows it as drawn. Colors that share a shade are one color to
    /// rgbgfx, the most used of them (`merges`).
    pub spec: String,
    /// The colors rgbgfx must see as another (`shared`): none unless two
    /// colors share a shade.
    pub merges: Vec<(Key, Rgba)>,
    /// Problems that don't stop the build.
    pub warnings: Vec<ImageProblem>,
    /// Size in tiles.
    pub width: u32,
    pub height: u32,
    /// Unique tiles: what rgbgfx `-u` keeps.
    pub tiles: u32,
    /// Each unique tile's color indices, 8 a row, in order of first use.
    pub unique: Vec<Vec<u8>>,
    /// Each map cell's unique tile, row by row: what rgbgfx's data must
    /// draw there (`agree`).
    pub cells: Vec<usize>,
}

/// Checks a background; the plan when it fits, else every problem found.
/// `shading` is the project's (`shades::choose`).
pub(crate) fn check(
    image: &str,
    pixels: &Pixels,
    shading: &Shading,
) -> Result<BgPlan, Vec<ImageProblem>> {
    if let Some(p) = census::size(image, pixels, 8) {
        return Err(vec![p]);
    }
    if let Some(p) = census::alpha(image, pixels, false) {
        return Err(vec![p]);
    }
    let colors = census::colors(pixels);
    if colors.len() > BG_SHADES {
        return Err(vec![census::too_many_colors(
            image,
            pixels,
            &colors,
            BG_SHADES,
            "the DMG background has one palette of 4 shades for the whole screen",
            "redraw it in at most 4 colors (4 grays: #ffffff, #aaaaaa, #555555, #000000); \
             start with the tiles listed",
        )]);
    }
    let list: Vec<Rgba> = colors.iter().map(|(c, _)| *c).collect();
    let mut slots = [
        "#none".to_string(),
        "#none".into(),
        "#none".into(),
        "#none".into(),
    ];
    let shade_of = shading.shades(&list);
    let mut shared = shared::find(image, pixels, &colors, &shade_of, shading.choice);
    shared
        .warnings
        .extend(shared::stretched(image, pixels, &colors, shading));
    let mut shade: HashMap<Key, u8> = HashMap::new();
    for (c, &s) in list.iter().zip(&shade_of) {
        if !shared.merges.iter().any(|(k, _)| *k == key(*c)) {
            slots[usize::from(s)] = hex(*c);
        }
        shade.insert(key(*c), s);
    }
    let warned = |p: ImageProblem| {
        let mut all = vec![p];
        all.extend(shared.warnings.iter().cloned());
        all
    };
    let (width, height) = (pixels.width / 8, pixels.height / 8);
    // Each unique tile's first place and how often it is used.
    let mut seen: HashMap<Vec<u8>, (TileAt, u32)> = HashMap::new();
    let mut order: Vec<Vec<u8>> = Vec::new();
    let mut number: HashMap<Vec<u8>, usize> = HashMap::new();
    let mut cells = Vec::with_capacity((width * height) as usize);
    for ty in 0..height {
        for tx in 0..width {
            let tile: Vec<u8> = (0..64)
                .map(|i| shade[&key(pixels.at(tx * 8 + i % 8, ty * 8 + i / 8))])
                .collect();
            let next = number.len();
            cells.push(*number.entry(tile.clone()).or_insert(next));
            let entry = seen.entry(tile.clone()).or_insert_with(|| {
                order.push(tile);
                (TileAt { x: tx, y: ty }, 0)
            });
            entry.1 += 1;
        }
    }
    let unique = order.len();
    if unique > MAP_TILES {
        let once: Vec<TileAt> = order
            .iter()
            .map(|t| seen[t])
            .filter(|&(_, n)| n == 1)
            .map(|(at, _)| at)
            .collect();
        let p = error(
            image,
            ImageLimit::BgTilesMap,
            unique,
            MAP_TILES,
            format!(
                "{image} needs {unique} unique tiles; one map addresses {MAP_TILES}. {} of them \
                 are used once (listed)",
                once.len()
            ),
            "redraw the tiles used once to match others, simplify detail, or split the image \
             into two backgrounds",
        );
        return Err(warned(ImageProblem { tiles: once, ..p }));
    }
    let map = (width * height) as usize;
    if let Some(p) = census::bank(
        image,
        unique * 16 + map,
        &format!("{unique} tiles of 16 bytes and a map of {map}, one byte a tile"),
        &format!(
            "split it into backgrounds of at most {} tiles each (a 32x32 screen map is 1024) \
             and load them as they scroll into view",
            census::BANK_BYTES - unique * 16
        ),
    ) {
        return Err(warned(p));
    }
    Ok(BgPlan {
        spec: slots.join(","),
        merges: shared.merges,
        warnings: shared.warnings,
        width,
        height,
        tiles: unique as u32,
        unique: order,
        cells,
    })
}
