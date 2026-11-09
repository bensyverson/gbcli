//! Frames taken from a core, owned and labeled: the last complete frame, or
//! the live, partly drawn one, never confused (design §9, DX 3 and 4).

use std::fmt;

use gbcli_core::{Core, FrameTag, PixelBuf, Pixels, PpuMode, PpuStatus, Ticks, VblankKind};

use crate::ViewError;
use crate::colors::DMG_GRAYS;
use crate::image::{Image, LabelKey};

/// Which frame to take.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FrameChoice {
    /// The last complete frame, refused unless its vblank was normal.
    Complete,
    /// The last complete frame whatever its vblank kind (LCD off, artificial).
    CompleteAnyKind,
    /// The framebuffer as it is now, partly drawn.
    Live,
}

/// Which frame an image shows.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FrameSource {
    /// A frame a vblank completed.
    Complete(FrameTag),
    /// The framebuffer mid-frame: lines above `ppu.ly` are the frame in
    /// progress; the rest still hold what was there before.
    Live {
        ppu: PpuStatus,
        at: Ticks,
        /// The number of the last complete frame, which this one follows.
        after: Option<u64>,
    },
}

pub(crate) fn describe_tag(tag: &FrameTag) -> String {
    let kind = match tag.kind {
        VblankKind::Normal => "normal",
        VblankKind::LcdOff => "lcd_off",
        VblankKind::Artificial => "artificial",
    };
    format!("complete frame {} (vblank {kind})", tag.number)
}

fn mode_name(mode: Option<PpuMode>) -> &'static str {
    match mode {
        Some(PpuMode::HBlank) => "hblank",
        Some(PpuMode::VBlank) => "vblank",
        Some(PpuMode::OamScan) => "oam_scan",
        Some(PpuMode::Drawing) => "drawing",
        None => "mode unknown",
    }
}

impl fmt::Display for FrameSource {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            FrameSource::Complete(tag) => f.write_str(&describe_tag(tag)),
            FrameSource::Live { ppu, after, .. } => {
                write!(
                    f,
                    "live partial frame: LY {} ({})",
                    ppu.ly,
                    mode_name(ppu.mode)
                )?;
                if !ppu.lcd_on {
                    f.write_str(", LCD off")?;
                }
                match after {
                    Some(n) => write!(f, ", after complete frame {n}"),
                    None => f.write_str(", before any complete frame"),
                }
            }
        }
    }
}

/// A frame, owned so a caller can collect several (for a sheet or a diff)
/// while the core runs on.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Frame {
    pub source: FrameSource,
    pub width: u32,
    pub height: u32,
    /// What the core output: shades on a DMG, 0x00RRGGBB otherwise.
    pub pixels: PixelBuf,
}

/// Takes a frame from `core` without running it.
pub fn capture(core: &dyn Core, choice: FrameChoice) -> Result<Frame, ViewError> {
    if choice == FrameChoice::Live {
        let live = core.live_frame()?;
        return Ok(Frame {
            source: FrameSource::Live {
                ppu: core.ppu(),
                at: core.now(),
                after: core.last_complete_frame().map(|f| f.tag.number),
            },
            width: live.width,
            height: live.height,
            pixels: live.pixels,
        });
    }
    let frame = core.last_complete_frame().ok_or(ViewError::NoFrame)?;
    if choice == FrameChoice::Complete && frame.tag.kind != VblankKind::Normal {
        return Err(ViewError::NotNormal { tag: frame.tag });
    }
    Ok(Frame {
        source: FrameSource::Complete(frame.tag),
        width: frame.width,
        height: frame.height,
        pixels: match frame.pixels {
            Pixels::Shades(s) => PixelBuf::Shades(s.to_vec()),
            Pixels::Rgb(rgb) => PixelBuf::Rgb(rgb.to_vec()),
        },
    })
}

impl Frame {
    /// The pixels as RGB: the core's own RGB, or DMG shades as [`DMG_GRAYS`].
    pub fn rgb(&self) -> Vec<u32> {
        match &self.pixels {
            PixelBuf::Shades(s) => s.iter().map(|&s| DMG_GRAYS[usize::from(s & 3)]).collect(),
            PixelBuf::Rgb(rgb) => rgb.clone(),
        }
    }

    /// The frame at native size, labeled with what it is.
    pub fn image(&self) -> Image {
        Image {
            width: self.width,
            height: self.height,
            pixels: self.rgb(),
            labels: Vec::new(),
        }
        .label(LabelKey::View, "frame")
        .label(LabelKey::Frame, self.source.to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn live(mode: Option<PpuMode>) -> String {
        FrameSource::Live {
            ppu: PpuStatus {
                ly: 50,
                mode,
                dot: None,
                lcd_on: true,
            },
            at: Ticks(0),
            after: Some(3),
        }
        .to_string()
    }

    #[test]
    fn a_live_frame_names_its_mode_or_says_it_is_unknown() {
        assert_eq!(
            live(Some(PpuMode::Drawing)),
            "live partial frame: LY 50 (drawing), after complete frame 3"
        );
        assert_eq!(
            live(None),
            "live partial frame: LY 50 (mode unknown), after complete frame 3"
        );
    }
}
