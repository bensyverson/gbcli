//! `build`: a project built by convention, with no build file. Every `.asm`
//! under the source directories is assembled with the include directories on
//! rgbasm's include path, the objects are linked into one ROM with its `.sym`
//! and `.map`, and rgbfix fixes the header. Each step's outputs are checked
//! to exist and be non-empty (DX 10). The ROM, `.sym` and `.map` are written
//! beside the final ones and moved into place only when every step passed,
//! so a failed build never leaves a stale or half-written ROM where a
//! session would load it.

use std::fs;
use std::path::{Component, Path, PathBuf};
use std::process::Command;

use gbcli_protocol::{
    BuildMade, Diagnostic, ErrorBody, ErrorCode, OutputKind, OutputMade, ResultBody, Severity, Tool,
};

use crate::diagnostics;
use crate::paths::display;
use crate::toolchain::Toolchain;

/// What to build, and how paths are printed. Relative paths are relative to
/// `project`.
#[derive(Clone, Debug)]
pub struct BuildOptions {
    pub project: PathBuf,
    /// Source directories; default `src`.
    pub src: Vec<PathBuf>,
    /// Include directories; default `include`, when it exists.
    pub include: Vec<PathBuf>,
    /// The ROM; default `build/<project directory name>.gb`. The `.sym` and
    /// `.map` sit beside it.
    pub out: Option<PathBuf>,
    /// Paths in the result are relative to this when inside it.
    pub display_base: PathBuf,
}

impl BuildOptions {
    /// The conventional layout of the project at `project`.
    pub fn new(project: PathBuf, display_base: PathBuf) -> BuildOptions {
        BuildOptions {
            project,
            src: Vec::new(),
            include: Vec::new(),
            out: None,
            display_base,
        }
    }
}

/// Why a build produced no ROM.
#[derive(Debug)]
pub enum BuildError {
    /// The project, a source directory or an include directory isn't there,
    /// or there is nothing to assemble.
    Layout { message: String, remedy: String },
    /// A tool reported errors or wrote no output; the report says which.
    Failed(Box<BuildMade>),
    Io {
        path: PathBuf,
        error: std::io::Error,
    },
}

impl BuildError {
    pub fn body(&self) -> ErrorBody {
        match self {
            BuildError::Layout { message, remedy } => {
                ErrorBody::new(ErrorCode::BadRequest, message, remedy)
            }
            BuildError::Failed(made) => {
                let errors: Vec<&Diagnostic> = made
                    .diagnostics
                    .iter()
                    .filter(|d| d.severity == Severity::Error)
                    .collect();
                let first = errors.first().map_or_else(
                    || "no diagnostic".to_string(),
                    |d| {
                        let place = match (&d.file, d.line) {
                            (Some(file), Some(line)) => format!("{file}:{line}: "),
                            (Some(file), None) => format!("{file}: "),
                            _ => String::new(),
                        };
                        format!("{place}{} ({})", d.message, d.tool.name())
                    },
                );
                let count = match errors.len() {
                    1 => "1 error".to_string(),
                    n => format!("{n} errors, the first"),
                };
                let mut body = ErrorBody::plain(
                    ErrorCode::BuildFailed,
                    format!("the build failed with {count}: {first}"),
                );
                body.result = Some(Box::new(ResultBody {
                    build: Some((**made).clone()),
                    ..ResultBody::default()
                }));
                body
            }
            BuildError::Io { path, error } => {
                ErrorBody::plain(ErrorCode::IoFailed, format!("{}: {error}", path.display()))
            }
        }
    }
}

/// Builds the project. Warnings ride in the result's diagnostics.
pub fn build(tools: &Toolchain, options: &BuildOptions) -> Result<BuildMade, BuildError> {
    let base = fs::canonicalize(&options.display_base).unwrap_or(options.display_base.clone());
    let shown = |p: &Path| display(p, &base);
    let project = fs::canonicalize(&options.project).map_err(|_| BuildError::Layout {
        message: format!("no project directory at {}", shown(&options.project)),
        remedy: "pass the project directory, or make one with `gbcli new DIR`".into(),
    })?;
    let layout = Layout::of(&project, options, &shown)?;
    let mut run = Run {
        tools,
        project: &project,
        shown: &shown,
        made: BuildMade {
            rgbds: tools.version().to_string(),
            sources: layout.sources.iter().map(|s| shown(s)).collect(),
            outputs: Vec::new(),
            diagnostics: Vec::new(),
        },
    };

    let mut objects = Vec::new();
    let mut ok = true;
    for source in &layout.sources {
        let rel = source.strip_prefix(&project).unwrap_or(source);
        // A source outside the project (an absolute --src) still gets its
        // object under build/obj/, never beside the source.
        let inside: PathBuf = rel
            .components()
            .filter(|c| matches!(c, Component::Normal(_)))
            .collect();
        let object = layout.objects.join(inside).with_extension("o");
        create_dir(object.parent().expect("an object has a directory"))?;
        let mut args: Vec<String> = Vec::new();
        for dir in &layout.include {
            args.push("-I".into());
            args.push(dir.display().to_string());
        }
        args.extend(["-o".into(), object.display().to_string()]);
        args.push(rel.display().to_string());
        ok &= run.step(Tool::Rgbasm, &args, Some(source), &[&object])?;
        objects.push(object);
    }
    if !ok {
        return Err(run.failed());
    }

    let staging = layout
        .out
        .parent()
        .expect("the ROM has a directory")
        .join(".staging");
    let _ = fs::remove_dir_all(&staging);
    create_dir(&staging)?;
    let finals = [
        (OutputKind::Rom, layout.out.clone()),
        (OutputKind::Sym, layout.out.with_extension("sym")),
        (OutputKind::Map, layout.out.with_extension("map")),
    ];
    let staged: Vec<PathBuf> = finals
        .iter()
        .map(|(_, p)| staging.join(p.file_name().expect("a file name")))
        .collect();
    let mut args: Vec<String> = Vec::new();
    for (flag, path) in ["-o", "-n", "-m"].iter().zip(&staged) {
        args.extend([flag.to_string(), path.display().to_string()]);
    }
    args.extend(objects.iter().map(|o| o.display().to_string()));
    let linked = run.step(
        Tool::Rgblink,
        &args,
        None,
        &staged.iter().collect::<Vec<_>>(),
    )?;
    let fixed = linked && {
        let args = ["-v", "-p", "0xFF"].map(String::from).to_vec();
        let rom = staged[0].display().to_string();
        run.step(
            Tool::Rgbfix,
            &[args, vec![rom]].concat(),
            None,
            &[&staged[0]],
        )?
    };
    if !fixed {
        let _ = fs::remove_dir_all(&staging);
        return Err(run.failed());
    }
    for ((kind, path), from) in finals.iter().zip(&staged) {
        fs::rename(from, path).map_err(|error| BuildError::Io {
            path: path.clone(),
            error,
        })?;
        let bytes = fs::metadata(path).map(|m| m.len()).unwrap_or(0);
        run.made.outputs.push(OutputMade {
            kind: *kind,
            path: shown(path),
            bytes,
        });
    }
    let _ = fs::remove_dir_all(&staging);
    Ok(run.made)
}

