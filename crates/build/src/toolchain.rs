//! RGBDS on `PATH`, checked against the pinned version.

use std::ffi::OsStr;
use std::fmt;
use std::path::{Path, PathBuf};

use gbcli_protocol::{ErrorBody, ErrorCode, Tool};

/// A tool's version: `1.0.4`, with anything after the numbers (`-rc1`) kept
/// in `suffix`, which makes it not the release of those numbers.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Version {
    pub major: u32,
    pub minor: u32,
    pub patch: u32,
    pub suffix: String,
}

/// The RGBDS release gbcli is built and tested against. A build refuses any
/// other minor version; another patch of this one works with a warning.
pub const RGBDS_PIN: Version = Version::release(1, 0, 4);

/// How the pinned RGBDS is installed where Homebrew is.
pub const INSTALL_COMMAND: &str = "brew install rgbds";

/// How a version compares with [`RGBDS_PIN`].
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PinMatch {
    Exact,
    /// Same major and minor, another patch or a pre-release: a warning.
    SameMinor,
    /// Anything else: refused.
    Other,
}

impl Version {
    pub const fn release(major: u32, minor: u32, patch: u32) -> Version {
        Version {
            major,
            minor,
            patch,
            suffix: String::new(),
        }
    }

    /// The first `MAJOR.MINOR.PATCH` in a tool's `--version` line
    /// (`rgbasm v1.0.4`), with what follows it up to a space as the suffix.
    pub fn parse(text: &str) -> Option<Version> {
        text.split_whitespace().find_map(|word| {
            let word = word.strip_prefix('v').unwrap_or(word);
            let digits_end = |s: &str| s.find(|c: char| !c.is_ascii_digit()).unwrap_or(s.len());
            let mut rest = word;
            let mut numbers = [0u32; 3];
            for (i, n) in numbers.iter_mut().enumerate() {
                if i > 0 {
                    rest = rest.strip_prefix('.')?;
                }
                let end = digits_end(rest);
                *n = rest[..end].parse().ok()?;
                rest = &rest[end..];
            }
            Some(Version {
                major: numbers[0],
                minor: numbers[1],
                patch: numbers[2],
                suffix: rest.to_string(),
            })
        })
    }

    pub fn against_pin(&self) -> PinMatch {
        if *self == RGBDS_PIN {
            PinMatch::Exact
        } else if (self.major, self.minor) == (RGBDS_PIN.major, RGBDS_PIN.minor) {
            PinMatch::SameMinor
        } else {
            PinMatch::Other
        }
    }
}

impl fmt::Display for Version {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{}.{}.{}{}",
            self.major, self.minor, self.patch, self.suffix
        )
    }
}

/// The pinned release's page.
pub fn release_url() -> String {
    format!("https://github.com/gbdev/rgbds/releases/tag/v{RGBDS_PIN}")
}

/// What to do when RGBDS is missing or the wrong version: the pinned
/// release first, since Homebrew installs whatever release it has now.
pub fn install_remedy() -> String {
    format!(
        "install RGBDS {RGBDS_PIN} from {} (`{INSTALL_COMMAND}` installs Homebrew's current \
         release, which may not be {RGBDS_PIN}); put rgbasm, rgblink and rgbfix on PATH, then \
         check with `gbcli doctor`",
        release_url()
    )
}

/// One tool, found and runnable at an accepted version.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ToolFound {
    pub tool: Tool,
    pub path: PathBuf,
    pub version: Version,
}

/// Why a tool can't be used.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ToolProblem {
    Missing {
        tool: Tool,
    },
    /// It ran, but `--version` printed no version.
    NoVersion {
        tool: Tool,
        path: PathBuf,
        output: String,
    },
    Unsupported {
        tool: Tool,
        path: PathBuf,
        version: Version,
    },
}

impl ToolProblem {
    pub fn tool(&self) -> Tool {
        match self {
            ToolProblem::Missing { tool }
            | ToolProblem::NoVersion { tool, .. }
            | ToolProblem::Unsupported { tool, .. } => *tool,
        }
    }

    pub fn message(&self) -> String {
        match self {
            ToolProblem::Missing { tool } => format!("{} is not on PATH", tool.name()),
            ToolProblem::NoVersion { tool, path, output } => format!(
                "{} ({}) printed no version for --version: {:?}",
                tool.name(),
                path.display(),
                output.trim(),
            ),
            ToolProblem::Unsupported {
                tool,
                path,
                version,
            } => format!(
                "{} ({}) is {version}; gbcli needs RGBDS {RGBDS_PIN}",
                tool.name(),
                path.display()
            ),
        }
    }
}

