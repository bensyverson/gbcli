//! What every check starts from: an image's colors and where they are, and
//! the problem a check reports.

use std::collections::BTreeMap;

use gbcli_protocol::{ImageLimit, ImageProblem, Rgb555, Severity, TileAt};

use super::pixels::{Pixels, Rgba, hex};

/// A color as rgbgfx reads it: RGB555, each channel's top 5 bits. Colors
/// with one key are one color to rgbgfx (it merges them with a warning), so
/// every check counts and shades them as one.
pub(crate) type Key = [u8; 3];

/// The protocol's one truncation (`Rgb555::from_rgb888`), so a color the
/// checks count and one the palettes hold can't differ.
pub(crate) fn key(c: Rgba) -> Key {
    Rgb555::from_rgb888(c[0], c[1], c[2]).channels()
}

/// An image's colors (fully transparent pixels excluded) as rgbgfx counts
/// them, most used first. Each is its key's most used RGB value, which
/// stands for the rest in messages, in rgbgfx's spec and for its shade.
pub(crate) fn colors(pixels: &Pixels) -> Vec<(Rgba, u32)> {
    let mut count: BTreeMap<Rgba, u32> = BTreeMap::new();
    for &p in pixels.rgba.iter().filter(|p| p[3] != 0) {
        *count.entry(p).or_default() += 1;
    }
    let mut by_key: BTreeMap<Key, (Rgba, u32, u32)> = BTreeMap::new();
    for (c, n) in count {
        let (best, most, total) = by_key.entry(key(c)).or_insert((c, 0, 0));
        // The BTreeMap's order breaks ties by the smaller color.
        if n > *most {
            (*best, *most) = (c, n);
        }
        *total += n;
    }
    let mut colors: Vec<(Rgba, u32)> = by_key.into_values().map(|(c, _, n)| (c, n)).collect();
    colors.sort_by_key(|&(c, n)| (std::cmp::Reverse(n), c));
    colors
}

/// The 8x8 tiles, row-major, holding any pixel `hit` picks.
pub(crate) fn tiles_where(pixels: &Pixels, hit: impl Fn(Rgba) -> bool) -> Vec<TileAt> {
    let mut found = Vec::new();
    for ty in 0..pixels.height.div_ceil(8) {
        for tx in 0..pixels.width.div_ceil(8) {
            let any = (ty * 8..(ty * 8 + 8).min(pixels.height))
                .any(|y| (tx * 8..(tx * 8 + 8).min(pixels.width)).any(|x| hit(pixels.at(x, y))));
            if any {
                found.push(TileAt { x: tx, y: ty });
            }
        }
    }
    found
}

/// A problem with one image's palette: `colors` (most used first) is more
/// than `max`, and the tiles holding the colors past `max` are the ones to
/// redraw.
pub(crate) fn too_many_colors(
    image: &str,
    pixels: &Pixels,
    colors: &[(Rgba, u32)],
    max: usize,
    what: &str,
    remedy: &str,
) -> ImageProblem {
    let extra = &colors[max..];
    let tiles = tiles_where(pixels, |p| {
        p[3] != 0 && extra.iter().any(|(c, _)| key(*c) == key(p))
    });
    let named: Vec<String> = extra
        .iter()
        .map(|(c, n)| format!("{} ({n} px)", hex(*c)))
        .collect();
    let problem = error(
        image,
        ImageLimit::DmgPalette,
        colors.len(),
        max,
        format!(
            "{image} uses {} colors; {what}. The {} least used, {}, are in {} tile(s)",
            colors.len(),
            extra.len(),
            named.join(", "),
            tiles.len()
        ),
        remedy,
    );
    ImageProblem { tiles, ..problem }
}

/// One ROMX bank: each image is one section in images.asm, and a section
/// lives in one bank (Pan Docs, Memory Map; `man 5 rgbasm`, SECTION).
pub(crate) const BANK_BYTES: usize = 0x4000;

/// The problem when an image's section, `bytes` itemized by `what`, won't
/// fit one bank.
pub(crate) fn bank(image: &str, bytes: usize, what: &str, remedy: &str) -> Option<ImageProblem> {
    (bytes > BANK_BYTES).then(|| {
        error(
            image,
            ImageLimit::RomxBank,
            bytes,
            BANK_BYTES,
            format!(
                "{image} becomes {bytes} bytes ({what}); its section in images.asm is one ROMX \
                 bank of {BANK_BYTES}"
            ),
            remedy,
        )
    })
}

/// An error-severity problem with `image`, with no tiles named yet.
pub(crate) fn error(
    image: &str,
    limit: ImageLimit,
    count: usize,
    max: usize,
    message: String,
    remedy: &str,
) -> ImageProblem {
    ImageProblem {
        image: Some(image.to_string()),
        limit,
        severity: Severity::Error,
        count: count as u32,
        max: max as u32,
        tiles: Vec::new(),
        line: None,
        message,
        remedy: remedy.to_string(),
    }
}

/// Size problems: width and height a multiple of 8 (and, for 8x16 objects,
/// a height a multiple of 16).
pub(crate) fn size(image: &str, pixels: &Pixels, row: u32) -> Option<ImageProblem> {
    let (w, h) = (pixels.width, pixels.height);
    if w % 8 != 0 || h % 8 != 0 || w == 0 || h == 0 {
        let fit = |n: u32| (n / 8).max(1) * 8;
        return Some(error(
            image,
            ImageLimit::SizeMultiple8,
            if w % 8 != 0 { w } else { h } as usize,
            if w % 8 != 0 { fit(w) } else { fit(h) } as usize,
            format!("{image} is {w}x{h} pixels; tiles are 8x8, so both must be multiples of 8"),
            &format!("crop or pad it to {}x{}", fit(w), fit(h)),
        ));
    }
    if h % row != 0 {
        let fit = (h / row).max(1) * row;
        return Some(error(
            image,
            ImageLimit::ObjPair8x16,
            h as usize,
            fit as usize,
            format!(
                "{image} is {h} pixels high; 8x16 objects pair two tiles top over bottom, so a \
                 sheet's height is a multiple of 16"
            ),
            &format!("crop or pad it to {w}x{fit}"),
        ));
    }
    None
}

/// Pixels neither opaque nor fully transparent, or (where nothing may be
/// transparent) any pixel not opaque.
pub(crate) fn alpha(image: &str, pixels: &Pixels, transparent_ok: bool) -> Option<ImageProblem> {
    let bad = |p: Rgba| p[3] != 0xFF && !(transparent_ok && p[3] == 0);
    let count = pixels.rgba.iter().filter(|&&p| bad(p)).count();
    if count == 0 {
        return None;
    }
    let (message, remedy) = if transparent_ok {
        (
            format!(
                "{image} has {count} partly transparent pixel(s); an object's pixel is drawn \
                 or not"
            ),
            "make each pixel fully opaque or fully transparent (alpha 255 or 0)",
        )
    } else {
        (
            format!(
                "{image} has {count} pixel(s) that aren't opaque; the background has no \
                 transparency, so color 0 would draw there"
            ),
            "make every pixel opaque (alpha 255), in one of the image's colors",
        )
    };
    Some(ImageProblem {
        tiles: tiles_where(pixels, bad),
        ..error(image, ImageLimit::Alpha, count, 0, message, remedy)
    })
}
