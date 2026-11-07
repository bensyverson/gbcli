//! `build` over the starter project with the pinned RGBDS, and over fake
//! tools for the failures RGBDS won't produce on purpose.

mod common;

use std::fs;
use std::path::PathBuf;

use gbcli_build::{BuildError, BuildOptions, Toolchain, build, new_project};
use gbcli_protocol::{BuildMade, ErrorCode, OutputKind, Severity, Tool};

use common::{Temp, fake_tools, rgbds};

/// A fresh starter project at `<temp>/demo`, printed relative to `<temp>`.
fn starter(name: &str) -> (Temp, BuildOptions) {
    let temp = Temp::new(name);
    new_project(&temp.path("demo"), &temp.0).unwrap();
    let options = BuildOptions::new(temp.path("demo"), temp.0.clone());
    (temp, options)
}

fn failed(result: Result<BuildMade, BuildError>) -> BuildMade {
    match result {
        Err(BuildError::Failed(made)) => *made,
        other => panic!("expected a failed build, got {other:?}"),
    }
}

/// The line of `needle` in `text`, from 1.
fn line_of(text: &str, needle: &str) -> u32 {
    text.lines().position(|l| l.contains(needle)).unwrap() as u32 + 1
}

#[test]
fn the_starter_builds_a_fixed_rom_with_its_sym_and_map() {
    let (temp, options) = starter("starter");
    let made = build(&rgbds(), &options).unwrap();
    assert_eq!(made.rgbds, "1.0.4");
    assert_eq!(made.sources, ["demo/src/main.asm", "demo/src/oam_dma.asm"]);
    assert!(made.diagnostics.is_empty(), "{:?}", made.diagnostics);
    let kinds: Vec<_> = made.outputs.iter().map(|o| o.kind).collect();
    assert_eq!(kinds, [OutputKind::Rom, OutputKind::Sym, OutputKind::Map]);
    assert_eq!(made.outputs[0].path, "demo/build/demo.gb");
    assert_eq!(made.outputs[1].path, "demo/build/demo.sym");
    assert_eq!(made.outputs[2].path, "demo/build/demo.map");
    for out in &made.outputs {
        let len = fs::metadata(temp.path(&out.path)).unwrap().len();
        assert_eq!(len, out.bytes, "{}", out.path);
        assert!(len > 0, "{}", out.path);
    }
    let rom = fs::read(temp.path("demo/build/demo.gb")).unwrap();
    assert_eq!(rom.len(), 32768);
    // rgbfix ran: the logo is in and the header checksum holds.
    assert_eq!(&rom[0x104..0x108], &[0xCE, 0xED, 0x66, 0x66]);
    let sum = rom[0x134..0x14D]
        .iter()
        .fold(0u8, |acc, &b| acc.wrapping_sub(b).wrapping_sub(1));
    assert_eq!(rom[0x14D], sum);
    let sym = fs::read_to_string(temp.path("demo/build/demo.sym")).unwrap();
    for label in ["VBlank", "hOAMDMA", "wShadowOAM", "wFrame"] {
        assert!(
            sym.lines().any(|l| l.ends_with(&format!(" {label}"))),
            "{sym}"
        );
    }
    assert!(
        !temp.path("demo/build/.staging").exists(),
        "staging is cleaned up"
    );
}

#[test]
fn a_syntax_error_is_a_structured_error_with_file_and_line() {
    let (temp, options) = starter("syntax");
    let main = temp.path("demo/src/main.asm");
    let text = fs::read_to_string(&main).unwrap();
    let broken = text.replacen("\tld sp, $E000", "\tld sp,", 1);
    fs::write(&main, &broken).unwrap();
    let made = failed(build(&rgbds(), &options));
    assert!(made.outputs.is_empty());
    let error = made
        .diagnostics
        .iter()
        .find(|d| d.severity == Severity::Error)
        .expect("an error diagnostic");
    assert_eq!(error.tool, Tool::Rgbasm);
    assert_eq!(error.file.as_deref(), Some("demo/src/main.asm"));
    assert_eq!(error.line, Some(line_of(&broken, "\tld sp,")));
    assert!(error.message.contains("syntax error"), "{}", error.message);
    let body = BuildError::Failed(Box::new(made.clone())).body();
    assert_eq!(body.code, ErrorCode::BuildFailed);
    assert_eq!(body.exit, 4);
    assert!(
        body.message.contains("demo/src/main.asm"),
        "{}",
        body.message
    );
    let carried = body.result.expect("the report rides in the error");
    assert_eq!(carried.build.as_ref(), Some(&made));
}

