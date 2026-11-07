use std::fmt;

use gbcli_core::{CoreError, FrameTag};

use crate::image::Rect;

#[derive(Debug)]
pub enum ViewError {
    /// No vblank has completed a frame yet.
    NoFrame,
    /// The last complete frame isn't a normal one (LCD off, artificial), and
    /// the caller didn't ask for any kind (design §9, DX 4).
    NotNormal {
        tag: FrameTag,
    },
    BadScale {
        scale: u32,
    },
    CropOutside {
        rect: Rect,
        width: u32,
        height: u32,
    },
    /// A sheet of no frames.
    NoFrames,
    /// A sheet of zero columns.
    BadColumns,
    /// Frames that can't share a sheet or a diff: sizes or pixel kinds differ.
    Mismatch {
        reason: String,
    },
    Core(CoreError),
    Png(String),
}

impl From<CoreError> for ViewError {
    fn from(err: CoreError) -> Self {
        ViewError::Core(err)
    }
}

impl fmt::Display for ViewError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ViewError::NoFrame => write!(f, "no frame has completed yet; run at least one frame"),
            ViewError::NotNormal { tag } => write!(
                f,
                "the last complete frame ({}) is not a normal frame; ask for any kind to see it",
                crate::frame::describe_tag(tag)
            ),
            ViewError::BadScale { scale } => {
                write!(f, "scale {scale} is outside 1-{}", crate::image::MAX_SCALE)
            }
            ViewError::CropOutside {
                rect,
                width,
                height,
            } => write!(
                f,
                "crop x={} y={} w={} h={} is empty or outside the {width}x{height} image",
                rect.x, rect.y, rect.width, rect.height
            ),
            ViewError::NoFrames => write!(f, "a sheet needs at least one frame"),
            ViewError::BadColumns => write!(f, "a sheet needs at least one column"),
            ViewError::Mismatch { reason } => write!(f, "{reason}"),
            ViewError::Core(err) => write!(f, "{err}"),
            ViewError::Png(err) => write!(f, "PNG encoding failed: {err}"),
        }
    }
}

impl std::error::Error for ViewError {}
