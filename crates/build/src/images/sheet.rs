//! A sprite sheet of 8x16 objects on the DMG: three shades plus
//! transparent per object palette, ten objects a line, forty in all, and
//! whole 8x16 units deduplicated with flips (rgbgfx has no 8x16 mode, and
//! its `-u` would split a unit's pair).

use std::collections::HashMap;

use gbcli_protocol::{Flip, ImageLimit, ImageProblem, PaletteSlot, Severity, TileAt, TileEntry};

use super::census::{self, Key, error, gbc_word, key};
use super::pixels::{Pixels, Rgba, hex};
use super::shades::Shading;
use super::shared;

/// Objects one line shows, and OAM holds (Pan Docs, OAM).
pub(crate) const PER_LINE: usize = 10;
pub(crate) const TOTAL: usize = 40;
/// Tiles objects address ($8000-$8FFF).
pub(crate) const OBJ_TILES: usize = 256;
/// Opaque shades in an object palette.
const OBJ_SHADES: usize = 3;

/// One object of a frame.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct Object {
    /// Pixels from the frame's top-left.
    pub y: u32,
    pub x: u32,
    /// Which kept unit; its tiles are 2 * unit and 2 * unit + 1.
    pub unit: usize,
    pub flip: Flip,
}

/// A sheet that fits, ready for rgbgfx and the dedup.
#[derive(Debug)]
pub(crate) struct SheetPlan {
    pub spec: String,
    /// The colors rgbgfx must see as another (`shared`): none unless two
    /// colors share a shade.
    pub merges: Vec<(Key, Rgba)>,
    /// OBP: index 0 transparent, 1-3 the sheet's shades, lightest first.
    pub obp: u8,
    /// Size in tiles.
    pub width: u32,
    pub height: u32,
    /// The frame size in pixels.
    pub frame: (u32, u32),
    /// The kept units by their top-left in sheet pixels, in order of first
    /// use, and each one's uses: on the DMG alone, units alike in DMG
    /// data; with a GBC version, alike in both.
    pub kept: Vec<(u32, u32)>,
    /// Each kept unit's color indices, 8 a row, 16 rows: what rgbgfx's
    /// data must draw (`agree`).
    pub drawn: Vec<Vec<u8>>,
    pub uses: Vec<u32>,
    pub frames: Vec<Vec<Object>>,
    /// Each 8x16 unit of the sheet, rows top to bottom: the kept unit it
    /// draws and how flipped, or none when it is transparent.
    pub units: Vec<Option<(usize, Flip)>>,
    /// Problems that don't stop the build.
    pub warnings: Vec<ImageProblem>,
}

impl SheetPlan {
    pub fn tiles(&self) -> u32 {
        self.kept.len() as u32 * 2
    }

    /// The protocol's tile map: each unit's top tile (`2 * unit`) and flip,
    /// with `slot(unit)` as its palette, of the kept unit it draws.
    pub fn tile_map(&self, slot: impl Fn(usize) -> Option<PaletteSlot>) -> Vec<Option<TileEntry>> {
        self.units
            .iter()
            .map(|u| {
                u.map(|(unit, flip)| TileEntry {
                    index: unit as u32 * 2,
                    flip,
                    palette: slot(unit),
                })
            })
            .collect()
    }
}

