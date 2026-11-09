//! A run's key timeline (design §11 step 3). An event at frame `at` is
//! applied when the run has completed `at` frames: at the vblank that ends
//! frame `at`, or before the first instruction for 0. Held keys stay in the
//! machine, so the head (and its checkpoint header) keeps them.

use gbcli_core::Keys;
use gbcli_protocol::{KeyAction, KeyEvent};

use super::Since;
use crate::cond::eval::frame_number;
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

/// A run's key events as the caller wrote them, counted from the run's start
/// or from a time base (`run --keys-since`), and as the run applies them.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct KeyTimeline {
    /// At offsets from the run's start.
    run: Vec<KeyEvent>,
    /// The caller's offset of the run's frame 0: negative when the base
    /// comes after the run's start.
    start: i64,
}

impl KeyTimeline {
    /// Offsets that count from the run's start.
    pub fn from_run_start(events: &[KeyEvent]) -> KeyTimeline {
        KeyTimeline {
            run: events.to_vec(),
            start: 0,
        }
    }

    /// Offsets that count from `since`, for a run starting where `m` is:
    /// frames counted as a stop record counts them, so an offset is the
    /// `frame=` a stop at its vblank would print with `--since` that base.
    pub fn from_base(events: &[KeyEvent], m: &Machine, since: &Since) -> KeyTimeline {
        let base = m.time_base(since).frame as i64;
        let now = frame_number(m.core()) as i64;
        KeyTimeline::rebased(events, now - base)
    }

    /// Offsets whose `start` is the run's frame 0. The events before it are
    /// dropped: the state the run starts from already holds them, when it
    /// came from the same timeline, and reporting them as unapplied would
    /// bury the ones that are. The events at `start` are applied before the
    /// first instruction, as a run's events at 0 are; the run that reached
    /// this state applied them at its last vblank too, which leaves the
    /// buttons as they are.
    fn rebased(events: &[KeyEvent], start: i64) -> KeyTimeline {
        let run = events
            .iter()
            .filter_map(|e| {
                let at = u64::try_from(e.at as i64 - start).ok()?;
                Some(KeyEvent { at, ..*e })
            })
            .collect();
        KeyTimeline { run, start }
    }

    /// The events at offsets from the run's start, for [`RunOptions::keys`].
    ///
    /// [`RunOptions::keys`]: super::RunOptions::keys
    pub fn events(&self) -> &[KeyEvent] {
        &self.run
    }

    /// The events a run that completed `frames` frames never reached, at
    /// the caller's offsets.
    pub fn unapplied(&self, frames: u64) -> Vec<KeyEvent> {
        unapplied_keys(&self.run, frames)
            .into_iter()
            .map(|e| KeyEvent {
                at: (e.at as i64 + self.start) as u64,
                ..e
            })
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use gbcli_protocol::parse_keys;

    use super::*;

    fn keys(text: &str) -> Vec<KeyEvent> {
        parse_keys(text).unwrap()
    }

    #[test]
    fn a_base_before_the_start_drops_the_earlier_events_and_shifts_the_rest() {
        // The run starts 5 frames after the base.
        let t = KeyTimeline::rebased(&keys("A@2 -A@5 B@7 START@20"), 5);
        assert_eq!(t.events(), keys("-A@0 B@2 START@15"));
        assert_eq!(t.unapplied(10), keys("START@20"), "in the caller's offsets");
        assert_eq!(t.unapplied(15), vec![]);
    }

    #[test]
    fn the_run_start_as_the_base_changes_nothing() {
        let events = keys("A@0 -A@3");
        let t = KeyTimeline::from_run_start(&events);
        assert_eq!(t, KeyTimeline::rebased(&events, 0));
        assert_eq!(t.events(), events);
        assert_eq!(t.unapplied(2), keys("-A@3"));
    }

    #[test]
    fn a_base_after_the_start_delays_every_event() {
        let t = KeyTimeline::rebased(&keys("A@0 B@2"), -3);
        assert_eq!(t.events(), keys("A@3 B@5"));
        assert_eq!(t.unapplied(4), keys("B@2"));
    }
}
