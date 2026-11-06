//! The baseline file: per row and model, pass, fail or skip, plus how it was made.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use crate::judge::Outcome;
use crate::rules::Verdict;

/// Counts of one suite's outcomes.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Tally {
    pub pass: usize,
    pub fail: usize,
    pub skip: usize,
}

/// What a baseline says it was made from.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Header {
    pub backend: String,
    pub backend_commit: String,
    /// UTC, `YYYY-MM-DD`.
    pub date: String,
    pub command: String,
}

/// Per-suite tallies, in suite order.
pub fn summarize(outcomes: &[Outcome]) -> BTreeMap<String, Tally> {
    let mut out: BTreeMap<String, Tally> = BTreeMap::new();
    for o in outcomes {
        let tally = out.entry(o.suite.clone()).or_default();
        match o.verdict {
            Verdict::Pass => tally.pass += 1,
            Verdict::Fail(_) => tally.fail += 1,
            Verdict::Skip(_) => tally.skip += 1,
        }
    }
    out
}

/// The baseline as JSON with one outcome per line, so diffs stay readable.
pub fn render(header: &Header, outcomes: &[Outcome]) -> String {
    let mut s = String::from("{\n");
    s += &format!("\"header\": {},\n", json(header));
    s += "\"summary\": {\n";
    let summary = summarize(outcomes);
    let lines: Vec<String> = summary
        .iter()
        .map(|(suite, t)| format!("  {}: {}", json(suite), json(t)))
        .collect();
    s += &lines.join(",\n");
    s += "\n},\n\"results\": [\n";
    let lines: Vec<String> = outcomes.iter().map(|o| format!("  {}", json(o))).collect();
    s += &lines.join(",\n");
    s += "\n]\n}\n";
    s
}

/// One line per (id, model) whose status differs between two baselines.
pub fn differences(old: &str, new: &[Outcome]) -> Result<Vec<String>, String> {
    #[derive(Deserialize)]
    struct File {
        results: Vec<Line>,
    }
    #[derive(Deserialize)]
    struct Line {
        id: String,
        model: String,
        status: String,
    }
    let file: File = serde_json::from_str(old).map_err(|e| format!("baseline: {e}"))?;
    let old: BTreeMap<(String, String), String> = file
        .results
        .into_iter()
        .map(|l| ((l.id, l.model), l.status))
        .collect();
    let mut out = Vec::new();
    for o in new {
        let status = match o.verdict {
            Verdict::Pass => "pass",
            Verdict::Fail(_) => "fail",
            Verdict::Skip(_) => "skip",
        };
        match old.get(&(o.id.clone(), o.model.to_string())) {
            Some(was) if was == status => {}
            Some(was) => out.push(format!("{} on {}: {was} -> {status}", o.id, o.model)),
            None => out.push(format!("{} on {}: new, {status}", o.id, o.model)),
        }
    }
    Ok(out)
}

fn json<T: Serialize + ?Sized>(value: &T) -> String {
    serde_json::to_string(value).expect("baseline values serialize")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn outcome(id: &str, suite: &str, verdict: Verdict) -> Outcome {
        Outcome {
            id: id.into(),
            suite: suite.into(),
            model: "dmg-B",
            substitute_for: None,
            verdict,
            emulated_s: Some(1.5),
            alternatives: Vec::new(),
        }
    }

    fn sample() -> Vec<Outcome> {
        vec![
            outcome("a.gb", "mooneye", Verdict::Pass),
            outcome("b.gb", "mooneye", Verdict::fail("BCDEHL wrong")),
            outcome("c.gb", "blargg", Verdict::Skip("manual".into())),
        ]
    }

    #[test]
    fn summary_counts_each_status_per_suite() {
        let summary = summarize(&sample());
        assert_eq!(
            summary["mooneye"],
            Tally {
                pass: 1,
                fail: 1,
                skip: 0
            }
        );
        assert_eq!(
            summary["blargg"],
            Tally {
                pass: 0,
                fail: 0,
                skip: 1
            }
        );
    }

    #[test]
    fn render_writes_valid_json_one_result_per_line() {
        let header = Header {
            backend: "sameboy".into(),
            backend_commit: "abc".into(),
            date: "2026-11-06".into(),
            command: "cargo run".into(),
        };
        let text = render(&header, &sample());
        let value: serde_json::Value = serde_json::from_str(&text).unwrap();
        assert_eq!(value["header"]["backend_commit"], "abc");
        assert_eq!(value["results"][1]["status"], "fail");
        assert_eq!(value["results"][1]["detail"], "BCDEHL wrong");
        assert_eq!(value["results"][0].get("detail"), None);
        assert!(text.contains("\n  {\"id\":\"b.gb\""), "{text}");
    }

    #[test]
    fn differences_name_changed_and_new_rows() {
        let header = Header {
            backend: "x".into(),
            backend_commit: "x".into(),
            date: "x".into(),
            command: "x".into(),
        };
        let old = render(&header, &sample()[..2]);
        let mut new = sample();
        new[0].verdict = Verdict::fail("regressed");
        let diffs = differences(&old, &new).unwrap();
        assert_eq!(
            diffs,
            vec!["a.gb on dmg-B: pass -> fail", "c.gb on dmg-B: new, skip"]
        );
    }
}
