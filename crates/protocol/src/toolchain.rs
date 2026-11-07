//! What the RGBDS loop's verbs report (design §12, Build and `doctor`):
//! `build`'s outputs and diagnostics, `doctor`'s checks and `new`'s files.
//! `gbcli-build` makes them and the CLI prints them, so they are defined
//! once, here.

use serde::{Deserialize, Serialize};

use crate::images::ImagesMade;

/// `op: "build"`: a project built by convention (design §12). Paths are
/// relative to the server's working directory; `src`, `include` and `out`
/// to the project.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct BuildArgs {
    /// The project directory; default the server's working directory.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub dir: Option<String>,
    /// Source directories; default `src`.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub src: Vec<String>,
    /// Include directories; default `include`, when it exists.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub include: Vec<String>,
    /// The ROM; default `build/<project directory name>.gb`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub out: Option<String>,
    /// After a good build, restore this checkpoint into the request's
    /// session, whose ROM must be the one just built.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub restore: Option<String>,
}

/// `op: "new"`: a starter project in `dir`, relative to the server's
/// working directory.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct NewArgs {
    pub dir: String,
}

/// One RGBDS program.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Tool {
    Rgbasm,
    Rgblink,
    Rgbfix,
    /// The image converter: needed only when `assets/` holds images, so not
    /// among [`Tool::ALL`].
    Rgbgfx,
}

impl Tool {
    /// The three, in the order a build runs them.
    pub const ALL: [Tool; 3] = [Tool::Rgbasm, Tool::Rgblink, Tool::Rgbfix];

    /// The executable's name, as on `PATH` and on the wire.
    pub fn name(self) -> &'static str {
        match self {
            Tool::Rgbasm => "rgbasm",
            Tool::Rgblink => "rgblink",
            Tool::Rgbfix => "rgbfix",
            Tool::Rgbgfx => "rgbgfx",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Severity {
    Error,
    Warning,
}

/// One message from an RGBDS tool, located where the tool located it.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Diagnostic {
    pub tool: Tool,
    pub severity: Severity,
    /// The source file, relative to the working directory when inside it.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub file: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub line: Option<u32>,
    /// The macro or `REPT` iteration `line` is in (`Bad`, `REPT~2`), when
    /// it is in one: `line` is then a line of its body.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub within: Option<String>,
    pub message: String,
    /// How `file:line` was reached, innermost first: the line that called
    /// the macro, ran the `REPT` or included the file, then what reached
    /// that line, out to the source that was assembled.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub chain: Vec<SourceLine>,
    /// The other places the message names (rgblink's "and also", as for a
    /// symbol defined twice), each with the chain that reached it,
    /// innermost first.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub also: Vec<Vec<SourceLine>>,
}

impl Diagnostic {
    /// A message with no location.
    pub fn new(tool: Tool, severity: Severity, message: impl Into<String>) -> Diagnostic {
        Diagnostic {
            tool,
            severity,
            file: None,
            line: None,
            within: None,
            message: message.into(),
            chain: Vec::new(),
            also: Vec::new(),
        }
    }
}

/// A line of a source file, inside a macro or `REPT` when `within` names it.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct SourceLine {
    /// Relative to the working directory when inside it.
    pub file: String,
    pub line: u32,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub within: Option<String>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum OutputKind {
    Rom,
    Sym,
    Map,
}

/// A file the build wrote, checked to exist and be non-empty (DX 10).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct OutputMade {
    pub kind: OutputKind,
    pub path: String,
    pub bytes: u64,
}

/// What `build` did. On failure it rides in the error's `result`, with no
/// outputs and at least one error diagnostic.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct BuildMade {
    /// rgbasm's version, as it printed it.
    pub rgbds: String,
    /// The sources assembled, in order.
    pub sources: Vec<String>,
    pub outputs: Vec<OutputMade>,
    pub diagnostics: Vec<Diagnostic>,
    /// The images under `assets/`, when there are any: converted, or the
    /// limits they break.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub images: Option<ImagesMade>,
    /// The songs under `assets/music` converted to hUGEDriver data and
    /// assembled with the sources.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub songs: Vec<SongBuilt>,
}

/// A song file `build` converted: hUGETracker's `.uge` or gbcli's song text,
/// written as RGBDS assembly for hUGEDriver.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct SongBuilt {
    /// The song file, relative to the working directory when inside it.
    pub path: String,
    /// The `.asm` written from it and assembled.
    pub asm: String,
    /// The label to load into `hl` before `call hUGE_init`.
    pub descriptor: String,
    /// The timer modulo (TMA) when the song wants `hUGE_dosound` called from
    /// the timer interrupt at 4096 / (256 − TMA) Hz; none when once per frame.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub timer: Option<u8>,
}

/// What `doctor` checks.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CheckName {
    Rgbasm,
    Rgblink,
    Rgbfix,
    BootRoms,
    Gitignore,
    Writable,
    CoreSupport,
    Rgbgfx,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum CheckStatus {
    Ok,
    /// Works, but not as pinned or recommended.
    Warn,
    Fail,
    /// Not checked by this gbcli; `detail` says why.
    Skip,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct CheckMade {
    pub name: CheckName,
    pub status: CheckStatus,
    pub detail: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub remedy: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct DoctorMade {
    pub checks: Vec<CheckMade>,
}

/// What `new` wrote.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct NewMade {
    pub name: String,
    pub dir: String,
    pub files: Vec<String>,
    /// The commands to run next, in order.
    pub next: Vec<String>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_diagnostic_spells_its_enums_lowercase_and_drops_what_it_lacks() {
        let d = Diagnostic {
            file: Some("src/main.asm".into()),
            ..Diagnostic::new(Tool::Rgbasm, Severity::Error, "syntax error")
        };
        let json = serde_json::to_string(&d).unwrap();
        assert_eq!(
            json,
            r#"{"tool":"rgbasm","severity":"error","file":"src/main.asm","message":"syntax error"}"#
        );
        assert_eq!(serde_json::from_str::<Diagnostic>(&json).unwrap(), d);
    }

    #[test]
    fn of_the_loops_ops_only_a_build_that_restores_needs_a_bundle_or_changes_the_session() {
        use crate::Op;
        let restoring = BuildArgs {
            restore: Some("start".into()),
            ..BuildArgs::default()
        };
        let quiet = [
            Op::Doctor,
            Op::New(NewArgs { dir: "game".into() }),
            Op::Build(BuildArgs::default()),
        ];
        for op in &quiet {
            assert!(!op.needs_bundle(), "{op:?}");
            assert!(!op.changes_session(), "{op:?}");
        }
        let op = Op::Build(restoring);
        assert!(op.needs_bundle() && op.changes_session() && op.mutates_head());
        assert!(Op::Show.needs_bundle() && Op::Sessions.needs_bundle());
    }

    #[test]
    fn check_names_are_snake_case_and_tool_names_match_the_wire() {
        assert_eq!(
            serde_json::to_value(CheckName::BootRoms).unwrap(),
            "boot_roms"
        );
        for tool in Tool::ALL {
            assert_eq!(serde_json::to_value(tool).unwrap(), tool.name());
        }
    }
}
