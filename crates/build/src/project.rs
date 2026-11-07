//! `build`: a project built by convention, with no build file. Every `.asm`
//! under the source directories is assembled with the include directories on
//! rgbasm's include path, the objects are linked into one ROM with its `.sym`
//! and `.map`, and rgbfix fixes the header. Each step's outputs are checked
//! to exist and be non-empty (DX 10). The ROM, `.sym` and `.map` are written
//! to a staging directory of this build's own beside the final ones and
//! moved into place only when every step passed, the ROM last, so a failed
//! build never leaves a stale or half-written ROM where a session would load
//! it. Every tool runs with `--color never` and under a time limit.

mod layout;
mod staging;
mod tool;

use std::fs;
use std::path::{Component, Path, PathBuf};
use std::time::Duration;

use gbcli_protocol::{
    BuildMade, Diagnostic, ErrorBody, ErrorCode, OutputKind, OutputMade, ResultBody, Severity, Tool,
};

use crate::diagnostics;
use crate::paths::{display, quote};
use crate::toolchain::Toolchain;
use layout::Layout;
use staging::Staging;

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
    /// How long one tool run may take before it is stopped and the build
    /// fails; default [`TOOL_TIMEOUT`].
    pub tool_timeout: Duration,
}

/// How long one rgbasm, rgblink or rgbfix run may take by default. The
/// starter assembles in milliseconds; a run this long is a tool stuck in a
/// loop (a huge `REPT`), and a build server must not hang on it.
pub const TOOL_TIMEOUT: Duration = Duration::from_secs(120);

impl BuildOptions {
    /// The conventional layout of the project at `project`.
    pub fn new(project: PathBuf, display_base: PathBuf) -> BuildOptions {
        BuildOptions {
            project,
            src: Vec::new(),
            include: Vec::new(),
            out: None,
            display_base,
            tool_timeout: TOOL_TIMEOUT,
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
    /// `again` is the command that builds the same way, for the remedy.
    Failed { made: Box<BuildMade>, again: String },
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
            BuildError::Failed { made, again } => {
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
                let mut body = ErrorBody::new(
                    ErrorCode::BuildFailed,
                    format!("the build failed with {count}: {first}"),
                    format!("fix the first error at its file and line, then `{again}` again"),
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
        timeout: options.tool_timeout,
        again: again(&shown(&project), options),
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

    let out_dir = layout.out.parent().expect("the ROM has a directory");
    create_dir(out_dir)?;
    let staging = Staging::new(out_dir)?;
    let finals = [
        (OutputKind::Rom, layout.out.clone()),
        (OutputKind::Sym, layout.out.with_extension("sym")),
        (OutputKind::Map, layout.out.with_extension("map")),
    ];
    let staged: Vec<PathBuf> = finals
        .iter()
        .map(|(_, p)| staging.path().join(p.file_name().expect("a file name")))
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
        return Err(run.failed());
    }
    // The `.sym` and `.map` first and the ROM last: the ROM's arrival marks
    // a complete build, so whatever loads it finds the `.sym` it was linked
    // with.
    let mut outputs = Vec::new();
    for i in [1, 2, 0] {
        let ((kind, path), from) = (&finals[i], &staged[i]);
        fs::rename(from, path).map_err(|error| BuildError::Io {
            path: path.clone(),
            error,
        })?;
        let bytes = fs::metadata(path).map(|m| m.len()).unwrap_or(0);
        outputs.push(OutputMade {
            kind: *kind,
            path: shown(path),
            bytes,
        });
    }
    outputs.rotate_right(1);
    run.made.outputs = outputs;
    Ok(run.made)
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
    timeout: Duration,
    /// The command that builds the same way, for a failure's remedy.
    again: String,
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
        // RGBDS colors its diagnostics when FORCE_COLOR is set, even into a
        // pipe, and colored text doesn't parse.
        let args = [&["--color".to_string(), "never".to_string()], args].concat();
        let ran =
            tool::run(path, &args, self.project, self.timeout).map_err(|error| BuildError::Io {
                path: path.to_path_buf(),
                error,
            })?;
        let stderr = ran.stderr;
        let mut found = diagnostics::parse(tool, &stderr);
        for d in &mut found {
            if let Some(file) = &d.file {
                d.file = Some(self.show(file));
            }
            for at in d.chain.iter_mut().chain(d.also.iter_mut().flatten()) {
                at.file = self.show(&at.file);
            }
        }
        let reported = found.iter().any(|d| d.severity == Severity::Error);
        self.made.diagnostics.extend(found);
        let file = source.map(|s| (self.shown)(s));
        let Some(status) = ran.status else {
            let message = format!(
                "{} ran past {:.1} s and was stopped",
                tool.name(),
                self.timeout.as_secs_f64()
            );
            self.error(tool, file, message);
            return Ok(false);
        };
        if !status.success() {
            if !reported {
                let status = status.code().map_or("was killed by a signal".into(), |c| {
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

    /// A file as a tool printed it (relative to the project, where it ran),
    /// as the caller reads it.
    fn show(&self, file: &str) -> String {
        (self.shown)(&self.project.join(file))
    }

    fn error(&mut self, tool: Tool, file: Option<String>, message: String) {
        self.made.diagnostics.push(Diagnostic {
            file,
            ..Diagnostic::new(tool, Severity::Error, message)
        });
    }

    fn failed(self) -> BuildError {
        BuildError::Failed {
            again: self.again,
            made: Box::new(self.made),
        }
    }
}

/// The command that builds the project shown as `dir` with `options`' layout
/// again, each word quoted for a shell.
fn again(dir: &str, options: &BuildOptions) -> String {
    let mut words = vec!["gbcli".to_string(), "build".to_string()];
    if dir != "." {
        words.push(quote(dir));
    }
    let flags = [
        ("--src", options.src.iter().collect::<Vec<_>>()),
        ("--include", options.include.iter().collect()),
        ("--out", options.out.iter().collect()),
    ];
    for (flag, paths) in flags {
        for path in paths {
            words.push(flag.to_string());
            words.push(quote(&path.display().to_string()));
        }
    }
    words.join(" ")
}
