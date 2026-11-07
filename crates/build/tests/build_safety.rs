//! What a build leaves behind when it fails, and what it never touches: the
//! last good ROM, files gbcli didn't make, a tool that hangs, a symlink loop.

mod common;

use std::fs;
use std::path::PathBuf;
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

use gbcli_build::{BuildError, BuildOptions, Toolchain, build, new_project};
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

/// Run again without `--src`, the build would fail for another reason (no
/// `src/`), so the remedy repeats every layout flag it was built with.
#[test]
fn a_failed_builds_remedy_keeps_the_layout_flags_it_was_built_with() {
    let (temp, mut options) = starter("remedy-flags");
    fs::rename(temp.path("demo/src"), temp.path("demo/code")).unwrap();
    fs::write(temp.path("demo/code/extra.asm"), "\tld q, 1\n").unwrap();
    options.src = vec![PathBuf::from("code")];
    options.include = vec![PathBuf::from("include")];
    options.out = Some(PathBuf::from("out/my game.gb"));
    let body = build(&rgbds(), &options).unwrap_err().body();
    assert!(
        body.remedy
            .contains("`gbcli build demo --src code --include include --out 'out/my game.gb'`"),
        "{}",
        body.remedy
    );
}

#[test]
fn a_failed_builds_remedy_is_a_command_a_shell_can_run() {
    let temp = Temp::new("remedy-space");
    fs::create_dir_all(temp.path("my games")).unwrap();
    new_project(&temp.path("my games/demo"), &temp.0).unwrap();
    fs::write(temp.path("my games/demo/src/extra.asm"), "\tld q, 1\n").unwrap();
    let options = BuildOptions::new(temp.path("my games/demo"), temp.0.clone());
    let body = build(&rgbds(), &options).unwrap_err().body();
    assert!(
        body.remedy.contains("`gbcli build 'my games/demo'`"),
        "{}",
        body.remedy
    );
}

/// A build killed midway (its server killed, say) never runs its cleanup;
/// the next build into the same directory removes what it left, and never
/// the staging directory of a build still running.
#[test]
fn a_killed_builds_staging_is_swept_by_the_next_build_and_a_live_ones_is_kept() {
    let (temp, options) = starter("staging-sweep");
    let build_dir = temp.path("demo/build");
    let killed = build_dir.join(".gbcli-staging-99999-0");
    fs::create_dir_all(&killed).unwrap();
    fs::write(killed.join("lock"), "").unwrap();
    fs::write(killed.join("demo.gb"), "half a rom").unwrap();
    // A build still running holds its staging directory's lock.
    let running = build_dir.join(".gbcli-staging-1-0");
    fs::create_dir_all(&running).unwrap();
    let held = fs::File::create(running.join("lock")).unwrap();
    held.lock().unwrap();

    build(&rgbds(), &options).unwrap();
    assert!(!killed.exists(), "a killed build's staging is swept");
    assert!(running.join("lock").is_file(), "a live build's is kept");
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

/// A tool behind a wrapper script (a version manager's shim, `sh` without
/// `exec`) runs the real program as the wrapper's child, holding stderr.
/// The time limit stops both, on time.
#[test]
fn a_wrapped_tool_that_hangs_is_stopped_on_time_with_its_child() {
    let (temp, mut options) = starter("timeout-wrapped");
    let pid_file = temp.path("child.pid");
    let script = format!("sleep 30 & echo $! > '{}'; wait", pid_file.display());
    let path = fake_tools(
        &temp,
        &[
            ("rgbasm", &script),
            ("rgblink", "exit 0"),
            ("rgbfix", "exit 0"),
        ],
    );
    let tools = Toolchain::discover(Some(&path), &temp.0).unwrap();
    options.tool_timeout = Duration::from_millis(200);
    let started = Instant::now();
    let made = failed(build(&tools, &options));
    let took = started.elapsed();
    assert!(
        took < Duration::from_secs(3),
        "the 0.2 s limit took {took:?}"
    );
    assert!(made.diagnostics[0].message.contains("stopped"));
    let pid = fs::read_to_string(&pid_file).unwrap();
    assert!(gone(pid.trim()), "the wrapper's child outlived the build");
}

/// A tool that exits but leaves a child holding its stderr (a daemonized
/// helper) doesn't hold the build past the time limit, and what it printed
/// before exiting is still read.
#[test]
fn a_tool_that_exits_leaving_a_child_on_stderr_does_not_hold_the_build() {
    let (temp, mut options) = starter("timeout-orphan");
    let path = fake_tools(
        &temp,
        &[
            ("rgbasm", "sleep 30 & echo 'error: boom' >&2; exit 1"),
            ("rgblink", "exit 0"),
            ("rgbfix", "exit 0"),
        ],
    );
    let tools = Toolchain::discover(Some(&path), &temp.0).unwrap();
    options.tool_timeout = Duration::from_millis(200);
    let started = Instant::now();
    let made = failed(build(&tools, &options));
    let took = started.elapsed();
    assert!(
        took < Duration::from_secs(3),
        "the 0.2 s limit took {took:?}"
    );
    assert!(
        made.diagnostics.iter().any(|d| d.message.contains("boom")),
        "{:?}",
        made.diagnostics
    );
}

/// Whether process `pid` is gone, allowing the system a moment to reap it.
fn gone(pid: &str) -> bool {
    let deadline = Instant::now() + Duration::from_secs(2);
    loop {
        let alive = Command::new("kill")
            .args(["-0", pid])
            .stderr(Stdio::null())
            .status()
            .unwrap()
            .success();
        if !alive {
            return true;
        }
        if Instant::now() > deadline {
            return false;
        }
        std::thread::sleep(Duration::from_millis(20));
    }
}

#[cfg(unix)]
#[test]
fn a_symlink_loop_under_src_is_followed_once() {
    let (temp, options) = starter("symlink-loop");
    std::os::unix::fs::symlink(".", temp.path("demo/src/again")).unwrap();
    let made = build(&rgbds(), &options).unwrap();
    assert_eq!(
        made.sources,
        [
            "demo/src/hUGEDriver.asm",
            "demo/src/main.asm",
            "demo/src/music.asm",
            "demo/src/oam_dma.asm",
            "demo/build/music/theme.asm",
        ]
    );
}
