//! Where a build's sources, include directories and outputs are, found
//! and checked before any tool runs.

use std::collections::HashSet;
use std::fs;
use std::path::{Path, PathBuf};

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
    pub(super) fn of(
        project: &Path,
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
        let name = project
            .file_name()
            .map_or("game".into(), |n| n.to_string_lossy().into_owned());
        let out = match &options.out {
            Some(out) => project.join(out),
            None => project.join("build").join(format!("{name}.gb")),
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
                        quote(&format!("build/{name}.gb"))
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
