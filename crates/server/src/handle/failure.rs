//! A failed request, and the gap-ledger entry it carries.

use gbcli_engine::DebugError;
use gbcli_protocol::{ErrorBody, FlashNoteRow, IgnoredWritesRow, StopRecord};

use crate::convert::{self, Gap, debug as debug_body};

/// A failed request: its body, and the gap-ledger entry it deserves.
pub(super) struct Failure {
    /// Boxed: a stop record makes an `ErrorBody` large, and every `Result`
    /// here pays for its error type on the stack.
    pub(super) body: Box<ErrorBody>,
    pub(super) gap: Option<Gap>,
}

impl Failure {
    pub(super) fn new(body: ErrorBody) -> Self {
        Failure {
            body: Box::new(body),
            gap: None,
        }
    }

    /// The same failure, carrying the stop record of the run it followed.
    pub(super) fn with_stop(mut self, stop: StopRecord) -> Self {
        self.body = Box::new((*self.body).with_stop(stop));
        self
    }

    /// The same failure, carrying the flash notes the failed run or step
    /// took (none leaves it as it was).
    pub(super) fn with_flash_notes(mut self, rows: Option<Vec<FlashNoteRow>>) -> Self {
        if rows.is_some() {
            self.body.flash_notes = rows;
        }
        self
    }

    /// The same failure, carrying the writes the PPU ignored in the failed
    /// run or step (none leaves it as it was).
    pub(super) fn with_ignored_writes(mut self, rows: Option<Vec<IgnoredWritesRow>>) -> Self {
        if rows.is_some() {
            self.body.ignored_writes = rows;
        }
        self
    }

    /// A core failure, with its gap-ledger entry when it is `Unsupported`.
    pub(super) fn from_core(error: &gbcli_core::CoreError) -> Self {
        Failure {
            body: Box::new(convert::core_error(error)),
            gap: convert::gap_of_core(error),
        }
    }

    /// A store failure, with its gap-ledger entry when the core underneath
    /// was `Unsupported`.
    pub(super) fn from_store(error: &gbcli_bundle::StoreError) -> Self {
        Failure {
            body: Box::new(convert::store(error)),
            gap: convert::gap_of_store(error),
        }
    }

    pub(super) fn from_debug(error: &DebugError) -> Self {
        Failure {
            body: Box::new(debug_body(error)),
            gap: convert::gap_of(error),
        }
    }
}

impl From<ErrorBody> for Failure {
    fn from(body: ErrorBody) -> Self {
        Failure::new(body)
    }
}
