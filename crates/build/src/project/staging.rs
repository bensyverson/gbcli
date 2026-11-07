//! A build's staging directory: beside the final outputs, made and removed
//! by this build alone. Its name is gbcli's and unique to the build, so a
//! directory of the user's or a concurrent build's is never touched.
//!
//! A build killed midway never runs its cleanup, so each staging directory
//! holds a `lock` file its build keeps locked while it runs; the lock dies
//! with the process. The next build into the same directory removes every
//! staging directory whose lock it can take: their builds are gone.

use std::fs::{self, File};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU32, Ordering};

use super::BuildError;

const PREFIX: &str = ".gbcli-staging-";
const LOCK: &str = "lock";

pub(super) struct Staging {
    dir: PathBuf,
    /// Held for the build's life; `None` where the filesystem can't lock,
    /// and then the directory, if the build is killed, is never swept.
    _lock: Option<File>,
}

impl Staging {
    /// Sweeps what killed builds left beside `beside`, then makes this
    /// build's directory there.
    pub(super) fn new(beside: &Path) -> Result<Staging, BuildError> {
        sweep(beside);
        static NEXT: AtomicU32 = AtomicU32::new(0);
        loop {
            let n = NEXT.fetch_add(1, Ordering::Relaxed);
            let dir = beside.join(format!("{PREFIX}{}-{n}", std::process::id()));
            match fs::create_dir(&dir) {
                Ok(()) => {
                    let lock = lock(&dir);
                    return Ok(Staging { dir, _lock: lock });
                }
                Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => continue,
                Err(error) => return Err(BuildError::Io { path: dir, error }),
            }
        }
    }

    pub(super) fn path(&self) -> &Path {
        &self.dir
    }
}

impl Drop for Staging {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.dir);
    }
}

/// Locks a file in `dir` and only then names it `lock`, so a sweep never
/// finds a `lock` file before it is locked. `None` when the filesystem
/// can't lock.
fn lock(dir: &Path) -> Option<File> {
    let pending = dir.join(format!("{LOCK}.new"));
    let file = File::create_new(&pending).ok()?;
    if file.lock().is_err() || fs::rename(&pending, dir.join(LOCK)).is_err() {
        let _ = fs::remove_file(&pending);
        return None;
    }
    Some(file)
}

/// Removes the staging directories in `dir` whose build is gone: those
/// whose `lock` this process can take. A directory without one (its build
/// is between making it and locking) is left alone. Best effort: a sweep
/// never fails a build.
fn sweep(dir: &Path) {
    let Ok(entries) = fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        if !entry.file_name().to_string_lossy().starts_with(PREFIX) {
            continue;
        }
        let staging = entry.path();
        let Ok(lock) = File::open(staging.join(LOCK)) else {
            continue;
        };
        if lock.try_lock().is_ok() {
            let _ = fs::remove_dir_all(&staging);
        }
    }
}
