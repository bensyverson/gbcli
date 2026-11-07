//! Views over several frames the caller collected: filmstrips, atlases and
//! diffs. Nothing here runs the core; the caller steps it between captures.

use gbcli_core::PixelBuf;

use crate::ViewError;
use crate::colors::{CHANGED, GAP};
use crate::frame::Frame;
use crate::image::{Image, LabelKey, Rect};

/// The gap between cells, in native pixels.
const GAP_PX: u32 = 1;

fn same_size(frames: &[&Frame]) -> Result<(u32, u32), ViewError> {
    let first = frames.first().ok_or(ViewError::NoFrames)?;
    let (w, h) = (first.width, first.height);
    if let Some(other) = frames.iter().find(|f| (f.width, f.height) != (w, h)) {
        return Err(ViewError::Mismatch {
            reason: format!(
                "frames differ in size: {w}x{h} and {}x{}",
                other.width, other.height
            ),
        });
    }
    Ok((w, h))
}

fn frames_label(frames: &[&Frame]) -> String {
    frames
        .iter()
        .enumerate()
        .map(|(i, f)| format!("[{i}] {}", f.source))
        .collect::<Vec<_>>()
        .join("; ")
}

/// The frames left to right, top to bottom, `columns` to a row, with a
/// one-pixel gap between cells.
pub fn sheet(frames: &[Frame], columns: u32) -> Result<Image, ViewError> {
    if columns == 0 {
        return Err(ViewError::BadColumns);
    }
    let refs: Vec<&Frame> = frames.iter().collect();
    let (w, h) = same_size(&refs)?;
    let count = frames.len() as u32;
    let columns = columns.min(count);
    let rows = count.div_ceil(columns);
    let mut image = Image::filled(
        columns * w + (columns - 1) * GAP_PX,
        rows * h + (rows - 1) * GAP_PX,
        GAP,
    );
    for (i, frame) in frames.iter().enumerate() {
        let (col, row) = (i as u32 % columns, i as u32 / columns);
        image.blit(&frame.image(), col * (w + GAP_PX), row * (h + GAP_PX));
    }
    Ok(image
        .label(LabelKey::View, format!("sheet {columns} columns"))
        .label(LabelKey::Frame, frames_label(&refs)))
}

/// The frames in one row.
pub fn filmstrip(frames: &[Frame]) -> Result<Image, ViewError> {
    sheet(frames, (frames.len() as u32).max(1))
}

/// How two frames differ.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct DiffSummary {
    /// Pixels that differ.
    pub changed: usize,
    /// The smallest rectangle holding them, if any do.
    pub bounds: Option<Rect>,
}

/// Halfway to white, so changed pixels stand out against the rest.
fn faded(color: u32) -> u32 {
    let channel = |shift: u32| (((color >> shift) & 0xFF) + 0xFF) / 2;
    (channel(16) << 16) | (channel(8) << 8) | channel(0)
}

/// `after` with every pixel that differs from `before` in [`CHANGED`] and
/// the rest faded, and the count and bounds of the changes.
pub fn diff(before: &Frame, after: &Frame) -> Result<(Image, DiffSummary), ViewError> {
    let (w, _) = same_size(&[before, after])?;
    let comparable = matches!(
        (&before.pixels, &after.pixels),
        (PixelBuf::Shades(_), PixelBuf::Shades(_)) | (PixelBuf::Rgb(_), PixelBuf::Rgb(_))
    );
    if !comparable {
        return Err(ViewError::Mismatch {
            reason: "one frame holds DMG shades and the other RGB".into(),
        });
    }
    let (a, b) = (before.rgb(), after.rgb());
    let mut changed = 0;
    let (mut x0, mut y0, mut x1, mut y1) = (u32::MAX, u32::MAX, 0, 0);
    let pixels = a
        .iter()
        .zip(&b)
        .enumerate()
        .map(|(i, (&pa, &pb))| {
            if pa == pb {
                return faded(pb);
            }
            let (x, y) = (i as u32 % w, i as u32 / w);
            changed += 1;
            (x0, y0, x1, y1) = (x0.min(x), y0.min(y), x1.max(x), y1.max(y));
            CHANGED
        })
        .collect();
    let bounds = (changed > 0).then(|| Rect {
        x: x0,
        y: y0,
        width: x1 - x0 + 1,
        height: y1 - y0 + 1,
    });
    let detail = match bounds {
        Some(r) => format!(
            "{changed} pixels changed within x={} y={} w={} h={}",
            r.x, r.y, r.width, r.height
        ),
        None => "no pixels changed".to_string(),
    };
    let image = Image {
        width: after.width,
        height: after.height,
        pixels,
        labels: Vec::new(),
    }
    .label(LabelKey::View, "diff")
    .label(LabelKey::Frame, frames_label(&[before, after]))
    .label(LabelKey::Detail, detail);
    Ok((image, DiffSummary { changed, bounds }))
}
