//! The machine around the server, as its caller describes it: where a
//! request's relative paths start, and the `PATH` RGBDS is found on. The
//! caller builds it at its edge (the CLI from its `Env`); no library crate
//! reads it from the process.

use std::ffi::{OsStr, OsString};
use std::fmt;
use std::path::{Path, PathBuf};

/// Where relative paths start, and the `PATH` RGBDS is found on.
#[derive(Clone, Debug)]
pub struct Host {
    cwd: PathBuf,
    path: Option<OsString>,
}

/// Why a [`Host`] wasn't made: its working directory is relative, so a
/// request's relative path would have nothing fixed to start from.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RelativeCwd(pub PathBuf);

impl fmt::Display for RelativeCwd {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "the working directory {:?} isn't absolute",
            self.0.display().to_string()
        )
    }
}

impl std::error::Error for RelativeCwd {}

impl Host {
    /// A host whose working directory is `cwd`, which must be absolute but
    /// need not exist (a memory store's project is a path under which
    /// nothing is on disk), and whose `PATH` is `path`, if it has one.
    pub fn new(cwd: impl Into<PathBuf>, path: Option<OsString>) -> Result<Host, RelativeCwd> {
        let cwd = cwd.into();
        if !cwd.is_absolute() {
            return Err(RelativeCwd(cwd));
        }
        Ok(Host { cwd, path })
    }

    /// The working directory: always absolute.
    pub fn cwd(&self) -> &Path {
        &self.cwd
    }

    /// The `PATH` RGBDS is found on, if the host has one.
    pub fn path(&self) -> Option<&OsStr> {
        self.path.as_deref()
    }

    /// A path a request gave, where the server reads or writes it: a
    /// relative one starts at the working directory.
    pub fn resolve(&self, path: impl AsRef<Path>) -> PathBuf {
        self.cwd.join(path)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_relative_working_directory_is_refused_and_an_absolute_one_kept() {
        for relative in [".", "", "project"] {
            let refused = Host::new(relative, None).unwrap_err();
            assert_eq!(refused, RelativeCwd(PathBuf::from(relative)));
            assert!(refused.to_string().contains("isn't absolute"), "{refused}");
        }
        let host = Host::new("/no/such/dir", Some("/bin".into())).unwrap();
        assert_eq!(host.cwd(), Path::new("/no/such/dir"));
        assert_eq!(host.path(), Some(OsStr::new("/bin")));
        assert_eq!(host.resolve("shot.png"), Path::new("/no/such/dir/shot.png"));
        assert_eq!(host.resolve("/abs.png"), Path::new("/abs.png"));
    }
}
