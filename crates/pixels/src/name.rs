//! What an image's file name says about its pixels: a sheet's frame size.
//! The build reads names by these rules too, so the two can't disagree.

/// What a DMG version's name adds to its color image's, before the
/// extension: `town.dmg.png` is `town.png`'s DMG version.
pub const DMG_SUFFIX: &str = "dmg";

/// The frame size a file name gives, `hero.16x16.pixels` (or `.png`, in
/// any case) as (16, 16): the build's sheet convention, which the
/// canonical writer follows to write one block per frame. A sheet's DMG
/// version (`hero.16x16.dmg.pixels`) has its sheet's frame size.
pub fn frame_size(file_name: &str) -> Option<(u32, u32)> {
    let stem = file_name.rsplit_once('.').map_or(file_name, |(s, _)| s);
    let stem = match stem.rsplit_once('.') {
        Some((color, suffix)) if suffix.eq_ignore_ascii_case(DMG_SUFFIX) => color,
        _ => stem,
    };
    let (_, size) = stem.split_once('.')?;
    size_word(size)
}

/// Whether `file_name` names pixel text: its extension is `.pixels`, in
/// any case, as `.png` is found in any case.
pub fn is_text_name(file_name: &str) -> bool {
    file_name
        .rsplit_once('.')
        .is_some_and(|(_, ext)| ext.eq_ignore_ascii_case(crate::EXTENSION))
}

/// The extension a file named like `file_name` takes in a rename that
/// keeps its form: `pixels` for pixel text, else `png`.
pub fn extension_of(file_name: &str) -> &'static str {
    if is_text_name(file_name) {
        crate::EXTENSION
    } else {
        "png"
    }
}

/// A frame size as a name spells it, `16x24` as (16, 24): digits, a
/// lowercase `x`, digits.
pub fn size_word(text: &str) -> Option<(u32, u32)> {
    let (w, h) = text.split_once('x')?;
    let digits = |s: &str| !s.is_empty() && s.bytes().all(|b| b.is_ascii_digit());
    if !digits(w) || !digits(h) {
        return None;
    }
    Some((w.parse().ok()?, h.parse().ok()?))
}
