//! `sessions prune` (design §1): removing sessions nobody has used lately.

use std::time::Duration;

use crate::bundle::Bundle;
use crate::error::StoreError;
use crate::store::SessionStore;

/// What a prune did.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct PruneReport {
    /// Sessions removed, including directories that held no `session.json`.
    pub removed: Vec<String>,
    /// Idle sessions kept because they hold named checkpoints, which only
    /// `sessions rm --with-checkpoints` deletes.
    pub kept: Vec<String>,
    /// Idle sessions skipped because another process held their lock past
    /// the wait; the next prune tries them again.
    pub busy: Vec<String>,
}

impl Bundle {
    /// Removes sessions whose last recorded use is older than `unused_for`,
    /// counted back from now by the bundle's clock. Each is judged again
    /// under its lock, which it waits up to `wait` for; one still held after
    /// that is skipped and reported as busy.
    pub fn prune(&self, unused_for: Duration, wait: Duration) -> Result<PruneReport, StoreError> {
        let cutoff = self.now().saturating_sub(unused_for.as_secs());
        let idle = |store: &SessionStore| store.last_used().is_some_and(|t| t < cutoff);
        let mut report = PruneReport::default();
        self.sweep_removed();
        for name in self.session_dirs()? {
            // A directory whose name no session could have isn't ours.
            let Ok(store) = self.session(&name) else {
                continue;
            };
            if !idle(&store) {
                continue;
            }
            let _lock = match store.acquire(wait) {
                Ok(lock) => lock,
                Err(StoreError::Busy { .. }) => {
                    report.busy.push(name);
                    continue;
                }
                Err(e) => return Err(e),
            };
            // Judged again: a request may have used it while we waited.
            if !idle(&store) || !store.is_dir() {
                continue;
            }
            if !store.checkpoint_names()?.is_empty() {
                report.kept.push(name);
                continue;
            }
            store.remove_locked()?;
            report.removed.push(name);
        }
        Ok(report)
    }
}

/// Parses a pruning duration: a whole number and a unit, `45s`, `30m`, `12h`
/// or `7d`.
pub fn parse_duration(text: &str) -> Result<Duration, StoreError> {
    let text = text.trim();
    let bad = || StoreError::BadDuration {
        text: text.to_string(),
    };
    let unit = text.chars().next_back().ok_or_else(bad)?;
    let digits = &text[..text.len() - unit.len_utf8()];
    if digits.is_empty() || !digits.bytes().all(|b| b.is_ascii_digit()) {
        return Err(bad());
    }
    let n: u64 = digits.parse().map_err(|_| bad())?;
    let seconds_per: u64 = match unit {
        's' => 1,
        'm' => 60,
        'h' => 60 * 60,
        'd' => 24 * 60 * 60,
        _ => return Err(bad()),
    };
    n.checked_mul(seconds_per)
        .map(Duration::from_secs)
        .ok_or_else(bad)
}
#[cfg(test)]
mod tests {
    use std::fs;

    use super::*;
    use crate::test_fixture::{DAY, Fixture, backdate};

    const SHORT: Duration = Duration::from_millis(20);

    #[test]
    fn durations_need_a_unit() {
        assert_eq!(parse_duration("30m").unwrap(), Duration::from_secs(1800));
        assert_eq!(parse_duration("7d").unwrap(), Duration::from_secs(604_800));
        assert!(parse_duration("7").is_err());
        assert!(parse_duration("d").is_err());
        assert!(parse_duration("1w").is_err());
    }

    #[test]
    fn a_duration_ending_in_a_multibyte_character_is_an_error_not_a_panic() {
        for text in ["7é", "é", "1日"] {
            assert!(
                matches!(parse_duration(text), Err(StoreError::BadDuration { .. })),
                "{text}"
            );
        }
    }

    #[test]
    fn a_duration_too_long_to_count_is_an_error_not_a_wrap() {
        for text in ["213503982334602d", "99999999999999999999s", "+5m"] {
            assert!(
                matches!(parse_duration(text), Err(StoreError::BadDuration { .. })),
                "{text}"
            );
        }
    }

    #[test]
    fn pruning_removes_sessions_unused_for_longer_than_asked() {
        let f = Fixture::new("prune");
        let store = f.demo();
        store.backdate_use(10 * DAY);
        let kept = f.bundle.prune(30 * DAY, SHORT).unwrap();
        assert!(kept.removed.is_empty(), "{kept:?}");
        let pruned = f.bundle.prune(DAY, SHORT).unwrap();
        assert_eq!(pruned.removed, vec!["demo".to_string()]);
        assert!(f.bundle.session_names().unwrap().is_empty());
    }

    #[test]
    fn a_session_used_only_for_reads_is_not_idle() {
        let f = Fixture::new("prune-reads");
        let store = f.demo();
        store.backdate_use(10 * DAY);
        backdate(&store.dir().join("session.json"), 10 * DAY);
        // A read-only request takes the lock and writes nothing else.
        drop(store.lock(SHORT).unwrap());
        assert!(f.bundle.prune(DAY, SHORT).unwrap().removed.is_empty());
        assert!(store.exists());
    }

    #[test]
    fn pruning_keeps_idle_sessions_that_hold_named_checkpoints_and_says_so() {
        let f = Fixture::new("prune-checkpoints");
        let store = f.demo();
        let mut machine = store.machine(&store.read_meta().unwrap()).unwrap();
        store
            .write_checkpoint("keep", &machine.capture(Default::default()))
            .unwrap();
        store.backdate_use(10 * DAY);
        let report = f.bundle.prune(DAY, SHORT).unwrap();
        assert!(report.removed.is_empty(), "{report:?}");
        assert_eq!(report.kept, vec!["demo".to_string()]);
        assert!(store.exists());
    }

    #[test]
    fn pruning_survives_a_directory_with_no_session_file() {
        let f = Fixture::new("prune-broken");
        let live = f.demo();
        let broken = f.bundle.sessions_dir().join("typo");
        fs::create_dir_all(&broken).unwrap();
        fs::write(broken.join("lock"), b"").unwrap();
        backdate(&broken, 10 * DAY);
        let report = f.bundle.prune(DAY, SHORT).unwrap();
        assert_eq!(report.removed, vec!["typo".to_string()]);
        assert!(!broken.exists());
        assert!(live.exists());
    }

    #[test]
    fn pruning_skips_a_busy_session_and_reports_it_beside_the_ones_it_removed() {
        let f = Fixture::new("prune-busy");
        let store = f.demo();
        let idle = f.bundle.session("idle").unwrap();
        idle.create(&f.source()).unwrap();
        // "busy" sorts first: a prune that stops at it removes nothing.
        let busy = f.bundle.session("busy").unwrap();
        busy.create(&f.source()).unwrap();
        let held = busy.lock(SHORT).unwrap();
        for s in [&store, &idle, &busy] {
            s.backdate_use(10 * DAY);
        }
        let report = f.bundle.prune(DAY, SHORT).unwrap();
        assert_eq!(report.removed, vec!["demo".to_string(), "idle".to_string()]);
        assert_eq!(report.busy, vec!["busy".to_string()]);
        assert!(busy.exists());
        drop(held);
    }
}
