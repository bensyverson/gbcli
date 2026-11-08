//! Color art's DMG version under `build --gbc` (the GBC images plan,
//! questions 2, 3 and 9): derived from the color image, each color taking
//! the shade the project's rule gives it (`shades::choose`, Ben's rulings of
//! 2026-11-07), unless `name.dmg.png` beside it overrides it.
//!
//! The derived version is drawn in the four DMG grays, one gray a shade, so
//! the DMG checks and rgbgfx see a 4-shade image. Color art maps many
//! colors to one shade by design, so the shared-shade warning is scoped to
//! what loses detail: two colors in one background tile, or one object,
//! that fall to one shade. A sheet whose derived version needs all four
//! shades can't be one object palette (3 plus transparent): `dmg_palette`.

use std::path::Path;

use gbcli_protocol::{BuildTarget, ImageKind, ImageLimit, ImageProblem, Severity, TileAt};

use super::census::{Key, error, key, tiles_where};
use super::find::Source;
use super::pixels::{Pixels, Rgba, hex};
use super::shades::{self, Shading, name};
use super::shared::{self, and};
use crate::project::BuildError;

/// Each shade's gray in a derived version, shade 0 first: the DMG grays,
/// a known palette, so the version's shading maps them exactly.
const GRAYS: [Rgba; 4] = [
    [0xFF, 0xFF, 0xFF, 0xFF],
    [0xAA, 0xAA, 0xAA, 0xFF],
    [0x55, 0x55, 0x55, 0xFF],
    [0x00, 0x00, 0x00, 0xFF],
];

/// Opaque shades in an object palette.
const OBJ_SHADES: usize = 3;

/// A derived DMG version, ready for the DMG checks.
pub(crate) struct Derived {
    pub pixels: Pixels,
    pub warnings: Vec<ImageProblem>,
}

/// The shading a derived version is checked and converted with: each gray
/// its own shade.
pub(crate) fn gray_shading() -> Shading {
    shades::choose(None, &GRAYS)
}

/// Where an image's DMG version comes from.
pub(crate) enum Version {
    /// The image is DMG art: a DMG build's every image, and indexed art.
    Art,
    /// Color art under `--gbc`: derived from its colors (`derive`).
    Derived,
    /// Color art under `--gbc` with `name.dmg.png` beside it: that image.
    Override(Pixels),
}

/// The DMG version of `s` (shown as `image`) for `target`: `assigned` is
/// the palette `assets/palettes.txt` assigns it, `over` its override's
/// pixels. An override beside indexed art is a layout error.
pub(crate) fn version(
    target: BuildTarget,
    assigned: Option<&String>,
    over: Option<Pixels>,
    s: &Source,
    image: &str,
    shown: &dyn Fn(&Path) -> String,
) -> Result<Version, BuildError> {
    Ok(match (target, assigned, over) {
        (BuildTarget::Dmg, _, _) | (BuildTarget::Gbc, Some(_), None) => Version::Art,
        (BuildTarget::Gbc, Some(palette), Some(_)) => {
            let dmg = s.dmg.as_deref().expect("an override has a path");
            return Err(BuildError::Layout {
                message: format!(
                    "{} is a DMG version, but {image} is indexed art (assets/palettes.txt \
                     assigns it `{palette}`): its shades already are its DMG version",
                    shown(dmg)
                ),
                remedy: format!(
                    "remove {} and draw {image} in its 4 shades; or drop the assignment \
                     to make {image} color art, which the DMG version then overrides",
                    shown(dmg)
                ),
            });
        }
        (BuildTarget::Gbc, None, None) => Version::Derived,
        (BuildTarget::Gbc, None, Some(o)) => Version::Override(o),
    })
}

/// The DMG version of the color image `image` (a `kind`), its colors
/// shaded by the project's `shading`; `dmg` names the override that would
/// replace it, for remedies.
pub(crate) fn derive(
    image: &str,
    dmg: &str,
    kind: ImageKind,
    pixels: &Pixels,
    shading: &Shading,
) -> Result<Derived, ImageProblem> {
    let shade = |p: Rgba| shading.shade(p);
    let rgba: Vec<Rgba> = pixels
        .rgba
        .iter()
        .map(|&p| {
            if p[3] == 0 {
                p
            } else {
                let g = GRAYS[usize::from(shade(p))];
                [g[0], g[1], g[2], p[3]]
            }
        })
        .collect();
    let derived = Pixels {
        width: pixels.width,
        height: pixels.height,
        rgba,
    };
    if kind == ImageKind::Sprites
        && let Some(p) = four_shades(image, dmg, pixels, &shade)
    {
        return Err(p);
    }
    let unit_height = if kind == ImageKind::Sprites { 16 } else { 8 };
    let mut warnings: Vec<ImageProblem> = shared_in_units(image, dmg, pixels, unit_height, &shade)
        .into_iter()
        .collect();
    let colors = super::census::colors(pixels);
    warnings.extend(shared::stretched(image, pixels, &colors, shading));
    Ok(Derived {
        pixels: derived,
        warnings,
    })
}