/// Where everything is, checked before any tool runs.
struct Layout {
    sources: Vec<PathBuf>,
    /// Relative to the project, as rgbasm is given them.
    include: Vec<PathBuf>,
    objects: PathBuf,
    out: PathBuf,
}

impl Layout {
    fn of(
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
        for dir in &src_dirs {
            let dir = project.join(dir);
            if !dir.is_dir() {
                return Err(missing("source", &dir, "src"));
            }
            asm_files(&dir, &mut sources).map_err(|error| BuildError::Io {
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
        Ok(Layout {
            sources,
            include,
            objects: project.join("build").join("obj"),
            out,
        })
    }
}

/// Every `.asm` under `dir`, recursively.
fn asm_files(dir: &Path, out: &mut Vec<PathBuf>) -> std::io::Result<()> {
    for entry in fs::read_dir(dir)? {
        let path = entry?.path();
        if path.is_dir() {
            asm_files(&path, out)?;
        } else if path.extension().is_some_and(|e| e == "asm") {
            out.push(path);
        }
    }
    Ok(())
}

fn create_dir(dir: &Path) -> Result<(), BuildError> {
    fs::create_dir_all(dir).map_err(|error| BuildError::Io {
        path: dir.to_path_buf(),
        error,
    })
}

/// One build's tool runs and what they reported.
struct Run<'a> {
    tools: &'a Toolchain,
    project: &'a Path,
    shown: &'a dyn Fn(&Path) -> String,
    made: BuildMade,
}

impl Run<'_> {
    /// Runs `tool` in the project and records its diagnostics. A step passes
    /// when the tool exits 0 and every one of `outputs` is non-empty;
    /// otherwise it records an error (on `source`, when it has one) unless
    /// the tool already reported one.
    fn step(
        &mut self,
        tool: Tool,
        args: &[String],
        source: Option<&Path>,
        outputs: &[&PathBuf],
    ) -> Result<bool, BuildError> {
        let path = self.tools.path(tool);
        let output = Command::new(path)
            .args(args)
            .current_dir(self.project)
            .output()
            .map_err(|error| BuildError::Io {
                path: path.to_path_buf(),
                error,
            })?;
        let stderr = String::from_utf8_lossy(&output.stderr);
        let mut found = diagnostics::parse(tool, &stderr);
        for d in &mut found {
            if let Some(file) = &d.file {
                d.file = Some((self.shown)(&self.project.join(file)));
            }
        }
        let reported = found.iter().any(|d| d.severity == Severity::Error);
        self.made.diagnostics.extend(found);
        let file = source.map(|s| (self.shown)(s));
        if !output.status.success() {
            if !reported {
                let status = output
                    .status
                    .code()
                    .map_or("was killed by a signal".into(), |c| {
                        format!("exited with status {c}")
                    });
                let said = stderr.trim();
                let said = if said.is_empty() {
                    "it printed nothing".to_string()
                } else {
                    said.lines().rev().take(3).collect::<Vec<_>>().join(" / ")
                };
                self.error(tool, file, format!("{} {status}: {said}", tool.name()));
            }
            return Ok(false);
        }
        for out in outputs {
            let name = tool.name();
            let shown = (self.shown)(out);
            let message = match fs::metadata(out).map(|m| m.len()) {
                Ok(0) => format!("{name} exited 0 but left {shown} empty"),
                Ok(_) => continue,
                Err(_) => format!("{name} exited 0 but didn't write {shown}"),
            };
            self.error(tool, file.clone(), message);
            return Ok(false);
        }
        Ok(true)
    }

    fn error(&mut self, tool: Tool, file: Option<String>, message: String) {
        self.made.diagnostics.push(Diagnostic {
            tool,
            severity: Severity::Error,
            file,
            line: None,
            message,
        });
    }

    fn failed(self) -> BuildError {
        BuildError::Failed(Box::new(self.made))
    }
}
