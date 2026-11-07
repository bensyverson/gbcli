//! A sprite sheet of 8x16 objects on the DMG: three shades plus
//! transparent per object palette, ten objects a line, forty in all, and
//! whole 8x16 units deduplicated with flips (rgbgfx has no 8x16 mode, and
//! its `-u` would split a unit's pair).

use std::collections::HashMap;

use gbcli_protocol::{ImageLimit, ImageProblem, Severity, TileAt};

use super::census::{self, error};
use super::pixels::{Pixels, Rgba, hex};
use super::shades::shades;

/// Objects one line shows, and OAM holds (Pan Docs, OAM).
pub(crate) const PER_LINE: usize = 10;
pub(crate) const TOTAL: usize = 40;
/// Tiles objects address ($8000-$8FFF).
pub(crate) const OBJ_TILES: usize = 256;
/// Opaque shades in an object palette.
const OBJ_SHADES: usize = 3;
/// OAM attribute bits.
pub(crate) const X_FLIP: u8 = 0x20;
pub(crate) const Y_FLIP: u8 = 0x40;

/// One object of a frame.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct Object {
    /// Pixels from the frame's top-left.
    pub y: u32,
    pub x: u32,
    /// Which kept unit; its tiles are 2 * unit and 2 * unit + 1.
    pub unit: usize,
    pub flip: u8,
}

/// A sheet that fits, ready for rgbgfx and the dedup.
#[derive(Debug)]
pub(crate) struct SheetPlan {
    pub spec: String,
    /// OBP: index 0 transparent, 1-3 the sheet's colors, lightest first.
    pub obp: u8,
    /// Size in tiles.
    pub width: u32,
    pub height: u32,
    /// The frame size in pixels.
    pub frame: (u32, u32),
    /// The kept units by their top-left in sheet pixels, in order of first
    /// use, and each one's uses.
    pub kept: Vec<(u32, u32)>,
    pub uses: Vec<u32>,
    pub frames: Vec<Vec<Object>>,
    /// Problems that don't stop the build.
    pub warnings: Vec<ImageProblem>,
}

impl SheetPlan {
    pub fn tiles(&self) -> u32 {
        self.kept.len() as u32 * 2
    }
}