/// The `dmg_palette` error when a sheet's colors fall to all four shades,
/// naming the tiles of the least used.
fn four_shades(
    image: &str,
    dmg: &str,
    pixels: &Pixels,
    shade: &dyn Fn(Rgba) -> u8,
) -> Option<ImageProblem> {
    let mut used = [0u32; 4];
    for &p in pixels.rgba.iter().filter(|p| p[3] != 0) {
        used[usize::from(shade(p))] += 1;
    }
    if used.iter().filter(|&&n| n > 0).count() <= OBJ_SHADES {
        return None;
    }
    let least = (0..4u8).min_by_key(|&s| (used[usize::from(s)], s)).unwrap();
    let tiles = tiles_where(pixels, |p| p[3] != 0 && shade(p) == least);
    let message = format!(
        "{image}'s DMG version, derived from its colors by the project's shade rule, draws all \
         4 shades; an object palette has 3 shades plus transparent. The least used, shade \
         {least} ({}, {} px), is in {} tile(s) (listed)",
        name(least),
        used[usize::from(least)],
        tiles.len()
    );
    let remedy = format!(
        "draw the DMG version yourself as {dmg}, the same size, in at most 3 shades plus \
         transparency; or recolor the sheet so its colors fall to 3 shades (gbcli help \
         images){}",
        super::census::key_green_hint(pixels).unwrap_or("")
    );
    Some(ImageProblem {
        tiles,
        ..error(
            image,
            ImageLimit::DmgPalette,
            4,
            OBJ_SHADES,
            message,
            &remedy,
        )
    })
}

/// One `shared_shade` warning when any 8x`unit_height` unit (a background
/// tile, or a sheet's 8x16 object) holds two or more colors that fall to
/// one shade, listing each such unit by its top tile.
fn shared_in_units(
    image: &str,
    dmg: &str,
    pixels: &Pixels,
    unit_height: u32,
    shade: &dyn Fn(Rgba) -> u8,
) -> Option<ImageProblem> {
    let mut tiles = Vec::new();
    // The most colors one shade takes in one unit, and the first unit's
    // colors and shade, for the message.
    let mut most = 0;
    let mut first: Option<(TileAt, Vec<Rgba>, u8)> = None;
    for uy in (0..pixels.height).step_by(unit_height as usize) {
        for ux in (0..pixels.width).step_by(8) {
            // Each color (by RGB555 key) the unit holds, in order of first
            // use, with its shade.
            let mut held: Vec<(Key, Rgba, u8)> = Vec::new();
            for y in uy..(uy + unit_height).min(pixels.height) {
                for x in ux..(ux + 8).min(pixels.width) {
                    let p = pixels.at(x, y);
                    if p[3] != 0 && !held.iter().any(|&(k, _, _)| k == key(p)) {
                        held.push((key(p), p, shade(p)));
                    }
                }
            }
            let mut clash: Option<(Vec<Rgba>, u8)> = None;
            for s in 0..4u8 {
                let group: Vec<Rgba> = held
                    .iter()
                    .filter(|&&(_, _, t)| t == s)
                    .map(|&(_, c, _)| c)
                    .collect();
                if group.len() > 1 {
                    most = most.max(group.len());
                    clash.get_or_insert((group, s));
                }
            }
            if let Some((group, s)) = clash {
                let at = TileAt {
                    x: ux / 8,
                    y: uy / 8,
                };
                tiles.push(at);
                first.get_or_insert((at, group, s));
            }
        }
    }
    let (at, group, s) = first?;
    let unit = if unit_height == 8 { "tile" } else { "object" };
    let colors: Vec<String> = group.iter().map(|&c| hex(c)).collect();
    let message = format!(
        "{image} draws two or more colors in one DMG shade within {} {unit}(s) (listed), so \
         the DMG version loses their difference there; the first, at tile ({}, {}), draws {} \
         in shade {s} ({}), by the project's shade rule",
        tiles.len(),
        at.x,
        at.y,
        and(&colors),
        name(s),
    );
    let remedy = format!(
        "if the difference matters on the DMG, draw the DMG version yourself as {dmg}, the \
         same size, in 4 shades; or recolor one of them so it falls to another shade (gbcli \
         help images)"
    );
    Some(ImageProblem {
        severity: Severity::Warning,
        tiles,
        ..error(image, ImageLimit::SharedShade, most, 1, message, &remedy)
    })
}

/// The `dmg_override_size` error when the override `dmg` isn't the size of
/// the color image `image`.
pub(crate) fn override_size(
    image: &str,
    dmg: &str,
    color: &Pixels,
    over: &Pixels,
) -> Option<ImageProblem> {
    let (cw, ch, ow, oh) = (color.width, color.height, over.width, over.height);
    if (cw, ch) == (ow, oh) {
        return None;
    }
    let (count, max) = if ow != cw { (ow, cw) } else { (oh, ch) };
    Some(error(
        dmg,
        ImageLimit::DmgOverrideSize,
        count as usize,
        max as usize,
        format!(
            "{dmg} is {ow}x{oh} pixels; as the DMG version of {image}, which is {cw}x{ch}, it \
             must be the same size, pixel for pixel"
        ),
        &format!("crop or pad it to {cw}x{ch}, or remove it to derive the DMG version"),
    ))
}

