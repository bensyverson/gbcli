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
    /// The machine the ROM is for; default the DMG, and left off the wire
    /// then. `gbcli build --gbc` sends `"gbc"`.
    #[serde(default, skip_serializing_if = "BuildTarget::is_dmg")]
    pub target: BuildTarget,
}

/// The machine a build is for, which sets the header's CGB flag ($0143).
/// User-facing names say "GBC" (Ben, 2026-11-07); Pan Docs calls the
/// machine CGB. A GBC-only build (rgbfix `-C`, $0143 = $C0) is not offered.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum BuildTarget {
    /// The original Game Boy: rgbfix leaves $0143 at $00.
    #[default]
    Dmg,
    /// Dual mode: GBC-enhanced, and still runs on the DMG. rgbfix `-c`
    /// writes $0143 = $80.
    Gbc,
}

impl BuildTarget {
    pub fn is_dmg(&self) -> bool {
        *self == BuildTarget::Dmg
    }

    /// The wire name: `dmg` or `gbc`.
    pub fn name(self) -> &'static str {
        match self {
            BuildTarget::Dmg => "dmg",
            BuildTarget::Gbc => "gbc",
        }
    }
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
    /// The machine the ROM was built for: `--gbc` makes it `gbc`.
    pub target: BuildTarget,
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
    /// The sections a cartridge with no MBC can't reach: set when the build
    /// failed because the link placed sections past bank 1.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub overflow: Option<BankOverflow>,
}

/// A link past 32 KB for a header that names no MBC (cartridge type $00,
/// $08 or $09): such a cartridge maps banks 0 and 1 only, so whatever reads
/// a section in bank 2 or later reads bank 1's bytes instead.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct BankOverflow {
    /// The header's cartridge type byte ($0147).
    pub cart_type: u8,
    /// Every section the `.map` places in a ROMX bank past 1, in map order.
    pub sections: Vec<PlacedSection>,
}

/// A section as the linker placed it.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PlacedSection {
    pub name: String,
    pub bank: u32,
    /// Its first address in the bank's window ($4000–$7FFF for ROMX).
    pub start: u16,
    pub bytes: u32,
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

    /// `target` is left out of a DMG build's args, as before the GBC target
    /// existed, and is `"gbc"` for dual mode; the result always says it.
    #[test]
    fn the_build_target_is_dmg_by_default_and_gbc_on_the_wire() {
        let dmg: BuildArgs = serde_json::from_str(r#"{"dir":"game"}"#).unwrap();
        assert_eq!(dmg.target, BuildTarget::Dmg);
        assert_eq!(serde_json::to_string(&dmg).unwrap(), r#"{"dir":"game"}"#);
        let gbc: BuildArgs = serde_json::from_str(r#"{"dir":"game","target":"gbc"}"#).unwrap();
        assert_eq!(gbc.target, BuildTarget::Gbc);
        assert_eq!(
            serde_json::to_string(&gbc).unwrap(),
            r#"{"dir":"game","target":"gbc"}"#
        );
        assert!(serde_json::from_str::<BuildArgs>(r#"{"target":"cgb"}"#).is_err());
        for target in [BuildTarget::Dmg, BuildTarget::Gbc] {
            assert_eq!(serde_json::to_value(target).unwrap(), target.name());
        }
        let made = BuildMade {
            rgbds: "1.0.4".into(),
            target: BuildTarget::Dmg,
            sources: Vec::new(),
            outputs: Vec::new(),
            diagnostics: Vec::new(),
            images: None,
            songs: Vec::new(),
            overflow: None,
        };
        let json = serde_json::to_string(&made).unwrap();
        assert_eq!(
            json,
            r#"{"rgbds":"1.0.4","target":"dmg","sources":[],"outputs":[],"diagnostics":[]}"#
        );
        assert_eq!(serde_json::from_str::<BuildMade>(&json).unwrap(), made);
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
