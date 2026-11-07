use std::fs;
use std::path::PathBuf;

use gbcli_protocol::{ErrorCode, Tool};

use super::*;

/// A directory of fake RGBDS tools, each a script printing `version_line`.
struct FakePath(PathBuf);

impl FakePath {
    fn new(name: &str, tools: &[(Tool, &str)]) -> FakePath {
        let dir = std::env::temp_dir().join(format!(
            "gbcli-build-toolchain-{}-{name}",
            std::process::id()
        ));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        for (tool, line) in tools {
            let path = dir.join(tool.name());
            fs::write(&path, format!("#!/bin/sh\necho '{line}'\n")).unwrap();
            make_executable(&path);
        }
        FakePath(dir)
    }

    fn var(&self) -> std::ffi::OsString {
        self.0.clone().into_os_string()
    }
}

impl Drop for FakePath {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

#[cfg(unix)]
fn make_executable(path: &std::path::Path) {
    use std::os::unix::fs::PermissionsExt;
    fs::set_permissions(path, fs::Permissions::from_mode(0o755)).unwrap();
}

fn all(line: &str) -> Vec<(Tool, String)> {
    Tool::ALL
        .iter()
        .map(|t| (*t, format!("{} {line}", t.name())))
        .collect()
}

fn fake(name: &str, lines: &[(Tool, String)]) -> FakePath {
    let refs: Vec<(Tool, &str)> = lines.iter().map(|(t, l)| (*t, l.as_str())).collect();
    FakePath::new(name, &refs)
}

#[test]
fn versions_parse_from_the_tools_version_line() {
    assert_eq!(
        Version::parse("rgbasm v1.0.4"),
        Some(Version::release(1, 0, 4))
    );
    assert_eq!(
        Version::parse("rgblink v0.9.1\n"),
        Some(Version::release(0, 9, 1))
    );
    let rc = Version::parse("rgbfix v1.0.4-rc1").unwrap();
    assert_eq!(rc.suffix, "-rc1");
    assert_eq!(rc.to_string(), "1.0.4-rc1");
    assert_eq!(Version::parse("rgbasm"), None);
    assert_eq!(Version::parse("rgbasm v1.0"), None);
}

#[test]
fn the_pin_is_exact_another_patch_warns_and_anything_else_is_refused() {
    assert_eq!(RGBDS_PIN.to_string(), "1.0.4");
    assert_eq!(RGBDS_PIN.against_pin(), PinMatch::Exact);
    assert_eq!(Version::release(1, 0, 2).against_pin(), PinMatch::SameMinor);
    assert_eq!(
        Version::parse("v1.0.4-rc1").unwrap().against_pin(),
        PinMatch::SameMinor
    );
    assert_eq!(Version::release(1, 1, 4).against_pin(), PinMatch::Other);
    assert_eq!(Version::release(0, 9, 4).against_pin(), PinMatch::Other);
}

#[test]
fn the_remedy_names_the_install_command_and_the_pinned_release() {
    let remedy = install_remedy();
    assert!(remedy.contains("`brew install rgbds`"), "{remedy}");
    assert!(
        remedy.contains("https://github.com/gbdev/rgbds/releases/tag/v1.0.4"),
        "{remedy}"
    );
}

#[test]
fn no_tool_on_path_is_toolchain_missing_naming_all_three() {
    let empty = FakePath::new("empty", &[]);
    let err = Toolchain::discover(Some(&empty.var())).unwrap_err();
    assert_eq!(err.problems.len(), 3);
    let body = err.body();
    assert_eq!(body.code, ErrorCode::ToolchainMissing);
    assert_eq!(body.exit, 5);
    for tool in Tool::ALL {
        assert!(body.message.contains(tool.name()), "{}", body.message);
    }
    assert!(
        body.remedy.contains("brew install rgbds"),
        "{}",
        body.remedy
    );
    assert!(Toolchain::discover(None).is_err(), "no PATH at all");
}

#[test]
fn the_pinned_version_is_found_with_no_warning() {
    let path = fake("pinned", &all("v1.0.4"));
    let chain = Toolchain::discover(Some(&path.var())).unwrap();
    assert_eq!(chain.version(), &RGBDS_PIN);
    assert_eq!(chain.path(Tool::Rgblink), path.0.join("rgblink"));
    assert!(chain.warnings().is_empty(), "{:?}", chain.warnings());
}

#[test]
fn another_patch_is_accepted_with_a_warning_naming_the_pin() {
    let mut lines = all("v1.0.4");
    lines[1].1 = "rgblink v1.0.2".into();
    let path = fake("patch", &lines);
    let chain = Toolchain::discover(Some(&path.var())).unwrap();
    let warnings = chain.warnings();
    assert_eq!(warnings.len(), 1, "{warnings:?}");
    assert!(warnings[0].contains("rgblink"), "{warnings:?}");
    assert!(warnings[0].contains("1.0.2"), "{warnings:?}");
    assert!(warnings[0].contains("1.0.4"), "{warnings:?}");
}

#[test]
fn another_minor_version_is_refused_naming_it_and_the_pin() {
    let path = fake("old", &all("v0.9.1"));
    let err = Toolchain::discover(Some(&path.var())).unwrap_err();
    let body = err.body();
    assert_eq!(body.code, ErrorCode::ToolchainMissing);
    assert!(body.message.contains("0.9.1"), "{}", body.message);
    assert!(body.message.contains("1.0.4"), "{}", body.message);
}

#[test]
fn a_tool_that_prints_no_version_is_a_problem_quoting_what_it_printed() {
    let mut lines = all("v1.0.4");
    lines[2].1 = "hello".into();
    let path = fake("garbled", &lines);
    let err = Toolchain::discover(Some(&path.var())).unwrap_err();
    assert_eq!(err.problems.len(), 1);
    assert!(matches!(
        err.problems[0],
        ToolProblem::NoVersion {
            tool: Tool::Rgbfix,
            ..
        }
    ));
    assert!(err.problems[0].message().contains("hello"));
}

#[test]
fn probe_takes_the_first_match_on_path() {
    let first = fake("first", &all("v1.0.4"));
    let second = fake("second", &all("v0.9.1"));
    let joined = std::env::join_paths([first.0.clone(), second.0.clone()]).unwrap();
    let found = probe(Tool::Rgbasm, Some(&joined)).unwrap();
    assert_eq!(found.path, first.0.join("rgbasm"));
    assert_eq!(found.warning(), None);
}
