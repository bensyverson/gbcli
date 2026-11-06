//! Where a project's bundle lives and what it holds (design §1): sessions,
//! the boot cache, and the gap ledger. `.gbcli/` is found by walking up from
//! the working directory, like `.git`; `GBCLI_DIR` overrides it.

use std::env;
use std::fs;
use std::path::{Component, Path, PathBuf};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use gbcli_protocol::{Rev, SessionRow};

use crate::error::StoreError;
use crate::store::SessionStore;

/// The project's bundle: the `.gbcli/` directory.
#[derive(Clone, Debug)]
pub struct Bundle {
    root: PathBuf,
}

impl Bundle {
    /// Opens an existing bundle, searching upwards from `start`.
    pub fn open(start: &Path) -> Result<Bundle, StoreError> {
        if let Some(dir) = env::var_os("GBCLI_DIR") {
            let root = PathBuf::from(dir);
            if root.is_dir() {
                return Ok(Bundle { root });
            }
            return Err(no_root(
                start,
                format!(
                    "GBCLI_DIR names {} which is not a directory",
                    root.display()
                ),
            ));
        }
        let start = absolute(start);
        let mut dir = start.as_path();
        loop {
            let candidate = dir.join(".gbcli");
            if candidate.is_dir() {
                return Ok(Bundle { root: candidate });
            }
            let Some(parent) = dir.parent() else {
                break;
            };
            dir = parent;
        }
        Err(no_root(
            &start,
            "no .gbcli/ in this directory or above".to_string(),
        ))
    }

    /// Opens a bundle, creating `.gbcli/` under `start` if there is none.
    pub fn open_or_create(start: &Path) -> Result<Bundle, StoreError> {
        if let Some(dir) = env::var_os("GBCLI_DIR") {
            let root = PathBuf::from(dir);
            fs::create_dir_all(&root).map_err(|source| StoreError::NoRoot {
                start: root.clone(),
                source,
            })?;
            return Ok(Bundle { root });
        }
        match Bundle::open(start) {
            Ok(bundle) => Ok(bundle),
            Err(StoreError::NoRoot { .. }) => {
                let root = absolute(start).join(".gbcli");
                fs::create_dir_all(&root).map_err(|source| StoreError::NoRoot {
                    start: root.clone(),
                    source,
                })?;
                Ok(Bundle { root })
            }
            Err(e) => Err(e),
        }
    }

    /// The `.gbcli/` directory itself.
    pub fn root(&self) -> &Path {
        &self.root
    }

    /// The project directory: `.gbcli/`'s parent. Stored paths are relative to it.
    pub fn project(&self) -> &Path {
        self.root.parent().unwrap_or(&self.root)
    }

    pub fn sessions_dir(&self) -> PathBuf {
        self.root.join("sessions")
    }

    /// A session store by name; the session need not exist yet.
    pub fn session(&self, name: &str) -> Result<SessionStore, StoreError> {
        SessionStore::new(self.clone(), name)
    }

    /// Session names, sorted.
    pub fn session_names(&self) -> Result<Vec<String>, StoreError> {
        let dir = self.sessions_dir();
        let entries = match fs::read_dir(&dir) {
            Ok(entries) => entries,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(Vec::new()),
            Err(source) => {
                return Err(StoreError::Io { path: dir, source });
            }
        };
        let mut names = Vec::new();
        for entry in entries {
            let entry = entry.map_err(|source| StoreError::Io {
                path: dir.clone(),
                source,
            })?;
            if entry.file_type().is_ok_and(|t| t.is_dir())
                && let Some(name) = entry.file_name().to_str()
            {
                names.push(name.to_string());
            }
        }
        names.sort();
        Ok(names)
    }

    /// One row per session, as `gbcli sessions` prints.
    pub fn list(&self) -> Result<Vec<SessionRow>, StoreError> {
        let mut rows = Vec::new();
        for name in self.session_names()? {
            let store = self.session(&name)?;
            let Ok(meta) = store.read_meta() else {
                continue;
            };
            rows.push(store.row(&meta)?);
        }
        Ok(rows)
    }

