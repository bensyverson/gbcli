//! Opening a project's bundle on disk: `GBCLI_DIR`, or the first `.gbcli/`
//! found walking up from the working directory, like `.git`. The bundle is
//! then a [`FileStore`] rooted at the project, the bundle directory's parent.

use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use super::{FileStore, SystemClock, normalize};
use crate::bundle::Bundle;
use crate::error::StoreError;
use crate::project_store::Key;

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
                return Ok(Bundle::on_disk(root));
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
                return Ok(Bundle::on_disk(candidate));
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
            return Ok(Bundle::on_disk(root));
        }
        match Bundle::locate(start, None) {
            Ok(bundle) => Ok(bundle),
            Err(StoreError::NoBundle { .. }) => {
                let root = absolute(start).join(".gbcli");
                fs::create_dir_all(&root).map_err(|source| StoreError::NoRoot {
                    start: root.clone(),
                    source,
                })?;
                Ok(Bundle::on_disk(root))
            }
            Err(e) => Err(e),
        }
    }

    /// The bundle directory `root` in a [`FileStore`] rooted at its parent,
    /// the project, whatever the bundle directory is called: under
    /// `GBCLI_DIR=/work/state` the project is `/work` and the bundle's keys
    /// start `state/`.
    fn on_disk(root: PathBuf) -> Bundle {
        let (project, dir) = match (root.parent(), root.file_name()) {
            (Some(parent), Some(name)) => (parent.to_path_buf(), Key::new(name.to_string_lossy())),
            _ => (root.clone(), Key::root()),
        };
        Bundle::new(
            Arc::new(FileStore::new(project)),
            dir,
            Arc::new(SystemClock),
        )
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

#[cfg(test)]
mod tests {
    use super::*;

    struct Scratch(PathBuf);

    impl Scratch {
        fn new(name: &str) -> Scratch {
            let dir =
                std::env::temp_dir().join(format!("gbcli-bundle-{}-{name}", std::process::id()));
            let _ = fs::remove_dir_all(&dir);
            fs::create_dir_all(&dir).unwrap();
            Scratch(dir)
        }
    }

    impl Drop for Scratch {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }

    #[test]
    fn finding_a_bundle_walks_up_from_a_nested_directory() {
        let s = Scratch::new("walk-up");
        let tmp = &s.0;
        let nested = tmp.join("a").join("b");
        fs::create_dir_all(&nested).unwrap();
        let root = tmp.join(".gbcli");
        fs::create_dir_all(&root).unwrap();
        let found = Bundle::locate(&nested, None).unwrap();
        assert_eq!(found.root(), root.as_path());
        assert_eq!(found.project(), tmp.as_path());
    }

    #[test]
    fn gbcli_dir_wins_over_the_walk_up_and_its_parent_is_the_project() {
        let s = Scratch::new("gbcli-dir");
        let tmp = &s.0;
        fs::create_dir_all(tmp.join(".gbcli")).unwrap();
        let state = tmp.join("elsewhere").join("state");
        fs::create_dir_all(&state).unwrap();
        let found = Bundle::locate(tmp, Some(&state)).unwrap();
        assert_eq!(found.root(), state.as_path());
        assert_eq!(found.project(), tmp.join("elsewhere").as_path());
    }

    #[test]
    fn a_gbcli_dir_that_does_not_exist_is_refused_by_open_and_made_by_open_or_create() {
        let s = Scratch::new("gbcli-dir-missing");
        let tmp = &s.0;
        let state = tmp.join("state");
        assert!(matches!(
            Bundle::locate(tmp, Some(&state)),
            Err(StoreError::NoBundle { .. })
        ));
        let made = Bundle::open_or_create_in(tmp, Some(&state)).unwrap();
        assert_eq!(made.root(), state.as_path());
        assert!(state.is_dir());
    }

    #[test]
    fn a_relative_gbcli_dir_is_made_absolute_so_the_project_is_a_real_directory() {
        let found = Bundle::locate(Path::new("/"), Some(Path::new("."))).unwrap();
        assert!(found.root().is_absolute(), "{:?}", found.root());
        assert!(found.project().is_absolute(), "{:?}", found.project());
    }
}
