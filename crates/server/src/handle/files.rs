//! The files a request names for itself (a shot, a filmstrip, a WAV, a
//! memory dump, a `.sav`): the host resolves the path, as a shell would, and
//! the project's store reads and writes it, so a store that isn't the
//! filesystem holds them too.

use std::io;
use std::path::{Path, PathBuf};

use gbcli_bundle::Bundle;
use gbcli_protocol::{ErrorBody, ErrorCode};

use super::Failure;
use super::host::Host;

/// A request's own paths: resolved by the host, kept by the bundle's store.
#[derive(Clone, Debug)]
pub(super) struct Files {
    host: Host,
    bundle: Bundle,
}

impl Files {
    pub(super) fn new(host: Host, bundle: Bundle) -> Files {
        Files { host, bundle }
    }

    /// Where a path the request gave is: a relative one starts at the
    /// host's working directory.
    pub(super) fn resolve(&self, path: impl AsRef<Path>) -> PathBuf {
        self.host.resolve(path)
    }

    pub(super) fn is_dir(&self, path: impl AsRef<Path>) -> bool {
        self.bundle.is_dir(&self.resolve(path))
    }

    pub(super) fn exists(&self, path: impl AsRef<Path>) -> bool {
        self.bundle.exists(&self.resolve(path))
    }

    pub(super) fn read(&self, path: impl AsRef<Path>) -> io::Result<Vec<u8>> {
        self.bundle.read_file(&self.resolve(path))
    }

    /// Writes a file atomically, so a reader never sees half of it.
    pub(super) fn write(&self, path: impl AsRef<Path>, bytes: &[u8]) -> Result<(), Failure> {
        let path = self.resolve(path);
        self.bundle
            .write_file(&path, bytes)
            .map_err(|e| couldnt_write(&path, &e))
    }

    /// Writes every file or none: a directory in a file's place is refused
    /// before anything is written, then the store writes each before any
    /// replaces its file (`ProjectStore::put_all`). Every reported failure
    /// but one leaves the files as they were; the exception, a replace
    /// refused after another succeeded, needs a change on disk between the
    /// two. A process killed between the replaces leaves the files it
    /// replaced new beside the rest old: atomic for each file, not for the
    /// set.
    pub(super) fn write_all(&self, files: &[(&Path, &[u8])]) -> Result<(), Failure> {
        let resolved: Vec<PathBuf> = files.iter().map(|(path, _)| self.resolve(path)).collect();
        if let Some(path) = resolved.iter().find(|path| self.bundle.is_dir(path)) {
            return Err(couldnt_write(path, &"a directory has its name"));
        }
        let blobs: Vec<(&Path, &[u8])> = resolved
            .iter()
            .zip(files)
            .map(|(path, (_, bytes))| (path.as_path(), *bytes))
            .collect();
        self.bundle
            .write_files(&blobs)
            .map_err(|e| couldnt_write(&resolved[e.index], &e.error))
    }
}

fn couldnt_write(path: &Path, e: &dyn std::fmt::Display) -> Failure {
    Failure::new(ErrorBody::new(
        ErrorCode::IoFailed,
        format!("couldn't write {}: {e}", path.display()),
        ErrorCode::IoFailed.default_remedy(),
    ))
}
