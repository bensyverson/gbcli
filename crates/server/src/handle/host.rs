//! The machine around the server, as its caller describes it: where a
//! request's relative paths start, and whether a request may reach the
//! host's own files and programs ([`HostFiles`]). The caller builds it at
//! its edge (the CLI from its `Env`); no library crate reads it from the
//! process.

use std::ffi::{OsStr, OsString};
use std::fmt;
use std::path::{Path, PathBuf};

use gbcli_protocol::{ErrorBody, ErrorCode, Op};

/// Where relative paths start, and whether a request may reach host files.
#[derive(Clone, Debug)]
pub struct Host {
    cwd: PathBuf,
    files: HostFiles,
}

/// Whether a request may read or write the host's own files, or run a
/// program, outside the project's store. The ops that do (`build`,
/// `doctor`, `new`, `assets`, `pixels`, `font` and the font authoring ops)
/// go to the filesystem directly, not through the store, so a caller whose
/// store isn't the filesystem denies them, and they are refused rather than
/// reaching a disk the caller never offered.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum HostFiles {
    /// The host's filesystem is the request's, and RGBDS is found on
    /// `path` (the host's `PATH`, if it has one): the CLI and `gbcli serve`.
    Allowed { path: Option<OsString> },
    /// Nothing outside the store: the ops that reach host files answer
    /// `unsupported`. A memory store's caller.
    Denied,
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
    /// nothing is on disk), and which allows or denies host files.
    pub fn new(cwd: impl Into<PathBuf>, files: HostFiles) -> Result<Host, RelativeCwd> {
        let cwd = cwd.into();
        if !cwd.is_absolute() {
            return Err(RelativeCwd(cwd));
        }
        Ok(Host { cwd, files })
    }

    /// The working directory: always absolute.
    pub fn cwd(&self) -> &Path {
        &self.cwd
    }

    /// Whether a request may reach host files.
    pub fn files(&self) -> &HostFiles {
        &self.files
    }

    /// The `PATH` RGBDS is found on, if the host allows host files and has
    /// one.
    pub fn path(&self) -> Option<&OsStr> {
        match &self.files {
            HostFiles::Allowed { path } => path.as_deref(),
            HostFiles::Denied => None,
        }
    }

    /// A path a request gave, where the server reads or writes it: a
    /// relative one starts at the working directory.
    pub fn resolve(&self, path: impl AsRef<Path>) -> PathBuf {
        self.cwd.join(path)
    }

    /// The refusal of `op`, when it reaches host files and this host denies
    /// them.
    pub(crate) fn refusal(&self, op: &Op) -> Option<ErrorBody> {
        if self.files != HostFiles::Denied || !reaches_host_files(op) {
            return None;
        }
        Some(ErrorBody::new(
            ErrorCode::Unsupported,
            format!(
                "`{}` reads or writes the host's files or runs a program, and this host \
                 denies host files",
                op.name()
            ),
            "this server's caller holds the project in its own store (a memory store), which \
             only session ops, `sessions`, `gaps`, `prune`, `rm`, `opcode` and `version` \
             reach; run it through the CLI or `gbcli serve`, which work on the filesystem",
        ))
    }
}

/// Whether `op` reads or writes host paths, or runs a program, outside the
/// project's store. Every op is named, so a new one must be placed.
fn reaches_host_files(op: &Op) -> bool {
    match op {
        Op::Build(_)
        | Op::Doctor
        | Op::New(_)
        | Op::Assets(_)
        | Op::Pixels(_)
        | Op::Font(_)
        | Op::FontEdit(_)
        | Op::FontSave(_)
        | Op::FontImport(_)
        | Op::FontExport(_)
        | Op::FontNew(_)
        | Op::FontDiff(_) => true,
        // Through the store, or touching nothing.
        Op::Run(_)
        | Op::Step(_)
        | Op::Show
        | Op::Read(_)
        | Op::Break(_)
        | Op::Watch(_)
        | Op::Delete(_)
        | Op::Checkpoint(_)
        | Op::Restore(_)
        | Op::Undo
        | Op::Write(_)
        | Op::Events(_)
        | Op::Shot(_)
        | Op::View(_)
        | Op::Sym(_)
        | Op::Checkpoints
        | Op::Measure(_)
        | Op::Equiv(_)
        | Op::PowerCycle(_)
        | Op::Sav(_)
        | Op::Sessions
        | Op::Gaps
        | Op::Prune(_)
        | Op::Rm(_)
        | Op::Opcode(_)
        | Op::Version => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_relative_working_directory_is_refused_and_an_absolute_one_kept() {
        for relative in [".", "", "project"] {
            let refused = Host::new(relative, HostFiles::Denied).unwrap_err();
            assert_eq!(refused, RelativeCwd(PathBuf::from(relative)));
            assert!(refused.to_string().contains("isn't absolute"), "{refused}");
        }
        let allowed = HostFiles::Allowed {
            path: Some("/bin".into()),
        };
        let host = Host::new("/no/such/dir", allowed.clone()).unwrap();
        assert_eq!(host.cwd(), Path::new("/no/such/dir"));
        assert_eq!(host.files(), &allowed);
        assert_eq!(host.path(), Some(OsStr::new("/bin")));
        assert_eq!(host.resolve("shot.png"), Path::new("/no/such/dir/shot.png"));
        assert_eq!(host.resolve("/abs.png"), Path::new("/abs.png"));
        let denied = Host::new("/no/such/dir", HostFiles::Denied).unwrap();
        assert_eq!(denied.path(), None, "a denied host runs nothing");
    }
}
