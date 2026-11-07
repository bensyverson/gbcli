//! `doctor`: what this machine and project need, checked, each with the
//! remedy for a problem (design §12). It never needs a session, a ROM or the
//! network.

use std::ffi::OsStr;
use std::fs;
use std::path::Path;
use std::process::{Command, Stdio};

use gbcli_protocol::{
    CheckMade, CheckName, CheckStatus, DoctorMade, ErrorBody, ErrorCode, ResultBody, Tool,
};

use crate::toolchain::{PinMatch, find, install_remedy, probe};

/// The sizes a real boot ROM has: 256 bytes (DMG, MGB, SGB) and 2304 bytes
/// (CGB, AGB: 256, a gap for the header, then 2 KiB).
pub const BOOT_ROM_SIZES: [usize; 2] = [256, 2304];

/// The facts `doctor` checks that its caller holds.
pub struct DoctorInput<'a> {
    /// The `PATH` to find RGBDS and git on.
    pub path_var: Option<&'a OsStr>,
    /// What a relative `PATH` entry is relative to.
    pub cwd: &'a Path,
    /// Where `.gbcli/` goes: the working directory, or `--dir`.
    pub project: &'a Path,
    /// The boot ROMs built into this gbcli, by name.
    pub boot_roms: &'a [(&'a str, &'a [u8])],
}

/// Every check. A failure makes it an error whose `result` still carries
/// every check; warnings and skips don't.
pub fn doctor(input: &DoctorInput) -> Result<DoctorMade, Box<ErrorBody>> {
    let mut checks = Vec::new();
    // The code a failing check makes the whole verdict, first failure first.
    let mut failure: Option<(ErrorCode, String, String)> = None;
    for tool in Tool::ALL {
        let check = match probe(tool, input.path_var, input.cwd) {
            Ok(found) => CheckMade {
                name: tool_check(tool),
                status: match found.version.against_pin() {
                    PinMatch::Exact => CheckStatus::Ok,
                    _ => CheckStatus::Warn,
                },
                detail: format!("{} at {}", found.version, found.path.display()),
                remedy: found.warning().map(|_| install_remedy()),
            },
            Err(problem) => {
                failure.get_or_insert((
                    ErrorCode::ToolchainMissing,
                    problem.message(),
                    install_remedy(),
                ));
                CheckMade {
                    name: tool_check(tool),
                    status: CheckStatus::Fail,
                    detail: problem.message(),
                    remedy: Some(install_remedy()),
                }
            }
        };
        checks.push(check);
    }
    for check in [
        boot_roms(input.boot_roms),
        gitignore(input.project, input.path_var, input.cwd),
        writable(input.project),
    ] {
        if check.status == CheckStatus::Fail {
            let code = match check.name {
                CheckName::BootRoms => ErrorCode::BootRomMissing,
                _ => ErrorCode::IoFailed,
            };
            failure.get_or_insert((
                code,
                check.detail.clone(),
                check.remedy.clone().unwrap_or_default(),
            ));
        }
        checks.push(check);
    }
    checks.push(CheckMade {
        name: CheckName::CoreSupport,
        status: CheckStatus::Skip,
        detail: "the core's support table is not listed by this gbcli".into(),
        remedy: Some("`gbcli gaps` lists every feature the core lacked in this project".into()),
    });
    let made = DoctorMade { checks };
    match failure {
        None => Ok(made),
        Some((code, message, remedy)) => {
            let mut body = ErrorBody::new(code, message, remedy);
            body.result = Some(Box::new(ResultBody {
                doctor: Some(made),
                ..ResultBody::default()
            }));
            Err(Box::new(body))
        }
    }
}

fn tool_check(tool: Tool) -> CheckName {
    match tool {
        Tool::Rgbasm => CheckName::Rgbasm,
        Tool::Rgblink => CheckName::Rgblink,
        Tool::Rgbfix => CheckName::Rgbfix,
    }
}

