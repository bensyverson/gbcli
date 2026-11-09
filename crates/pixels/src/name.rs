//! What an image's file name says about its pixels: a sheet's frame size.

/// The frame size a file name gives, `hero.16x16.pixels` (or `.png`) as
/// (16, 16): the build's sheet convention, which the canonical writer
/// follows to write one block per frame.
pub fn frame_size(file_name: &str) -> Option<(u32, u32)> {
    let stem = file_name.rsplit_once('.').map_or(file_name, |(s, _)| s);
    let (_, size) = stem.split_once('.')?;
    let (w, h) = size.split_once('x')?;
    let digits = |s: &str| !s.is_empty() && s.bytes().all(|b| b.is_ascii_digit());
    if !digits(w) || !digits(h) {
        return None;
    }
    Some((w.parse().ok()?, h.parse().ok()?))
}