#[test]
fn a_failed_build_leaves_the_last_good_rom_in_place() {
    let (temp, options) = starter("stale");
    build(&rgbds(), &options).unwrap();
    let good = fs::read(temp.path("demo/build/demo.gb")).unwrap();
    fs::write(temp.path("demo/src/extra.asm"), "\tjp Nowhere\n").unwrap();
    let made = failed(build(&rgbds(), &options));
    assert!(
        made.diagnostics.iter().any(|d| d.tool == Tool::Rgbasm),
        "{:?}",
        made.diagnostics
    );
    assert_eq!(fs::read(temp.path("demo/build/demo.gb")).unwrap(), good);
}

#[test]
fn every_assembler_error_is_reported_before_linking_stops() {
    let (temp, options) = starter("all-errors");
    fs::write(
        temp.path("demo/src/a.asm"),
        "SECTION \"a\", ROM0\n\tld q, 1\n",
    )
    .unwrap();
    fs::write(
        temp.path("demo/src/b.asm"),
        "SECTION \"b\", ROM0\n\n\tjp ,\n",
    )
    .unwrap();
    let made = failed(build(&rgbds(), &options));
    let located: Vec<_> = made
        .diagnostics
        .iter()
        .filter(|d| d.severity == Severity::Error)
        .map(|d| (d.file.clone().unwrap(), d.line.unwrap()))
        .collect();
    assert_eq!(
        located,
        [
            ("demo/src/a.asm".to_string(), 2),
            ("demo/src/b.asm".to_string(), 3)
        ]
    );
}

#[test]
fn a_link_error_comes_from_rgblink_with_the_line_that_used_the_symbol() {
    let (temp, options) = starter("link");
    fs::write(
        temp.path("demo/src/extra.asm"),
        "SECTION \"extra\", ROM0\n\tjp Nowhere\n",
    )
    .unwrap();
    let made = failed(build(&rgbds(), &options));
    let error = &made.diagnostics[0];
    assert_eq!(error.tool, Tool::Rgblink);
    assert_eq!(error.file.as_deref(), Some("demo/src/extra.asm"));
    assert_eq!(error.line, Some(2));
    assert!(error.message.contains("Nowhere"), "{}", error.message);
}

#[test]
fn a_warning_rides_in_a_successful_build() {
    let (temp, options) = starter("warning");
    fs::write(
        temp.path("demo/src/extra.asm"),
        "SECTION \"extra\", ROM0\nExtra::\n\tld b, $123\n",
    )
    .unwrap();
    let made = build(&rgbds(), &options).unwrap();
    assert_eq!(made.outputs.len(), 3);
    assert_eq!(made.diagnostics.len(), 1, "{:?}", made.diagnostics);
    let warning = &made.diagnostics[0];
    assert_eq!(warning.severity, Severity::Warning);
    assert_eq!(warning.file.as_deref(), Some("demo/src/extra.asm"));
    assert_eq!(warning.line, Some(3));
}

#[test]
fn the_layout_can_be_overridden_and_sources_are_found_in_subdirectories() {
    let temp = Temp::new("layout");
    new_project(&temp.path("demo"), &temp.0).unwrap();
    fs::rename(temp.path("demo/src"), temp.path("demo/code")).unwrap();
    fs::create_dir(temp.path("demo/code/sub")).unwrap();
    fs::rename(
        temp.path("demo/code/oam_dma.asm"),
        temp.path("demo/code/sub/oam_dma.asm"),
    )
    .unwrap();
    fs::rename(temp.path("demo/include"), temp.path("demo/inc")).unwrap();
    let mut options = BuildOptions::new(temp.path("demo"), temp.path("demo"));
    options.src = vec![PathBuf::from("code")];
    options.include = vec![PathBuf::from("inc")];
    options.out = Some(PathBuf::from("out/game.gb"));
    let made = build(&rgbds(), &options).unwrap();
    assert_eq!(made.sources, ["code/main.asm", "code/sub/oam_dma.asm"]);
    let paths: Vec<_> = made.outputs.iter().map(|o| o.path.as_str()).collect();
    assert_eq!(paths, ["out/game.gb", "out/game.sym", "out/game.map"]);
}

