//! A background on the DMG: one palette of four shades for the whole
//! screen, and at most 256 unique tiles for one map to address.

use std::collections::HashMap;

use gbcli_protocol::{ImageKind, ImageLimit, ImageProblem, TileAt};

use super::census::{self, Key, error, gbc_word, key};
use super::pixels::{Color, Pixels, Rgba, hex};
use super::shades::Shading;
use super::shared;

/// Tiles one background map addresses (Pan Docs, Tile Maps).
pub(crate) const MAP_TILES: usize = 256;
/// Shades in BGP.
const BG_SHADES: usize = 4;

/// A tile as deduplicated: its DMG color indices, and its GBC colors
/// (`census::gbc_word`, none without a GBC version).
type Tile = (Vec<u8>, Vec<u16>);

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
    /// Unique tiles: on the DMG alone, what rgbgfx's dedup would keep;
    /// with a GBC version, the pairs (DMG tile, GBC tile).
    pub tiles: u32,
    /// Each unique tile's DMG color indices, 8 a row, in order of first
    /// use (a DMG tile may repeat, beside two GBC tiles).
    pub unique: Vec<Vec<u8>>,
    /// Each map cell's unique tile, row by row: what rgbgfx's data must
    /// draw there (`agree`).
    pub cells: Vec<usize>,
}

/// Checks a background, or a tileset (`kind`), which is checked alike but
/// keeps no map; the plan when it fits, else every problem found.
/// `shading` is the project's (`shades::choose`). `color` is the color
/// image whose DMG version `pixels` is, under `--gbc`: tiles are then one
/// only when they match in both (plan question 1), never by a flip, since
/// the DMG's tilemap has none; the limits on those joint tiles name it.
pub(crate) fn check(
    image: &str,
    kind: ImageKind,
    pixels: &Pixels,
    shading: &Shading,
    color: Option<Color>,
) -> Result<BgPlan, Vec<ImageProblem>> {
    let tileset = kind == ImageKind::Tiles;
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
    // Each unique tile (its DMG indices, and its GBC colors when it has a
    // GBC version): its first place and how often it is used.
    let mut seen: HashMap<Tile, (TileAt, u32)> = HashMap::new();
    let mut order: Vec<Tile> = Vec::new();
    let mut number: HashMap<Tile, usize> = HashMap::new();
    let mut cells = Vec::with_capacity((width * height) as usize);
    for ty in 0..height {
        for tx in 0..width {
            let at = |i: u32| (tx * 8 + i % 8, ty * 8 + i / 8);
            let dmg: Vec<u8> = (0..64)
                .map(at)
                .map(|(x, y)| shade[&key(pixels.at(x, y))])
                .collect();
            let gbc = color.map_or_else(Vec::new, |c| {
                (0..64)
                    .map(at)
                    .map(|(x, y)| gbc_word(c.pixels.at(x, y)))
                    .collect()
            });
            let tile = (dmg, gbc);
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
        // Joint tiles are the color image's: its DMG version may need one.
        let (named, joint, alike) = match color {
            Some(c) => (
                c.image,
                ", counting tiles alike on the DMG but distinct in color as two",
                " in color too",
            ),
            None => (image, "", ""),
        };
        let p = error(
            named,
            ImageLimit::BgTilesMap,
            unique,
            MAP_TILES,
            format!(
                "{named} needs {unique} unique tiles{joint}; one map addresses {MAP_TILES}. {} \
                 of them are used once (listed)",
                once.len()
            ),
            &format!(
                "redraw the tiles used once to match others{alike}, simplify detail, or split \
                 the image into two {}",
                if tileset { "tilesets" } else { "backgrounds" }
            ),
        );
        return Err(warned(ImageProblem { tiles: once, ..p }));
    }
    let map = (width * height) as usize;
    // A tileset's data is its tiles alone, and under --gbc its section
    // holds a byte per tile (`<Name>Attrs`) where a background's holds an
    // attribute map.
    let (what, gbc_what, rom, gbc_rom) = if tileset {
        (
            format!("{unique} tiles of 16 bytes"),
            format!(
                "its GBC section: {unique} tiles of 16 bytes, an attribute byte a tile and up \
                 to 8 palettes of 8 bytes"
            ),
            unique * 16,
            unique * 17 + 8 * 8,
        )
    } else {
        (
            format!("{unique} tiles of 16 bytes and a map of {map}, one byte a tile"),
            format!(
                "its GBC section: {unique} tiles of 16 bytes, an attribute map of {map} and up \
                 to 8 palettes of 8 bytes"
            ),
            unique * 16 + map,
            unique * 16 + map + 8 * 8,
        )
    };
    let remedy = |spare: usize| {
        if tileset {
            // Unreachable while `MAP_TILES` holds: 256 tiles fit a bank.
            "split it into two tilesets".to_string()
        } else {
            format!(
                "split it into backgrounds of at most {spare} tiles each (a 32x32 screen map is \
                 1024) and load them as they scroll into view"
            )
        }
    };
    if let Some(p) = census::bank(image, rom, &what, &remedy(census::BANK_BYTES - unique * 16)) {
        return Err(warned(p));
    }
    // Color art's GBC section (`emit::gbc`): its own tiles, the attribute
    // bytes, and up to 8 palettes of 8 bytes, before packing says how many.
    if let Some(c) = color
        && let Some(p) = census::bank(
            c.image,
            gbc_rom,
            &gbc_what,
            &remedy(census::BANK_BYTES - unique * 16 - 8 * 8),
        )
    {
        return Err(warned(p));
    }
    Ok(BgPlan {
        spec: slots.join(","),
        merges: shared.merges,
        warnings: shared.warnings,
        width,
        height,
        tiles: unique as u32,
        unique: order.into_iter().map(|(dmg, _)| dmg).collect(),
        cells,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::images::derive::gray_shading;

    /// 128x96 tiles of 256 distinct two-shade tiles: 4096 bytes of tiles
    /// and a 12288-byte map, exactly one bank on the DMG.
    fn full_bank() -> Pixels {
        let (w, h) = (128 * 8, 96 * 8);
        let rgba = (0..w * h)
            .map(|i| {
                let (x, y) = (i % w, i / w);
                let n = ((y / 8) * 128 + x / 8) % 256;
                let bit = (y % 8) * 8 + x % 8;
                if bit < 8 && (n >> bit) & 1 == 1 {
                    [0, 0, 0, 0xFF]
                } else {
                    [0xFF; 4]
                }
            })
            .collect();
        Pixels {
            width: w,
            height: h,
            rgba,
        }
    }

    /// The GBC section of color art holds its tiles again, its attribute
    /// map and up to 8 palettes: one bank of its own, so a background that
    /// just fits on the DMG doesn't on the GBC.
    #[test]
    fn a_color_backgrounds_gbc_section_must_fit_a_bank_too() {
        let pixels = full_bank();
        let shading = gray_shading();
        let plan =
            check("town.png", ImageKind::Bg, &pixels, &shading, None).expect("fits the DMG bank");
        assert_eq!(plan.tiles, 256);
        let color = Color {
            image: "town.png",
            pixels: &pixels,
        };
        let problems =
            check("town.png", ImageKind::Bg, &pixels, &shading, Some(color)).unwrap_err();
        let p = &problems[0];
        assert_eq!(p.limit, ImageLimit::RomxBank);
        assert_eq!(p.count, 4096 + 12288 + 64);
        assert!(p.message.contains("GBC"), "{}", p.message);
    }
}