/// Checks a sheet whose frames are `frame` pixels (the whole sheet when
/// `None`; its name, `<name>.WxH.png`, may give another); the plan when it
/// fits, else every problem found.
pub(crate) fn check(
    image: &str,
    stem: &str,
    pixels: &Pixels,
    frame: Option<(u32, u32)>,
) -> Result<SheetPlan, Vec<ImageProblem>> {
    if let Some(p) = census::size(image, pixels, 16) {
        return Err(vec![p]);
    }
    let (fw, fh) = frame.unwrap_or((pixels.width, pixels.height));
    if fw == 0
        || fh == 0
        || fw % 8 != 0
        || fh % 16 != 0
        || !pixels.width.is_multiple_of(fw)
        || !pixels.height.is_multiple_of(fh)
    {
        let p = error(
            image,
            ImageLimit::ObjPair8x16,
            fh as usize,
            16,
            format!(
                "{image}'s frames are {fw}x{fh}; a frame is whole 8x16 objects (width a multiple \
                 of 8, height of 16) and the {}x{} sheet a grid of them",
                pixels.width, pixels.height
            ),
            &format!("rename it with a frame size that tiles the sheet, e.g. {stem}.16x16.png"),
        );
        return Err(vec![p]);
    }
    if let Some(p) = census::alpha(image, pixels, true) {
        return Err(vec![p]);
    }
    let colors = census::colors(pixels);
    if colors.len() > OBJ_SHADES {
        return Err(vec![census::too_many_colors(
            image,
            pixels,
            &colors,
            OBJ_SHADES,
            "an object palette has 3 shades plus transparent",
            "redraw it in at most 3 colors plus transparency, or split it into two sheets \
             (each object picks OBP0 or OBP1)",
        )]);
    }
    let mut warnings = Vec::new();
    if pixels.rgba.iter().all(|p| p[3] != 0) {
        warnings.push(ImageProblem {
            severity: Severity::Warning,
            ..error(
                image,
                ImageLimit::ObjNoTransparency,
                0,
                0,
                format!(
                    "{image} has no transparent pixel, so every object draws as a solid 8x16 \
                     block"
                ),
                "make the background transparent (alpha 0); color 0 of an object never draws",
            )
        });
    }
    // Lightest first as indices 1-3; OBP gives each its shade.
    let mut list: Vec<Rgba> = colors.iter().map(|(c, _)| *c).collect();
    let shade = shades(&list);
    let mut by_shade: Vec<(u8, Rgba)> = shade.iter().copied().zip(list.drain(..)).collect();
    by_shade.sort();
    let mut index: HashMap<Rgba, u8> = HashMap::new();
    let mut slots = vec!["#none".to_string()];
    let mut obp = 0u8;
    for (i, (s, c)) in by_shade.iter().enumerate() {
        let i = i as u8 + 1;
        index.insert(*c, i);
        slots.push(hex(*c));
        obp |= s << (2 * i);
    }
    while slots.len() < 4 {
        slots.push("#none".into());
    }
    let unit = |ux: u32, uy: u32| -> Vec<u8> {
        (0..128)
            .map(|i| {
                let p = pixels.at(ux + i % 8, uy + i / 8);
                if p[3] == 0 { 0 } else { index[&p] }
            })
            .collect()
    };

    let mut kept: Vec<(u32, u32)> = Vec::new();
    let mut uses: Vec<u32> = Vec::new();
    let mut known: HashMap<Vec<u8>, (usize, u8)> = HashMap::new();
    let mut frames = Vec::new();
    let mut problems = Vec::new();
    let (mut worst_line, mut worst_total): (Vec<TileAt>, Vec<TileAt>) = (Vec::new(), Vec::new());
    for fy in (0..pixels.height).step_by(fh as usize) {
        for fx in (0..pixels.width).step_by(fw as usize) {
            let mut objects = Vec::new();
            let mut placed = Vec::new();
            for oy in (0..fh).step_by(16) {
                let mut line = Vec::new();
                for ox in (0..fw).step_by(8) {
                    let (ux, uy) = (fx + ox, fy + oy);
                    let data = unit(ux, uy);
                    if data.iter().all(|&i| i == 0) {
                        continue;
                    }
                    let (k, flip) = *known.entry(data.clone()).or_insert_with(|| {
                        kept.push((ux, uy));
                        uses.push(0);
                        (kept.len() - 1, 0)
                    });
                    if flip == 0 && kept[k] == (ux, uy) {
                        for (f, variant) in flips(&data) {
                            known.entry(variant).or_insert((k, f));
                        }
                    }
                    uses[k] += 1;
                    objects.push(Object {
                        y: oy,
                        x: ox,
                        unit: k,
                        flip,
                    });
                    line.push(TileAt {
                        x: ux / 8,
                        y: uy / 8,
                    });
                }
                if line.len() > worst_line.len() {
                    worst_line = line.clone();
                }
                placed.extend(line);
            }
            if placed.len() > worst_total.len() {
                worst_total = placed;
            }
            frames.push(objects);
        }
    }
    let rename = format!(
        "if the sheet holds several frames, name their size: {stem}.WxH.png (e.g. {stem}.16x16.png)"
    );
    if worst_line.len() > PER_LINE {
        let n = worst_line.len();
        problems.push(ImageProblem {
            tiles: worst_line,
            ..error(
                image,
                ImageLimit::ObjPerLine,
                n,
                PER_LINE,
                format!(
                    "a {fw}x{fh} frame of {image} puts {n} objects side by side; a line shows \
                     {PER_LINE}, and the rest don't draw"
                ),
                &format!(
                    "narrow the frame to at most {} pixels; {rename}",
                    PER_LINE * 8
                ),
            )
        });
    }
    if worst_total.len() > TOTAL {
        let n = worst_total.len();
        problems.push(ImageProblem {
            tiles: worst_total,
            ..error(
                image,
                ImageLimit::ObjTotal,
                n,
                TOTAL,
                format!("a {fw}x{fh} frame of {image} needs {n} objects; OAM holds {TOTAL}"),
                &format!("make the frame smaller, or draw part of it on the background; {rename}"),
            )
        });
    }
    if kept.len() * 2 > OBJ_TILES {
        problems.push(error(
            image,
            ImageLimit::ObjTiles,
            kept.len() * 2,
            OBJ_TILES,
            format!(
                "{image} needs {} tiles ({} unique 8x16 objects); objects address {OBJ_TILES}",
                kept.len() * 2,
                kept.len()
            ),
            "reuse units across frames (flipped copies are free), or split the sheet",
        ));
    }
    if !problems.is_empty() {
        problems.extend(warnings);
        return Err(problems);
    }
    Ok(SheetPlan {
        spec: slots.join(","),
        obp,
        width: pixels.width / 8,
        height: pixels.height / 8,
        frame: (fw, fh),
        kept,
        uses,
        frames,
        warnings,
    })
}

/// A unit's flipped variants, as the hardware flips a whole 8x16 object.
fn flips(data: &[u8]) -> [(u8, Vec<u8>); 3] {
    let at = |flip: u8| -> Vec<u8> {
        (0..128)
            .map(|i| {
                let (x, y) = (i % 8, i / 8);
                let x = if flip & X_FLIP != 0 { 7 - x } else { x };
                let y = if flip & Y_FLIP != 0 { 15 - y } else { y };
                data[y * 8 + x]
            })
            .collect()
    };
    [
        (X_FLIP, at(X_FLIP)),
        (Y_FLIP, at(Y_FLIP)),
        (X_FLIP | Y_FLIP, at(X_FLIP | Y_FLIP)),
    ]
}