/// The `alpha` error when the color image `image` can't stand beside its
/// override `dmg`: the DMG checks read the override, so the color image's
/// own transparency is checked here, before its palettes are packed (which
/// would count a half-transparent pixel opaque). A background must be
/// opaque, as on the DMG; a sheet's pixels must be opaque or clear, and
/// clear exactly where the override's are, since one frame list says which
/// objects both machines draw.
pub(crate) fn override_alpha(
    image: &str,
    dmg: &str,
    kind: ImageKind,
    color: &Pixels,
    over: &Pixels,
) -> Option<ImageProblem> {
    let sheet = kind == ImageKind::Sprites;
    if let Some(p) = super::census::alpha(image, color, sheet) {
        return Some(p);
    }
    if !sheet {
        return None;
    }
    let clear = |p: &Pixels, i: usize| p.rgba[i][3] == 0;
    let differ: Vec<usize> = (0..color.rgba.len())
        .filter(|&i| clear(color, i) != clear(over, i))
        .collect();
    let first = *differ.first()?;
    let at = |i: usize| (i as u32 % color.width, i as u32 / color.width);
    let mut tiles: Vec<TileAt> = differ
        .iter()
        .map(|&i| {
            let (x, y) = at(i);
            TileAt { x: x / 8, y: y / 8 }
        })
        .collect();
    tiles.dedup();
    tiles.sort_by_key(|t| (t.y, t.x));
    tiles.dedup();
    let (x, y) = at(first);
    let (here, there) = if clear(color, first) {
        ("transparent", "opaque")
    } else {
        ("opaque", "transparent")
    };
    let message = format!(
        "{image} and its DMG version {dmg} are transparent in different places: {} pixel(s) in \
         {} tile(s) (listed), the first ({x}, {y}), {here} in {image} and {there} in {dmg}. One \
         frame list serves both machines, so an object exists on both or on neither",
        differ.len(),
        tiles.len()
    );
    let remedy = format!(
        "make {dmg} transparent exactly where {image} is, pixel for pixel; or remove it to \
         derive the DMG version"
    );
    Some(ImageProblem {
        tiles,
        ..error(image, ImageLimit::Alpha, differ.len(), 0, message, &remedy)
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    const W: Rgba = [0xFF, 0xFF, 0xFF, 0xFF];
    const K: Rgba = [0x00, 0x00, 0x00, 0xFF];
    const BLUE: Rgba = [0x00, 0x00, 0xFF, 0xFF];
    const GREEN: Rgba = [0x00, 0xFF, 0x00, 0xFF];
    const CLEAR: Rgba = [0, 0, 0, 0];

    fn pixels(width: u32, height: u32, at: impl Fn(u32, u32) -> Rgba) -> Pixels {
        let rgba = (0..height)
            .flat_map(|y| (0..width).map(move |x| (x, y)))
            .map(|(x, y)| at(x, y))
            .collect();
        Pixels {
            width,
            height,
            rgba,
        }
    }

    fn shading(colors: &[Rgba]) -> Shading {
        shades::choose(None, colors)
    }

    /// Each pixel becomes its shade's gray, and transparency stays.
    #[test]
    fn each_color_becomes_the_gray_of_its_nearest_shade() {
        let colors = [W, GREEN, BLUE, K];
        let p = pixels(5, 1, |x, _| [W, GREEN, BLUE, K, CLEAR][x as usize]);
        let d = derive(
            "a.png",
            "a.dmg.png",
            ImageKind::Sprites,
            &p,
            &shading(&colors),
        );
        let d = d.unwrap();
        assert_eq!(
            d.pixels.rgba,
            [GRAYS[0], GRAYS[1], GRAYS[3], GRAYS[3], CLEAR]
        );
        let s = gray_shading();
        assert_eq!(s.shades(&GRAYS), [0, 1, 2, 3]);
    }

    #[test]
    fn a_shade_shared_within_an_object_names_its_top_tile() {
        let colors = [W, BLUE, K];
        // 16x16: the left object blue over black, the right blue only.
        let p = pixels(16, 16, |x, y| match (x < 8, y < 8) {
            (true, true) => BLUE,
            (true, false) => K,
            _ => BLUE,
        });
        let shade = |c: Rgba| shading(&colors).shade(c);
        let w = shared_in_units("a.png", "a.dmg.png", &p, 16, &shade).unwrap();
        assert_eq!(w.tiles, [TileAt { x: 0, y: 0 }]);
        assert_eq!((w.count, w.max), (2, 1));
        // As 8x8 tiles, no tile holds both.
        assert!(shared_in_units("a.png", "a.dmg.png", &p, 8, &shade).is_none());
    }
}
