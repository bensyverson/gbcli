//! The gap ledger (design §10): every `Unsupported` the core answers appends
//! one line, and `gbcli gaps` counts them by feature.

use std::collections::BTreeMap;
use std::fs;

use gbcli_core::Feature;
use gbcli_protocol::{GapCount, GapLine};

use crate::bundle::{Bundle, now_secs};
use crate::error::StoreError;
use crate::io::append;

impl Bundle {
    /// Adds one gap to the project's ledger.
    pub fn append_gap(
        &self,
        session: Option<&str>,
        op: &str,
        feature: Feature,
        reason: &str,
    ) -> Result<(), StoreError> {
        let line = GapLine {
            at: now_secs(),
            session: session.map(str::to_string),
            op: op.to_string(),
            feature: feature.name().to_string(),
            reason: reason.to_string(),
        };
        let mut text = serde_json::to_string(&line).expect("a gap line serializes");
        text.push('\n');
        append(&self.gaps_path(), &text)
    }

    /// Counts by feature across the project, most frequent first.
    pub fn gap_counts(&self) -> Result<Vec<GapCount>, StoreError> {
        self.gap_counts_where(|_| true)
    }

    /// Counts by feature for one session's gaps, most frequent first.
    pub fn session_gap_counts(&self, session: &str) -> Result<Vec<GapCount>, StoreError> {
        self.gap_counts_where(|line| line.session.as_deref() == Some(session))
    }

    fn gap_counts_where(
        &self,
        keep: impl Fn(&GapLine) -> bool,
    ) -> Result<Vec<GapCount>, StoreError> {
        let path = self.gaps_path();
        let text = match fs::read_to_string(&path) {
            Ok(text) => text,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(Vec::new()),
            Err(source) => return Err(StoreError::Io { path, source }),
        };
        let mut counts: BTreeMap<String, u64> = BTreeMap::new();
        for line in text.lines() {
            if let Ok(line) = serde_json::from_str::<GapLine>(line)
                && keep(&line)
            {
                *counts.entry(line.feature).or_default() += 1;
            }
        }
        let mut rows: Vec<GapCount> = counts
            .into_iter()
            .map(|(feature, count)| GapCount { feature, count })
            .collect();
        rows.sort_by(|a, b| b.count.cmp(&a.count).then(a.feature.cmp(&b.feature)));
        Ok(rows)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn gaps_count_by_feature_most_frequent_first() {
        let root = std::env::temp_dir().join(format!("gbcli-gaps-{}", std::process::id()));
        let _ = fs::remove_dir_all(&root);
        fs::create_dir_all(&root).unwrap();
        let bundle = Bundle::open_or_create_in(&root, None).unwrap();
        bundle
            .append_gap(
                Some("demo"),
                "step",
                Feature::DotStep,
                "needs a dot counter",
            )
            .unwrap();
        bundle
            .append_gap(Some("demo"), "run", Feature::DotStep, "needs a dot counter")
            .unwrap();
        bundle
            .append_gap(None, "read", Feature::ExtraOam, "FEA0-FEFF")
            .unwrap();
        let rows = bundle.gap_counts().unwrap();
        assert_eq!(rows[0].feature, "dot_step");
        assert_eq!(rows[0].count, 2);
        assert_eq!(rows[1].feature, "extra_oam");
        assert_eq!(rows[1].count, 1);
        let demo = bundle.session_gap_counts("demo").unwrap();
        assert_eq!(demo.len(), 1, "{demo:?}");
        fs::remove_dir_all(&root).unwrap();
    }
}
