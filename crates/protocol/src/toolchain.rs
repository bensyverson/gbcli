//! What the RGBDS loop's verbs report (design §12, Build and `doctor`):
//! `build`'s outputs and diagnostics, `doctor`'s checks and `new`'s files.
//! `gbcli-build` makes them and the CLI prints them, so they are defined
//! once, here.

use serde::{Deserialize, Serialize};

/// One RGBDS program.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Tool {
    Rgbasm,
    Rgblink,
    Rgbfix,
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
    pub message: String,
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
            tool: Tool::Rgbasm,
            severity: Severity::Error,
            file: Some("src/main.asm".into()),
            line: None,
            message: "syntax error".into(),
        };
        let json = serde_json::to_string(&d).unwrap();
        assert_eq!(
            json,
            r#"{"tool":"rgbasm","severity":"error","file":"src/main.asm","message":"syntax error"}"#
        );
        assert_eq!(serde_json::from_str::<Diagnostic>(&json).unwrap(), d);
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
