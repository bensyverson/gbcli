//! Where a project's bundle lives and what it holds (design §1): sessions,
//! the boot cache, and the gap ledger. `.gbcli/` is found by walking up from
//! the working directory, like `.git`; `GBCLI_DIR` overrides it.

use std::env;
use std::fs;
use std::path::{Component, Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use gbcli_protocol::{Rev, SessionRow};

use crate::error::StoreError;
use crate::store::SessionStore;

/// The project's bundle: the `.gbcli/` directory.
#[derive(Clone, Debug)]
pub struct Bundle {
    root: PathBuf,
}

impl Bundle {
    /// Opens an existing bundle: `GBCLI_DIR` when it is set, else the first
    /// `.gbcli/` found searching upwards from `start`.
    pub fn open(start: &Path) -> Result<Bundle, StoreError> {
        Bundle::locate(start, gbcli_dir().as_deref())
    }

    /// Opens a bundle, creating it if there is none: `GBCLI_DIR` when it is
    /// set, else an existing `.gbcli/` above `start`, else `start/.gbcli/`.
    pub fn open_or_create(start: &Path) -> Result<Bundle, StoreError> {
        Bundle::open_or_create_in(start, gbcli_dir().as_deref())
    }

    /// [`Bundle::open`] with `GBCLI_DIR`'s value passed in, so tests don't
    /// depend on the environment. Never creates anything; no bundle is
    /// `NoBundle`, naming where it looked.
    pub fn open_in(start: &Path, gbcli_dir: Option<&Path>) -> Result<Bundle, StoreError> {
        Bundle::locate(start, gbcli_dir)
    }

    pub(crate) fn locate(start: &Path, gbcli_dir: Option<&Path>) -> Result<Bundle, StoreError> {
        if let Some(dir) = gbcli_dir {
            let root = absolute(dir);
            if root.is_dir() {
                return Ok(Bundle { root });
            }
            return Err(StoreError::NoBundle {
                start: absolute(start),
                gbcli_dir: Some(root),
            });
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
        Err(StoreError::NoBundle {
            start,
            gbcli_dir: None,
        })
    }

    /// [`Bundle::open_or_create`] with `GBCLI_DIR`'s value passed in, for callers
    /// (tests above all) that must not depend on the environment.
    pub fn open_or_create_in(start: &Path, gbcli_dir: Option<&Path>) -> Result<Bundle, StoreError> {
        if let Some(dir) = gbcli_dir {
            let root = absolute(dir);
            fs::create_dir_all(&root).map_err(|source| StoreError::NoRoot {
                start: root.clone(),
                source,
            })?;
            return Ok(Bundle { root });
        }
        match Bundle::locate(start, None) {
            Ok(bundle) => Ok(bundle),
            Err(StoreError::NoBundle { .. }) => {
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

    /// The project directory: the bundle directory's parent, whatever the
    /// bundle directory is called. Under `GBCLI_DIR=/work/state` the project
    /// is `/work`. Stored ROM and `.sym` paths are relative to it, and so are
    /// relative paths in a [`crate::RomSource`].
    pub fn project(&self) -> &Path {
        self.root.parent().unwrap_or(&self.root)
    }

    pub fn sessions_dir(&self) -> PathBuf {
        self.root.join("sessions")
    }

    /// Session lock files live here, outside the session directories, so
    /// removing a session never unlinks a lock another process holds or
    /// waits on. They are never deleted.
    pub(crate) fn locks_dir(&self) -> PathBuf {
        self.root.join("locks")
    }

    /// A session store by name; the session need not exist yet.
    pub fn session(&self, name: &str) -> Result<SessionStore, StoreError> {
        SessionStore::new(self.clone(), name)
    }

    /// Where a removal moves a session directory before deleting it, so a
    /// removal killed midway never leaves part of a session under its name.
    pub(crate) fn removed_dir(&self) -> PathBuf {
        self.root.join("removed")
    }

    /// Deletes what removals left in [`Bundle::removed_dir`], best effort.
    pub(crate) fn sweep_removed(&self) {
        let _ = fs::remove_dir_all(self.removed_dir());
    }

    /// The sessions: directories with a valid name and a `session.json`,
    /// sorted. Anything else under `sessions/` is not a session.
    pub fn session_names(&self) -> Result<Vec<String>, StoreError> {
        Ok(self
            .session_dirs()?
            .into_iter()
            .filter(|name| self.session(name).is_ok_and(|s| s.exists()))
            .collect())
    }

    /// Every directory under `sessions/`, sorted, session or not.
    pub(crate) fn session_dirs(&self) -> Result<Vec<String>, StoreError> {
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

    /// One row per session, as `gbcli sessions` prints; directories that
    /// aren't sessions are left out.
    pub fn list(&self) -> Result<Vec<SessionRow>, StoreError> {
        let mut rows = Vec::new();
        for name in self.session_names()? {
            let Ok(store) = self.session(&name) else {
                continue;
            };
            let Ok(meta) = store.read_meta() else {
                continue;
            };
            rows.push(store.row(&meta)?);
        }
        Ok(rows)
    }

    pub fn gaps_path(&self) -> PathBuf {
        self.root.join("gaps.jsonl")
    }

    pub fn cache_dir(&self) -> PathBuf {
        self.root.join("cache")
    }
}

fn gbcli_dir() -> Option<PathBuf> {
    env::var_os("GBCLI_DIR").map(PathBuf::from)
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

/// Host time in Unix seconds. Only logs, listing and pruning read it; emulation
/// never does (design §10).
pub fn now_secs() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |d| d.as_secs())
}

/// A session name: `[a-z0-9-]{1,40}`, not starting with `-`, which a CLI would read as a flag.
pub fn validate_name(name: &str) -> Result<(), StoreError> {
    let ok = (1..=40).contains(&name.len())
        && !name.starts_with('-')
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

    fn scratch(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("gbcli-bundle-{}-{name}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn names_are_lowercase_digits_and_dashes() {
        for name in ["dx", "demo-1", "a", "a-", &"x".repeat(40)] {
            assert!(validate_name(name).is_ok(), "{name}");
        }
        for name in ["", "DX", "demo_1", "a b", &"x".repeat(41)] {
            assert!(validate_name(name).is_err(), "{name}");
        }
    }

    #[test]
    fn a_name_cannot_start_with_a_dash_a_cli_would_read_as_a_flag() {
        for name in ["-", "-x", "--rom"] {
            assert!(validate_name(name).is_err(), "{name}");
        }
    }

    #[test]
    fn finding_a_bundle_walks_up_from_a_nested_directory() {
        let tmp = scratch("walk-up");
        let nested = tmp.join("a").join("b");
        fs::create_dir_all(&nested).unwrap();
        let root = tmp.join(".gbcli");
        fs::create_dir_all(&root).unwrap();
        let found = Bundle::locate(&nested, None).unwrap();
        assert_eq!(found.root(), root.as_path());
        assert_eq!(found.project(), tmp.as_path());
        fs::remove_dir_all(&tmp).unwrap();
    }

    #[test]
    fn gbcli_dir_wins_over_the_walk_up_and_its_parent_is_the_project() {
        let tmp = scratch("gbcli-dir");
        fs::create_dir_all(tmp.join(".gbcli")).unwrap();
        let state = tmp.join("elsewhere").join("state");
        fs::create_dir_all(&state).unwrap();
        let found = Bundle::locate(&tmp, Some(&state)).unwrap();
        assert_eq!(found.root(), state.as_path());
        assert_eq!(found.project(), tmp.join("elsewhere").as_path());
        fs::remove_dir_all(&tmp).unwrap();
    }

    #[test]
    fn a_gbcli_dir_that_does_not_exist_is_refused_by_open_and_made_by_open_or_create() {
        let tmp = scratch("gbcli-dir-missing");
        let state = tmp.join("state");
        assert!(matches!(
            Bundle::locate(&tmp, Some(&state)),
            Err(StoreError::NoBundle { .. })
        ));
        let made = Bundle::open_or_create_in(&tmp, Some(&state)).unwrap();
        assert_eq!(made.root(), state.as_path());
        assert!(state.is_dir());
        fs::remove_dir_all(&tmp).unwrap();
    }

    #[test]
    fn a_relative_gbcli_dir_is_made_absolute_so_the_project_is_a_real_directory() {
        let found = Bundle::locate(Path::new("/"), Some(Path::new("."))).unwrap();
        assert!(found.root().is_absolute(), "{:?}", found.root());
        assert!(found.project().is_absolute(), "{:?}", found.project());
    }
}