impl ToolFound {
    /// The warning an accepted but unpinned version earns, if any.
    pub fn warning(&self) -> Option<String> {
        (self.version.against_pin() != PinMatch::Exact).then(|| {
            format!(
                "{} is {}, not the pinned RGBDS {RGBDS_PIN}; builds may differ from gbcli's tests",
                self.tool.name(),
                self.version
            )
        })
    }
}

/// Finds `tool` on `path_var` (a `PATH` value) and checks its version. A
/// relative `PATH` entry is relative to `cwd`.
pub fn probe(tool: Tool, path_var: Option<&OsStr>, cwd: &Path) -> Result<ToolFound, ToolProblem> {
    let path = find(tool.name(), path_var, cwd).ok_or(ToolProblem::Missing { tool })?;
    let output = std::process::Command::new(&path)
        .arg("--version")
        .output()
        .map_err(|e| ToolProblem::NoVersion {
            tool,
            path: path.clone(),
            output: e.to_string(),
        })?;
    let text = String::from_utf8_lossy(&output.stdout).into_owned();
    let Some(version) = Version::parse(text.lines().next().unwrap_or("")) else {
        let mut said = text;
        said.push_str(&String::from_utf8_lossy(&output.stderr));
        said.truncate(200);
        return Err(ToolProblem::NoVersion {
            tool,
            path,
            output: said,
        });
    };
    if version.against_pin() == PinMatch::Other {
        return Err(ToolProblem::Unsupported {
            tool,
            path,
            version,
        });
    }
    Ok(ToolFound {
        tool,
        path,
        version,
    })
}

/// The first executable called `name` in the directories of `path_var`.
/// A relative entry is resolved against `cwd` here, since the tool may run
/// from another directory (a build runs in the project).
pub(crate) fn find(name: &str, path_var: Option<&OsStr>, cwd: &Path) -> Option<PathBuf> {
    let file = format!("{name}{}", std::env::consts::EXE_SUFFIX);
    std::env::split_paths(path_var?)
        .filter(|dir| !dir.as_os_str().is_empty())
        .map(|dir| cwd.join(dir).join(&file))
        .find(|candidate| is_executable(candidate))
}

#[cfg(unix)]
fn is_executable(path: &Path) -> bool {
    use std::os::unix::fs::PermissionsExt;
    path.metadata()
        .is_ok_and(|m| m.is_file() && m.permissions().mode() & 0o111 != 0)
}

#[cfg(not(unix))]
fn is_executable(path: &Path) -> bool {
    path.is_file()
}

/// rgbasm, rgblink and rgbfix, each found at an accepted version, and
/// rgbgfx when it is there: only a project with images needs it.
#[derive(Clone, Debug)]
pub struct Toolchain {
    found: Vec<ToolFound>,
    rgbgfx: Result<ToolFound, ToolProblem>,
}

/// Every tool that can't be used, so one message names them all.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ToolchainError {
    pub problems: Vec<ToolProblem>,
}

impl Toolchain {
    /// Finds the three on `path_var`; a relative entry is relative to `cwd`.
    pub fn discover(path_var: Option<&OsStr>, cwd: &Path) -> Result<Toolchain, ToolchainError> {
        let mut found = Vec::new();
        let mut problems = Vec::new();
        for tool in Tool::ALL {
            match probe(tool, path_var, cwd) {
                Ok(f) => found.push(f),
                Err(p) => problems.push(p),
            }
        }
        if problems.is_empty() {
            let rgbgfx = probe(Tool::Rgbgfx, path_var, cwd);
            Ok(Toolchain { found, rgbgfx })
        } else {
            Err(ToolchainError { problems })
        }
    }

    fn get(&self, tool: Tool) -> &ToolFound {
        self.found
            .iter()
            .find(|f| f.tool == tool)
            .expect("discover finds every tool or fails")
    }

    pub fn path(&self, tool: Tool) -> &Path {
        &self.get(tool).path
    }

    /// rgbgfx, or why it can't be used.
    pub fn rgbgfx(&self) -> Result<&Path, ToolchainError> {
        match &self.rgbgfx {
            Ok(found) => Ok(&found.path),
            Err(problem) => Err(ToolchainError {
                problems: vec![problem.clone()],
            }),
        }
    }

    /// rgbasm's version: the one a build reports.
    pub fn version(&self) -> &Version {
        &self.get(Tool::Rgbasm).version
    }

    /// One warning per tool at an unpinned version.
    pub fn warnings(&self) -> Vec<String> {
        self.found.iter().filter_map(ToolFound::warning).collect()
    }
}

impl ToolchainError {
    /// `toolchain_missing`, naming each problem, with the install command.
    pub fn body(&self) -> ErrorBody {
        let problems: Vec<String> = self.problems.iter().map(ToolProblem::message).collect();
        ErrorBody::new(
            ErrorCode::ToolchainMissing,
            problems.join("; "),
            install_remedy(),
        )
    }
}

#[cfg(test)]
mod tests;