/// Checks a sheet whose frames are `frame` pixels (the whole sheet when
/// `None`; its name, `<name>.WxH.png`, may give another); the plan when it
/// fits, else every problem found. `shading` is the project's
/// (`shades::choose`). `color` is the color image whose DMG version
/// `pixels` is, under `--gbc`, transparent where it is (`derive`): units
/// are then one only when they match in both, flipped alike, so one frame
/// list serves both machines (plan question 1).
pub(crate) fn check(
    image: &str,
    stem: &str,
    pixels: &Pixels,
    frame: Option<(u32, u32)>,
    shading: &Shading,
    color: Option<&Pixels>,
) -> Result<SheetPlan, Vec<ImageProblem>> {
    if let Some(p) = census::size(image, pixels, 16) {
        return Err(vec![p]);
    }
    let (fw, fh) = frame.unwrap_or((pixels.width, pixels.height));
    if let Some(p) = frame_shape(image, stem, pixels, frame) {
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
    // One index a shade, lightest first, from 1; OBP gives each its shade.
    // Colors that share a shade share its index (`shared`).
    let list: Vec<Rgba> = colors.iter().map(|(c, _)| *c).collect();
    let shade = shading.shades(&list);
    let shared = shared::find(image, pixels, &colors, &shade, shading.choice);
    warnings.extend(shared.warnings);
    warnings.extend(shared::stretched(image, pixels, &colors, shading));
    let mut used = shade.clone();
    used.sort();
    used.dedup();
    let mut index: HashMap<Key, u8> = HashMap::new();
    let mut slots = vec!["#none".to_string()];
    let mut obp = 0u8;
    for (i, &s) in used.iter().enumerate() {
        let i = i as u8 + 1;
        // The shade's most used color stands for it in rgbgfx's spec.
        let mut first = true;
        for (&c, _) in list.iter().zip(&shade).filter(|&(_, &t)| t == s) {
            index.insert(key(c), i);
            if first {
                slots.push(hex(c));
                first = false;
            }
        }
        obp |= s << (2 * i);
    }
    while slots.len() < 4 {
        slots.push("#none".into());
    }
    let unit = |ux: u32, uy: u32| -> Vec<u8> {
        (0..128)
            .map(|i| {
                let p = pixels.at(ux + i % 8, uy + i / 8);
                if p[3] == 0 { 0 } else { index[&key(p)] }
            })
            .collect()
    };

    let mut kept: Vec<(u32, u32)> = Vec::new();
    let mut drawn: Vec<Vec<u8>> = Vec::new();
    let mut uses: Vec<u32> = Vec::new();
    // Each unit as deduplicated: its DMG indices and, with a GBC version,
    // its GBC colors (`census::gbc_word`).
    let gbc = |ux: u32, uy: u32| -> Vec<u16> {
        color.map_or_else(Vec::new, |c| {
            (0..128)
                .map(|i| gbc_word(c.at(ux + i % 8, uy + i / 8)))
                .collect()
        })
    };
    let mut known: HashMap<(Vec<u8>, Vec<u16>), (usize, Flip)> = HashMap::new();
    let columns = (pixels.width / 8) as usize;
    let mut units: Vec<Option<(usize, Flip)>> = vec![None; columns * (pixels.height / 16) as usize];
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
                    let both = (data.clone(), gbc(ux, uy));
                    let (k, flip) = *known.entry(both.clone()).or_insert_with(|| {
                        kept.push((ux, uy));
                        drawn.push(data.clone());
                        uses.push(0);
                        (kept.len() - 1, Flip::None)
                    });
                    if flip.is_none() && kept[k] == (ux, uy) {
                        let colors = flips(&both.1);
                        for ((f, variant), (_, colored)) in flips(&both.0).into_iter().zip(colors) {
                            known.entry((variant, colored)).or_insert((k, f));
                        }
                    }
                    units[(uy / 16) as usize * columns + (ux / 8) as usize] = Some((k, flip));
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
    // images.asm: the units, a pointer per frame, then each frame's count
    // and four bytes an object.
    let lists: usize = frames.iter().map(|f| 1 + 4 * f.len()).sum();
    problems.extend(census::bank(
        image,
        kept.len() * 32 + frames.len() * 2 + lists,
        &format!(
            "{} units of 32 bytes, and {} frames' pointers and object lists",
            kept.len(),
            frames.len()
        ),
        "split it into sheets of fewer frames; each sheet is its own section",
    ));
    if !problems.is_empty() {
        problems.extend(warnings);
        return Err(problems);
    }
    Ok(SheetPlan {
        spec: slots.join(","),
        merges: shared.merges,
        obp,
        width: pixels.width / 8,
        height: pixels.height / 8,
        frame: (fw, fh),
        kept,
        drawn,
        uses,
        frames,
        units,
        warnings,
    })
}

/// Pixels a frame spans at most each way: OAM positions are bytes, and so
/// are the offsets in a frame's object list.
const FRAME_SPAN: u32 = 256;

/// The problem with the sheet's frames, if any: whole 8x16 objects, a grid
/// that tiles the sheet, and a span OAM can place. `named` is the frame
/// size its file name gives; the whole sheet is one frame without one.
fn frame_shape(
    image: &str,
    stem: &str,
    pixels: &Pixels,
    named: Option<(u32, u32)>,
) -> Option<ImageProblem> {
    let (w, h) = (pixels.width, pixels.height);
    let (fw, fh) = named.unwrap_or((w, h));
    let named = named.is_some();
    // The frame size nearest (at most) the one named that is whole objects,
    // tiles the sheet and fits the span: 8x16 always does, since the sheet
    // is whole objects (census::size).
    let tiling = |want: u32, unit: u32, sheet: u32| {
        (1..=want.clamp(unit, FRAME_SPAN) / unit)
            .rev()
            .map(|n| n * unit)
            .find(|&f| sheet.is_multiple_of(f))
            .unwrap_or(unit)
    };
    let offer = format!("{stem}.{}x{}.png", tiling(fw, 8, w), tiling(fh, 16, h));
    let whole = |n: u32, unit: u32| (n / unit).max(1) * unit;
    let frames = format!("{image}'s frames are {fw}x{fh}");
    let (limit, count, max, message, remedy) = if fw == 0 || !fw.is_multiple_of(8) {
        (
            ImageLimit::SizeMultiple8,
            fw,
            whole(fw, 8),
            format!("{frames}; a frame is whole 8x16 objects, so its width is a multiple of 8"),
            format!("rename it {offer}"),
        )
    } else if fh == 0 || !fh.is_multiple_of(16) {
        (
            ImageLimit::ObjPair8x16,
            fh,
            whole(fh, 16),
            format!(
                "{frames}; 8x16 objects pair two tiles top over bottom, so a frame's height is \
                 a multiple of 16"
            ),
            format!("rename it {offer}"),
        )
    } else if !w.is_multiple_of(fw) || !h.is_multiple_of(fh) {
        let (count, frame) = if w.is_multiple_of(fw) {
            (h, fh)
        } else {
            (w, fw)
        };
        let pad = (w.div_ceil(fw) * fw, h.div_ceil(fh) * fh);
        (
            ImageLimit::FrameGrid,
            count,
            count / frame * frame,
            format!(
                "{frames}, which don't tile the {w}x{h} sheet; frames run left to right, then \
                 down, so its width and height are whole frames"
            ),
            format!(
                "rename it {offer}, whose frames tile it, or pad the sheet to {}x{} to keep \
                 {fw}x{fh} frames",
                pad.0, pad.1
            ),
        )
    } else if fw > FRAME_SPAN || fh > FRAME_SPAN {
        let named = if named {
            "name a smaller frame size"
        } else {
            "if the sheet holds several frames, name their size"
        };
        (
            ImageLimit::FrameSpan,
            fw.max(fh),
            FRAME_SPAN,
            format!(
                "{frames}; OAM positions are bytes, so one frame's objects span at most \
                 {FRAME_SPAN} pixels each way"
            ),
            format!("{named}, e.g. {offer}, or split the image into frames that move together"),
        )
    } else {
        return None;
    };
    Some(error(
        image,
        limit,
        count as usize,
        max as usize,
        message,
        &remedy,
    ))
}

/// A unit's flipped variants, as the hardware flips a whole 8x16 object;
/// none of an empty unit (a DMG-only key's GBC half).
fn flips<T: Copy>(data: &[T]) -> [(Flip, Vec<T>); 3] {
    let at = |flip: Flip| -> Vec<T> {
        (0..data.len())
            .map(|i| {
                let (x, y) = (i % 8, i / 8);
                let x = if flip.x() { 7 - x } else { x };
                let y = if flip.y() { 15 - y } else { y };
                data[y * 8 + x]
            })
            .collect()
    };
    [Flip::X, Flip::Y, Flip::Xy].map(|f| (f, at(f)))
}
