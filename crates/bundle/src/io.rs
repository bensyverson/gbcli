//! Filesystem helpers for [`crate::FileStore`]. Writes are atomic against a
//! killed process (temp file, then rename) but not against power loss:
//! nothing is fsynced (design §1, "Durability").

use std::fs::{self, OpenOptions};
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

/// Distinguishes temp files written by threads of one process.
static TEMP_SEQ: AtomicU64 = AtomicU64::new(0);

/// Writes `bytes` to a temp file beside `path`, then renames it over `path`.
/// The parent must exist: creating it here could resurrect a session another
/// process just removed. A failed write or rename removes the temp file.
pub(crate) fn atomic_write(path: &Path, bytes: &[u8]) -> io::Result<()> {
    let temp = temp_path(path);
    let result = fs::write(&temp, bytes).and_then(|()| fs::rename(&temp, path));
    if result.is_err() {
        let _ = fs::remove_file(&temp);
    }
    result
}

/// A temp file's name beside `path`, unique to this process and call.
pub(crate) fn temp_path(path: &Path) -> PathBuf {
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
pub(crate) fn ensure_dir(dir: &Path) -> io::Result<()> {
    match fs::create_dir(dir) {
        Ok(()) => Ok(()),
        // Fine only when what has the name is a directory: a file there
        // would fail every write under it later, far from the cause.
        Err(e) if e.kind() == io::ErrorKind::AlreadyExists && dir.is_dir() => Ok(()),
        Err(e) => Err(e),
    }
}

/// Appends `bytes` to `path`, creating the file but not its directory, for
/// the reason [`atomic_write`] gives.
pub(crate) fn append(path: &Path, bytes: &[u8]) -> io::Result<()> {
    let mut file = OpenOptions::new().create(true).append(true).open(path)?;
    file.write_all(bytes)
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
