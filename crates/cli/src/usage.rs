//! A malformed invocation: exit 2, with what was wrong and the next command.

use gbcli_protocol::{ErrorBody, ErrorCode};

/// What was wrong with the command line, and what to type instead.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Usage {
    pub message: String,
    pub remedy: String,
}

impl Usage {
    pub fn new(message: impl Into<String>, remedy: impl Into<String>) -> Usage {
        Usage {
            message: message.into(),
            remedy: remedy.into(),
        }
    }

    /// The protocol's spelling of the same failure, so every error the CLI
    /// prints has one shape: `bad_request`, exit 2.
    pub fn body(&self) -> ErrorBody {
        ErrorBody::new(ErrorCode::BadRequest, &self.message, &self.remedy)
    }
}

/// The candidates closest to `word`, nearest first: what "did you mean"
/// offers. Close means a small edit distance for the word's length, or a
/// shared prefix of at least three letters.
pub fn near<'a>(word: &str, candidates: impl IntoIterator<Item = &'a str>) -> Vec<&'a str> {
    let limit = (word.chars().count() / 3).max(1);
    let mut scored: Vec<(usize, &str)> = candidates
        .into_iter()
        .filter_map(|c| {
            let d = distance(word, c);
            let prefix = word.len() >= 3 && (c.starts_with(word) || word.starts_with(c));
            (d <= limit || prefix).then_some((d, c))
        })
        .collect();
    scored.sort();
    scored.dedup();
    scored.into_iter().take(3).map(|(_, c)| c).collect()
}

/// "did you mean `a` or `b`? " when there are near misses, else "".
pub fn did_you_mean(near: &[&str], prefix: &str) -> String {
    match near {
        [] => String::new(),
        [one] => format!("did you mean `{prefix}{one}`? "),
        many => {
            let quoted: Vec<String> = many.iter().map(|n| format!("`{prefix}{n}`")).collect();
            format!("did you mean {}? ", quoted.join(" or "))
        }
    }
}

/// Levenshtein distance over chars.
fn distance(a: &str, b: &str) -> usize {
    let b: Vec<char> = b.chars().collect();
    let mut row: Vec<usize> = (0..=b.len()).collect();
    for (i, ca) in a.chars().enumerate() {
        let mut prev = row[0];
        row[0] = i + 1;
        for (j, cb) in b.iter().enumerate() {
            let old = row[j + 1];
            row[j + 1] = if ca == *cb {
                prev
            } else {
                1 + prev.min(row[j]).min(row[j + 1])
            };
            prev = old;
        }
    }
    row[b.len()]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn near_misses_are_typos_and_prefixes_nearest_first() {
        let verbs = ["run", "read", "restore", "rm", "step"];
        assert_eq!(near("rn", verbs), vec!["rm", "run"]);
        assert_eq!(near("resto", verbs), vec!["restore"]);
        assert_eq!(near("frame", ["frames", "from", "at"]), vec!["frames"]);
        assert!(near("zzzzzz", verbs).is_empty());
    }

    #[test]
    fn did_you_mean_quotes_each_candidate() {
        assert_eq!(did_you_mean(&[], "--"), "");
        assert_eq!(did_you_mean(&["frames"], "--"), "did you mean `--frames`? ");
        assert_eq!(
            did_you_mean(&["rm", "run"], ""),
            "did you mean `rm` or `run`? "
        );
    }
}
