//! Filesystem helpers the store shares. Writes are atomic against a killed
//! process (temp file, then rename) but not against power loss: nothing is
//! fsynced (design §1, "Durability").

use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

use crate::error::StoreError;

/// Distinguishes temp files written by threads of one process.
static TEMP_SEQ: AtomicU64 = AtomicU64::new(0);

/// A closure that wraps an `io::Error` with the path it concerns.
pub(crate) fn io_at(path: &Path) -> impl FnOnce(std::io::Error) -> StoreError + '_ {
    move |source| StoreError::Io {
        path: path.to_path_buf(),
        source,
    }
}

/// Writes `bytes` to a temp file beside `path`, then renames it over `path`.
/// The parent must exist: creating it here could resurrect a session another
/// process just removed. A failed write or rename removes the temp file.
pub(crate) fn atomic_write(path: &Path, bytes: &[u8]) -> Result<(), StoreError> {
    let temp = temp_path(path);
    let result = fs::write(&temp, bytes)
        .map_err(io_at(&temp))
        .and_then(|()| fs::rename(&temp, path).map_err(io_at(path)));
    if result.is_err() {
        let _ = fs::remove_file(&temp);
    }
    result
}

fn temp_path(path: &Path) -> PathBuf {
    let parent = path.parent().unwrap_or(Path::new("."));
    let name = path
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_default();
    let seq = TEMP_SEQ.fetch_add(1, Ordering::Relaxed);
    parent.join(format!(".{name}.tmp.{}.{seq}", std::process::id()))
}

/// Creates one directory level; an existing directory is fine. The parent must
/// exist, for the reason [`atomic_write`] gives.
pub(crate) fn ensure_dir(dir: &Path) -> Result<(), StoreError> {
    match fs::create_dir(dir) {
        Ok(()) => Ok(()),
        Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => Ok(()),
        Err(source) => Err(StoreError::Io {
            path: dir.to_path_buf(),
            source,
        }),
    }
}

pub(crate) fn append(path: &Path, text: &str) -> Result<(), StoreError> {
    let parent = path.parent().unwrap_or(Path::new("."));
    fs::create_dir_all(parent).map_err(io_at(parent))?;
    let mut file = OpenOptions::new()
        .create(true)
        .append(true)
        .open(path)
        .map_err(io_at(path))?;
    file.write_all(text.as_bytes()).map_err(io_at(path))
}

/// Bytes under `dir`, skipping `exclude`.
pub(crate) fn dir_size(dir: &Path, exclude: &Path) -> u64 {
    let Ok(entries) = fs::read_dir(dir) else {
        return 0;
    };
    let mut total = 0;
    for entry in entries.filter_map(Result::ok) {
        let path = entry.path();
        if path == exclude {
            continue;
        }
        match entry.metadata() {
            Ok(meta) if meta.is_dir() => total += dir_size(&path, exclude),
            Ok(meta) => total += meta.len(),
            Err(_) => {}
        }
    }
    total
}

#[cfg(test)]
mod tests {
    use super::*;

    fn scratch(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("gbcli-io-{}-{name}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn a_failed_rename_leaves_no_temp_file() {
        let dir = scratch("rename");
        // Renaming a file over a directory fails.
        let target = dir.join("head.gbck");
        fs::create_dir(&target).unwrap();
        fs::write(target.join("inside"), b"x").unwrap();
        assert!(atomic_write(&target, b"data").is_err());
        let names: Vec<_> = fs::read_dir(&dir)
            .unwrap()
            .map(|e| e.unwrap().file_name().to_string_lossy().into_owned())
            .collect();
        assert_eq!(names, vec!["head.gbck"], "{names:?}");
        fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn an_atomic_write_never_creates_its_directory() {
        let dir = scratch("no-parent");
        let gone = dir.join("removed-session");
        assert!(atomic_write(&gone.join("session.json"), b"{}").is_err());
        assert!(!gone.exists());
        fs::remove_dir_all(&dir).unwrap();
    }
}