fn boot_roms(roms: &[(&str, &[u8])]) -> CheckMade {
    let wrong: Vec<String> = roms
        .iter()
        .filter(|(_, bytes)| !BOOT_ROM_SIZES.contains(&bytes.len()))
        .map(|(name, bytes)| format!("{name} is {} bytes", bytes.len()))
        .collect();
    if wrong.is_empty() {
        let names: Vec<&str> = roms.iter().map(|(n, _)| *n).collect();
        CheckMade {
            name: CheckName::BootRoms,
            status: CheckStatus::Ok,
            detail: format!("built in: {}", names.join(", ")),
            remedy: None,
        }
    } else {
        CheckMade {
            name: CheckName::BootRoms,
            status: CheckStatus::Fail,
            detail: format!(
                "a built-in boot ROM is the wrong size ({}): this gbcli was built badly",
                wrong.join(", ")
            ),
            remedy: Some("rebuild gbcli from a clean checkout".into()),
        }
    }
}

/// Whether `.gbcli/` is ignored, when the project is in a git repository,
/// asked of git. Without a git that answers, any `.gitignore` from the
/// project up to the repository root may say so in one of a few spellings.
fn gitignore(project: &Path, path_var: Option<&OsStr>, cwd: &Path) -> CheckMade {
    let start = fs::canonicalize(project).unwrap_or(project.to_path_buf());
    let Some(root) = start.ancestors().find(|d| d.join(".git").exists()) else {
        return CheckMade {
            name: CheckName::Gitignore,
            status: CheckStatus::Ok,
            detail: "not in a git repository; nothing to ignore".into(),
            remedy: None,
        };
    };
    let warn = || CheckMade {
        name: CheckName::Gitignore,
        status: CheckStatus::Warn,
        detail: format!(
            "the repository at {} doesn't ignore .gbcli/, where sessions and checkpoints live",
            root.display()
        ),
        remedy: Some("add the line `.gbcli/` to .gitignore".into()),
    };
    // git knows every rule: parent directories, `**/`, .git/info/exclude
    // and the global excludes. Only without a git that answers is the
    // check approximated from the .gitignore files.
    if let Some(ignored) = git_ignores(&start, path_var, cwd) {
        return if ignored {
            CheckMade {
                name: CheckName::Gitignore,
                status: CheckStatus::Ok,
                detail: format!(
                    "`git check-ignore` says {} is ignored",
                    start.join(".gbcli/").display()
                ),
                remedy: None,
            }
        } else {
            warn()
        };
    }
    let ignores = |dir: &Path| {
        fs::read_to_string(dir.join(".gitignore")).is_ok_and(|text| {
            text.lines().any(|l| {
                matches!(
                    l.trim(),
                    ".gbcli" | ".gbcli/" | "/.gbcli" | "/.gbcli/" | ".gbcli/*" | "/.gbcli/*"
                )
            })
        })
    };
    let found = start
        .ancestors()
        .take_while(|d| d.starts_with(root))
        .find(|d| ignores(d));
    match found {
        Some(dir) => CheckMade {
            name: CheckName::Gitignore,
            status: CheckStatus::Ok,
            detail: format!("{} ignores .gbcli/", dir.join(".gitignore").display()),
            remedy: None,
        },
        None => warn(),
    }
}

/// Whether git ignores `.gbcli/` in `project`, or `None` when no git on
/// `path_var` answers (none there, or one that can't read its config).
fn git_ignores(project: &Path, path_var: Option<&OsStr>, cwd: &Path) -> Option<bool> {
    let git = find("git", path_var, cwd)?;
    // The trailing slash says it is a directory, which a `.gbcli/` pattern
    // needs to match a path that doesn't exist yet.
    let status = Command::new(git)
        .args(["check-ignore", "-q", ".gbcli/"])
        .current_dir(project)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .ok()?;
    match status.code() {
        Some(0) => Some(true),
        Some(1) => Some(false),
        _ => None,
    }
}

/// Whether gbcli can write where `.gbcli/` goes, probed with a file it
/// removes again.
fn writable(project: &Path) -> CheckMade {
    let probe = project.join(format!(".gbcli-doctor-{}", std::process::id()));
    let result = fs::write(&probe, b"probe");
    let _ = fs::remove_file(&probe);
    match result {
        Ok(()) => CheckMade {
            name: CheckName::Writable,
            status: CheckStatus::Ok,
            detail: format!("{} is writable", project.display()),
            remedy: None,
        },
        Err(e) => CheckMade {
            name: CheckName::Writable,
            status: CheckStatus::Fail,
            detail: format!("can't write in {}: {e}", project.display()),
            remedy: Some(
                "run gbcli from a directory it may write in (in a sandbox, the project or \
                 $TMPDIR)"
                    .into(),
            ),
        },
    }
}