    /// Removes sessions unused for longer than `unused_for`; returns their names.
    pub fn prune(&self, unused_for: Duration) -> Result<Vec<String>, StoreError> {
        let cutoff = now_secs().saturating_sub(unused_for.as_secs());
        let mut removed = Vec::new();
        for name in self.session_names()? {
            let store = self.session(&name)?;
            if store.last_used()? < cutoff {
                store.remove(true)?;
                removed.push(name);
            }
        }
        Ok(removed)
    }

    pub fn gaps_path(&self) -> PathBuf {
        self.root.join("gaps.jsonl")
    }

    pub fn cache_dir(&self) -> PathBuf {
        self.root.join("cache")
    }
}

/// Absolute without requiring the path to exist.
fn absolute(path: &Path) -> PathBuf {
    if path.is_absolute() {
        return normalize(path);
    }
    env::current_dir()
        .map(|cwd| normalize(&cwd.join(path)))
        .unwrap_or_else(|_| path.to_path_buf())
}

fn normalize(path: &Path) -> PathBuf {
    let mut out = PathBuf::new();
    for part in path.components() {
        match part {
            Component::ParentDir => {
                out.pop();
            }
            Component::CurDir => {}
            other => out.push(other.as_os_str()),
        }
    }
    out
}

fn no_root(start: &Path, reason: String) -> StoreError {
    StoreError::NoRoot {
        start: start.to_path_buf(),
        source: std::io::Error::new(std::io::ErrorKind::NotFound, reason),
    }
}

/// Host time in Unix seconds. Only logs, listing and pruning read it; emulation
/// never does (design §10).
pub fn now_secs() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |d| d.as_secs())
}

/// Parses a pruning duration: `30m`, `12h`, `7d`, `45s`.
pub fn parse_duration(text: &str) -> Result<Duration, StoreError> {
    let text = text.trim();
    let (digits, unit) = text.split_at(text.len().saturating_sub(1));
    let n: u64 = digits.parse().map_err(|_| StoreError::Configuration {
        what: format!("{text:?} is not a duration; try 30m, 12h or 7d"),
    })?;
    let unit = match unit {
        "s" => Duration::from_secs(n),
        "m" => Duration::from_secs(n * 60),
        "h" => Duration::from_secs(n * 60 * 60),
        "d" => Duration::from_secs(n * 60 * 60 * 24),
        _ => {
            return Err(StoreError::Configuration {
                what: format!("{text:?} needs a unit: s, m, h or d"),
            });
        }
    };
    Ok(unit)
}

/// A session name: `[a-z0-9-]{1,40}`.
pub fn validate_name(name: &str) -> Result<(), StoreError> {
    let ok = (1..=40).contains(&name.len())
        && name
            .chars()
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-');
    if ok {
        Ok(())
    } else {
        Err(StoreError::BadSessionName {
            name: name.to_string(),
        })
    }
}

/// The revision a session would have after its next head write.
pub fn next(rev: Rev) -> Rev {
    Rev(rev.0 + 1)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn names_are_lowercase_digits_and_dashes() {
        for name in ["dx", "demo-1", "a", &"x".repeat(40)] {
            assert!(validate_name(name).is_ok(), "{name}");
        }
        for name in ["", "DX", "demo_1", "a b", &"x".repeat(41)] {
            assert!(validate_name(name).is_err(), "{name}");
        }
    }

    #[test]
    fn durations_need_a_unit() {
        assert_eq!(parse_duration("30m").unwrap(), Duration::from_secs(1800));
        assert_eq!(parse_duration("7d").unwrap(), Duration::from_secs(604_800));
        assert!(parse_duration("7").is_err());
        assert!(parse_duration("d").is_err());
        assert!(parse_duration("1w").is_err());
    }

    #[test]
    fn finding_a_bundle_walks_up_from_a_nested_directory() {
        let tmp = std::env::temp_dir().join(format!("gbcli-bundle-test-{}", std::process::id()));
        let nested = tmp.join("a").join("b");
        fs::create_dir_all(&nested).unwrap();
        let root = tmp.join(".gbcli");
        fs::create_dir_all(&root).unwrap();
        assert_eq!(Bundle::open(&nested).unwrap().root(), root.as_path());
        assert_eq!(Bundle::open(&nested).unwrap().project(), tmp.as_path());
        fs::remove_dir_all(&tmp).unwrap();
    }
}
