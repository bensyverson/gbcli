//! What a build leaves behind when it fails, and what it never touches: the
//! last good ROM, files gbcli didn't make, a tool that hangs, a symlink loop.

mod common;

use std::fs;
use std::path::PathBuf;
use std::time::{Duration, Instant};

use gbcli_build::{BuildError, Toolchain, build};
use gbcli_protocol::{Severity, Tool};

use common::{Temp, failed, fake_tools, real, rgbds, starter};

/// The ROM, `.sym` and `.map` of the starter's default build.
fn outputs(temp: &Temp) -> Vec<Vec<u8>> {
    ["demo.gb", "demo.sym", "demo.map"]
        .iter()
        .map(|f| fs::read(temp.path(&format!("demo/build/{f}"))).unwrap())
        .collect()
}

#[test]
fn a_failed_link_leaves_the_last_good_rom_sym_and_map_in_place() {
    let (temp, options) = starter("failed-link");
    build(&rgbds(), &options).unwrap();
    let good = outputs(&temp);
    // A linker that writes all three outputs, then fails.
    let path = fake_tools(
        &temp,
        &[
            ("rgbasm", &real(Tool::Rgbasm, "")),
            (
                "rgblink",
                "while [ $# -gt 0 ]; do case $1 in -o|-n|-m) echo partial > \"$2\"; shift;; esac; \
                 shift; done; echo 'error: boom' >&2; exit 1",
            ),
            ("rgbfix", &real(Tool::Rgbfix, "")),
        ],
    );
    let tools = Toolchain::discover(Some(&path), &temp.0).unwrap();
    let made = failed(build(&tools, &options));
    assert_eq!(made.diagnostics[0].tool, Tool::Rgblink);
    assert_eq!(outputs(&temp), good);
}

#[test]
fn a_failed_rgbfix_leaves_the_last_good_rom_sym_and_map_in_place() {
    let (temp, options) = starter("failed-fix");
    build(&rgbds(), &options).unwrap();
    let good = outputs(&temp);
    // rgbfix that scribbles on the ROM it was given, then fails.
    let path = fake_tools(
        &temp,
        &[
            ("rgbasm", &real(Tool::Rgbasm, "")),
            ("rgblink", &real(Tool::Rgblink, "")),
            (
                "rgbfix",
                "for last; do :; done; echo scribbled > \"$last\"; echo 'error: boom' >&2; exit 1",
            ),
        ],
    );
    let tools = Toolchain::discover(Some(&path), &temp.0).unwrap();
    let made = failed(build(&tools, &options));
    assert_eq!(made.diagnostics[0].tool, Tool::Rgbfix);
    assert_eq!(outputs(&temp), good);
}

#[test]
fn a_failed_builds_remedy_names_the_directory_it_built() {
    let (temp, options) = starter("remedy");
    fs::write(temp.path("demo/src/extra.asm"), "\tld q, 1\n").unwrap();
    let body = build(&rgbds(), &options).unwrap_err().body();
    assert!(
        body.remedy.contains("`gbcli build demo`"),
        "{}",
        body.remedy
    );

    // Built from inside the project, the command takes no directory.
    let mut here = options.clone();
    here.display_base = temp.path("demo");
    let body = build(&rgbds(), &here).unwrap_err().body();
    assert!(body.remedy.contains("`gbcli build`"), "{}", body.remedy);
}

#[test]
fn staging_never_removes_a_directory_gbcli_didnt_make() {
    let (temp, mut options) = starter("staging-own");
    // With the ROM in the project's root, the old staging directory was
    // `<project>/.staging`: perhaps the user's.
    options.out = Some(PathBuf::from("game.gb"));
    fs::create_dir(temp.path("demo/.staging")).unwrap();
    fs::write(temp.path("demo/.staging/mine.txt"), "mine").unwrap();
    build(&rgbds(), &options).unwrap();
    assert_eq!(
        fs::read_to_string(temp.path("demo/.staging/mine.txt")).unwrap(),
        "mine"
    );
    let left: Vec<String> = fs::read_dir(temp.path("demo"))
        .unwrap()
        .map(|e| e.unwrap().file_name().to_string_lossy().into_owned())
        .filter(|n| n.contains("staging") && n != ".staging")
        .collect();
    assert!(left.is_empty(), "staging is cleaned up: {left:?}");
}

#[test]
fn the_rom_moves_into_place_last_so_its_arrival_marks_a_complete_build() {
    let (temp, options) = starter("rename-order");
    // A directory where the ROM goes makes its move fail: whatever moved
    // before it has moved.
    fs::create_dir_all(temp.path("demo/build/demo.gb/blocker")).unwrap();
    match build(&rgbds(), &options) {
        Err(BuildError::Io { .. }) => {}
        other => panic!("expected an io failure, got {other:?}"),
    }
    assert!(temp.path("demo/build/demo.sym").is_file());
    assert!(temp.path("demo/build/demo.map").is_file());
}

#[test]
fn a_tool_that_hangs_is_stopped_and_the_build_fails_naming_it() {
    let (temp, mut options) = starter("timeout");
    let path = fake_tools(
        &temp,
        &[
            ("rgbasm", "exec sleep 5"),
            ("rgblink", "exit 0"),
            ("rgbfix", "exit 0"),
        ],
    );
    let tools = Toolchain::discover(Some(&path), &temp.0).unwrap();
    options.tool_timeout = Duration::from_millis(200);
    let started = Instant::now();
    let made = failed(build(&tools, &options));
    assert!(
        started.elapsed() < Duration::from_secs(3),
        "{:?}",
        started.elapsed()
    );
    let error = &made.diagnostics[0];
    assert_eq!(error.tool, Tool::Rgbasm);
    assert_eq!(error.severity, Severity::Error);
    assert!(error.message.contains("stopped"), "{}", error.message);
    assert!(error.message.contains("0.2 s"), "{}", error.message);
}

#[cfg(unix)]
#[test]
fn a_symlink_loop_under_src_is_followed_once() {
    let (temp, options) = starter("symlink-loop");
    std::os::unix::fs::symlink(".", temp.path("demo/src/again")).unwrap();
    let made = build(&rgbds(), &options).unwrap();
    assert_eq!(made.sources, ["demo/src/main.asm", "demo/src/oam_dma.asm"]);
}
