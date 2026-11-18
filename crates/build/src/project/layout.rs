//! Where a build's sources, include directories and outputs are, found
//! and checked before any tool runs.

use std::collections::HashSet;
use std::fs;
use std::path::{Path, PathBuf};

use gbcli_protocol::BuildTarget;

use super::{BuildError, BuildOptions};
use crate::paths::quote;

/// Where everything is, checked before any tool runs.
pub(super) struct Layout {
    pub sources: Vec<PathBuf>,
    /// Relative to the project, as rgbasm is given them.
    pub include: Vec<PathBuf>,
    pub objects: PathBuf,
    pub out: PathBuf,
}

impl Layout {
    /// The layout of `project` under `options`, a relative `out` read
    /// against `cwd`.
    pub(super) fn of(
        project: &Path,
        cwd: &Path,
        options: &BuildOptions,
        shown: &dyn Fn(&Path) -> String,
    ) -> Result<Layout, BuildError> {
        let missing = |what: &str, dir: &Path, flag: &str| BuildError::Layout {
            message: format!("no {what} directory at {}", shown(dir)),
            remedy: format!(
                "make it, or pass --{flag} DIR; `gbcli new DIR` scaffolds a project with the \
                 conventional layout"
            ),
        };
        let src_dirs: Vec<PathBuf> = if options.src.is_empty() {
            vec![PathBuf::from("src")]
        } else {
            options.src.clone()
        };
        let mut sources = Vec::new();
        let mut seen = HashSet::new();
        for dir in &src_dirs {
            let dir = project.join(dir);
            if !dir.is_dir() {
                return Err(missing("source", &dir, "src"));
            }
            asm_files(&dir, &mut sources, &mut seen).map_err(|error| BuildError::Io {
                path: dir.clone(),
                error,
            })?;
        }
        sources.sort();
        sources.dedup();
        if sources.is_empty() {
            let dirs: Vec<String> = src_dirs.iter().map(|d| shown(&project.join(d))).collect();
            return Err(BuildError::Layout {
                message: format!(
                    "nothing to assemble: no .asm file under {}",
                    dirs.join(", ")
                ),
                remedy: "add one, pass --src DIR, or scaffold a starter with `gbcli new DIR`"
                    .into(),
            });
        }
        let include = if options.include.is_empty() {
            let conventional = PathBuf::from("include");
            if project.join(&conventional).is_dir() {
                vec![conventional]
            } else {
                Vec::new()
            }
        } else {
            for dir in &options.include {
                if !project.join(dir).is_dir() {
                    return Err(missing("include", &project.join(dir), "include"));
                }
            }
            options.include.clone()
        };
        let out = match &options.out {
            Some(out) => cwd.join(out),
            None => default_rom(project, options.target),
        };
        // rgblink would be given one path for two outputs.
        for beside in ["sym", "map"] {
            if out.with_extension(beside) == out {
                return Err(BuildError::Layout {
                    message: format!(
                        "the ROM can't be {}: the build writes its .{beside} there, beside \
                         the ROM and named after it",
                        shown(&out)
                    ),
                    remedy: format!(
                        "pass an --out that ends .gb, e.g. --out {}",
                        quote(&shown(
                            &project
                                .join("build")
                                .join(format!("{}.gb", project_name(project)))
                        ))
                    ),
                });
            }
        }
        Ok(Layout {
            sources,
            include,
            objects: project.join("build").join("obj"),
            out,
        })
    }
}

/// The ROM a build of `project` for `target` writes without --out, under
/// `project`: `build/NAME.gb` or `build/gbc/NAME.gbc`, NAME being the
/// project directory's name.
pub fn default_rom(project: &Path, target: BuildTarget) -> PathBuf {
    project.join(default_out(&project_name(project), target))
}

/// The target whose build of `project`, without --out, wrote `rom`; `None`
/// for any other file, or one that doesn't exist.
pub fn default_target_of(project: &Path, rom: &Path) -> Option<BuildTarget> {
    let rom = rom.canonicalize().ok()?;
    [BuildTarget::Dmg, BuildTarget::Gbc]
        .into_iter()
        .find(|&t| default_rom(project, t).canonicalize().ok().as_ref() == Some(&rom))
}

/// The name a project's ROM takes: its directory's.
fn project_name(project: &Path) -> String {
    project
        .file_name()
        .map_or("game".into(), |n| n.to_string_lossy().into_owned())
}

/// The ROM a build writes without --out: `build/NAME.gb`, or for the GBC
/// `build/gbc/NAME.gbc`, in a folder of its own because every tool finds a
/// ROM's .sym by swapping its extension, so `build/NAME.gbc` would share
/// the plain build's `build/NAME.sym` (Ben's ruling, 2026-11-09).
fn default_out(name: &str, target: BuildTarget) -> PathBuf {
    match target {
        BuildTarget::Dmg => Path::new("build").join(format!("{name}.gb")),
        BuildTarget::Gbc => Path::new("build").join("gbc").join(format!("{name}.gbc")),
    }
}

/// Every `.asm` under `dir`, recursively. A directory reached twice (a
/// symlink back up the tree, or to a directory already walked) is walked
/// once.
fn asm_files(
    dir: &Path,
    out: &mut Vec<PathBuf>,
    seen: &mut HashSet<PathBuf>,
) -> std::io::Result<()> {
    if !seen.insert(fs::canonicalize(dir)?) {
        return Ok(());
    }
    for entry in fs::read_dir(dir)? {
        let path = entry?.path();
        if path.is_dir() {
            asm_files(&path, out, seen)?;
        } else if path.extension().is_some_and(|e| e == "asm") {
            out.push(path);
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_default_rom_names_its_target_and_any_other_file_none() {
        let project = std::env::temp_dir()
            .join(format!("gbcli-layout-{}", std::process::id()))
            .join("game");
        fs::create_dir_all(project.join("build/gbc")).unwrap();
        for rom in ["build/game.gb", "build/gbc/game.gbc", "build/other.gb"] {
            fs::write(project.join(rom), b"rom").unwrap();
        }
        let of = |rom: &str| default_target_of(&project, &project.join(rom));
        assert_eq!(of("build/game.gb"), Some(BuildTarget::Dmg));
        assert_eq!(of("build/./gbc/game.gbc"), Some(BuildTarget::Gbc));
        assert_eq!(of("build/other.gb"), None);
        assert_eq!(of("build/missing.gb"), None);
        fs::remove_dir_all(project.parent().unwrap()).unwrap();
    }
}
