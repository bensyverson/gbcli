//! What the audio views can fail with.

use std::fmt;

use gbcli_core::{ApuChannel, CoreError};
use gbcli_view::ViewError;

use crate::reg::channel_name;

#[derive(Debug)]
pub enum AudioError {
    Core(CoreError),
    /// A per-channel replay ended in another state, or rendered another
    /// number of samples, than the mixed run: the runs weren't the same span.
    ReplayDiverged {
        channel: ApuChannel,
    },
    /// There are no samples to draw.
    NoSamples,
    /// The time span is empty or reversed.
    EmptySpan,
    Image(ViewError),
}

impl From<CoreError> for AudioError {
    fn from(err: CoreError) -> Self {
        AudioError::Core(err)
    }
}

impl From<ViewError> for AudioError {
    fn from(err: ViewError) -> Self {
        AudioError::Image(err)
    }
}

impl fmt::Display for AudioError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            AudioError::Core(err) => err.fmt(f),
            AudioError::ReplayDiverged { channel } => write!(
                f,
                "the {} replay diverged from the mixed run; each replay must start from the \
                 same state and run the same span",
                channel_name(*channel)
            ),
            AudioError::NoSamples => f.write_str("no audio samples to draw; capture some first"),
            AudioError::EmptySpan => f.write_str("the time span is empty"),
            AudioError::Image(err) => err.fmt(f),
        }
    }
}

impl std::error::Error for AudioError {}