#[test]
fn a_source_directory_outside_the_project_keeps_its_objects_in_the_project() {
    let (temp, mut options) = starter("outside");
    fs::rename(temp.path("demo/src"), temp.path("shared")).unwrap();
    options.src = vec![temp.path("shared")];
    build(&rgbds(), &options).unwrap();
    assert!(!temp.path("shared/main.o").exists());
    let objects = temp.path("demo/build/obj");
    let mut found = Vec::new();
    let mut stack = vec![objects];
    while let Some(dir) = stack.pop() {
        for entry in fs::read_dir(dir).unwrap() {
            let path = entry.unwrap().path();
            if path.is_dir() {
                stack.push(path);
            } else {
                found.push(path.file_name().unwrap().to_string_lossy().into_owned());
            }
        }
    }
    found.sort();
    assert_eq!(found, ["main.o", "oam_dma.o"]);
}

#[test]
fn nothing_to_assemble_or_a_missing_directory_is_a_layout_error() {
    let temp = Temp::new("layout-errors");
    fs::create_dir_all(temp.path("empty/src")).unwrap();
    let tools = rgbds();
    let body = build(
        &tools,
        &BuildOptions::new(temp.path("empty"), temp.0.clone()),
    )
    .unwrap_err()
    .body();
    assert_eq!(body.code, ErrorCode::BadRequest);
    assert!(body.message.contains("empty/src"), "{}", body.message);
    assert!(body.remedy.contains("gbcli new"), "{}", body.remedy);

    let body = build(
        &tools,
        &BuildOptions::new(temp.path("nowhere"), temp.0.clone()),
    )
    .unwrap_err()
    .body();
    assert_eq!(body.code, ErrorCode::BadRequest);
    assert!(body.message.contains("nowhere"), "{}", body.message);

    new_project(&temp.path("demo"), &temp.0).unwrap();
    let mut options = BuildOptions::new(temp.path("demo"), temp.0.clone());
    options.include = vec![PathBuf::from("headers")];
    let body = build(&tools, &options).unwrap_err().body();
    assert_eq!(body.code, ErrorCode::BadRequest);
    assert!(body.message.contains("demo/headers"), "{}", body.message);
}

#[test]
fn a_step_that_exits_zero_but_writes_nothing_fails_the_build() {
    let (temp, options) = starter("empty-output");
    // rgbasm writes its objects; rgblink "succeeds" and writes an empty ROM.
    let real = rgbds();
    let rgbasm = real.path(Tool::Rgbasm).display().to_string();
    let path = fake_tools(
        &temp,
        &[
            ("rgbasm", &format!("exec '{rgbasm}' \"$@\"")),
            (
                "rgblink",
                "while [ $# -gt 0 ]; do case $1 in -o|-n|-m) : > \"$2\"; shift;; esac; shift; done",
            ),
            ("rgbfix", "exit 0"),
        ],
    );
    let fake = Toolchain::discover(Some(&path)).unwrap();
    let made = failed(build(&fake, &options));
    let error = &made.diagnostics[0];
    assert_eq!(error.tool, Tool::Rgblink);
    assert_eq!(error.severity, Severity::Error);
    assert!(error.message.contains("empty"), "{}", error.message);
    assert!(
        error.message.contains("demo/build/.staging/demo.gb"),
        "{}",
        error.message
    );
}

#[test]
fn a_tool_that_fails_without_a_diagnostic_still_reports_one() {
    let (temp, options) = starter("silent-failure");
    let path = fake_tools(
        &temp,
        &[
            ("rgbasm", "echo 'something odd' >&2; exit 3"),
            ("rgblink", "exit 0"),
            ("rgbfix", "exit 0"),
        ],
    );
    let fake = Toolchain::discover(Some(&path)).unwrap();
    let made = failed(build(&fake, &options));
    assert_eq!(made.diagnostics.len(), 2, "one per source");
    let error = &made.diagnostics[0];
    assert_eq!(error.tool, Tool::Rgbasm);
    assert_eq!(error.file.as_deref(), Some("demo/src/main.asm"));
    assert!(error.message.contains("status 3"), "{}", error.message);
    assert!(error.message.contains("something odd"), "{}", error.message);
}
