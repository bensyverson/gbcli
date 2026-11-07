//! A run's key timeline (design §11 step 3). An event at frame `at` is
//! applied when the run has completed `at` frames: at the vblank that ends
//! frame `at`, or before the first instruction for 0. Held keys stay in the
//! machine, so the head (and its checkpoint header) keeps them.

use gbcli_core::Keys;
use gbcli_protocol::{KeyAction, KeyEvent};

use crate::machine::Machine;

/// The events not yet applied, in frame order (stable for one frame).
pub(crate) struct Schedule {
    events: Vec<KeyEvent>,
    next: usize,
}

impl Schedule {
    pub fn new(events: &[KeyEvent]) -> Schedule {
        let mut events = events.to_vec();
        events.sort_by_key(|e| e.at);
        Schedule { events, next: 0 }
    }

    /// Applies every event due once the run has completed `frames` frames.
    pub fn apply_due(&mut self, m: &mut Machine, frames: u64) {
        while let Some(event) = self.events.get(self.next).filter(|e| reached(e, frames)) {
            let bit = event.button.bit();
            let held = m.keys().0;
            let keys = match event.action {
                KeyAction::Press => held | bit,
                KeyAction::Release => held & !bit,
            };
            m.set_keys(Keys(keys));
            self.next += 1;
        }
    }

    /// Whether an event is still waiting, so the run must stop at vblanks.
    pub fn pending(&self) -> bool {
        self.next < self.events.len()
    }
}

/// The events a run that completed `frames` frames never reached: the ones
/// to report, since the run applied every other.
pub fn unapplied_keys(events: &[KeyEvent], frames: u64) -> Vec<KeyEvent> {
    events
        .iter()
        .filter(|e| !reached(e, frames))
        .copied()
        .collect()
}

/// Whether a run that completed `frames` frames has applied `event`.
fn reached(event: &KeyEvent, frames: u64) -> bool {
    event.at <= frames
}
