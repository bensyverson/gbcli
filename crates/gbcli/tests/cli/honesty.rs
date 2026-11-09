//! Arguments are honored or refused, never silently ignored: session
//! settings that disagree with the session, `--rev` on every change, `--from`
//! with `--at`, bundles made only by what can create a session, flags that
//! don't apply, and `--dir` over `GBCLI_DIR`.

use std::fs;

use crate::common::{Project, V1_SYM};

const S: [&str; 2] = ["--session", "demo"];

fn args<'a>(head: &[&'a str]) -> Vec<&'a str> {
    let mut all = head.to_vec();
    all.extend(S);
    all
}

#[test]
fn a_session_setting_that_differs_from_the_session_is_refused() {
    let p = Project::with_session("settings-differ");
    fs::write(p.path("other.sym"), V1_SYM).unwrap();
    let refused = p.gbcli(&args(&[
        "run", "demo.gb", "--model", "cgb-e", "--frames", "1",
    ]));
    refused.fails(2, "bad_request", "drop --model");
    assert!(refused.stderr.contains("dmg-b"), "{}", refused.stderr);
    let refused = p.gbcli(&args(&["show", "--rom", "demo.gb", "--seed", "7"]));
    refused.fails(2, "bad_request", "drop --seed");
    assert!(refused.stderr.contains('0'), "{}", refused.stderr);
    p.gbcli(&args(&["show", "--rom", "demo.gb", "--sym", "other.sym"]))
        .fails(2, "bad_request", "drop --sym");
}

#[test]
fn a_session_setting_equal_to_the_sessions_is_accepted() {
    let p = Project::with_session("settings-same");
    let same = [
        "show", "--rom", "demo.gb", "--model", "dmg-b", "--seed", "0", "--sym", "demo.sym",
    ];
    let run = p.gbcli(&args(&same));
    assert_eq!(run.code, 0, "{}", run.stderr);
}

#[test]
fn rev_guards_every_change_to_the_session() {
    let p = Project::with_session("rev-everywhere");
    for change in [
        &["checkpoint", "here"][..],
        &["break", "Main.loop"],
        &["watch", "write", "wSpeed"],
        &["delete", "1"],
    ] {
        let mut a = args(change);
        a.extend(["--rev", "s9"]);
        p.gbcli(&a).fails(3, "rev_mismatch", "re-read the session");
    }
}

#[test]
fn rev_on_a_verb_that_changes_nothing_is_a_usage_error() {
    let p = Project::with_session("rev-read");
    for read in [
        &["show"][..],
        &["read", "pc"],
        &["checkpoints"],
        &["events"],
    ] {
        let mut a = args(read);
        a.extend(["--rev", "s0"]);
        p.gbcli(&a).fails(2, "bad_request", "drop --rev");
    }
}

#[test]
fn from_with_at_is_refused_and_the_help_says_power_on_skips_the_head() {
    let p = Project::with_session("from-at");
    p.gbcli(&args(&["checkpoint", "zero"]));
    p.gbcli(&args(&[
        "run", "--at", "zero", "--from", "power-on", "--frames", "1",
    ]))
    .fails(2, "bad_request", "drop one of them");
    let help = p.gbcli(&["run", "--help"]);
    let from = help.stdout.lines().find(|l| l.contains("--from")).unwrap();
    assert!(from.contains("skips the head"), "{from}");
}

#[test]
fn only_an_invocation_that_can_create_a_session_creates_a_bundle() {
    let p = Project::new("no-bundle");
    for verb in [
        &["show", "--session", "demo"][..],
        &["sessions"],
        &["gaps"],
        &["events", "--session", "demo"],
    ] {
        let run = p.gbcli(verb);
        run.fails(5, "no_session", "gbcli run");
        // The directory searched, by name: the child may see it through a
        // symlink (macOS's /var is /private/var).
        let name = p.dir.file_name().unwrap().to_string_lossy().to_string();
        assert!(run.stderr.contains(&name), "{}", run.stderr);
        assert!(!p.path(".gbcli").exists(), "{verb:?} made a bundle");
    }
    let made = p.gbcli(&["show", "--session", "demo", "--rom", "demo.gb"]);
    assert_eq!(made.code, 0, "{}", made.stderr);
    assert!(p.path(".gbcli").is_dir());
}

#[test]
fn view_map_with_tiles_or_oam_is_refused() {
    let p = Project::with_session("view-map");
    for what in ["tiles", "oam"] {
        p.gbcli(&args(&["view", what, "--map", "window"]))
            .fails(2, "bad_request", "drop --map");
    }
}

#[test]
fn live_with_any_frame_is_refused() {
    let p = Project::with_session("live-any");
    p.gbcli(&args(&["shot", "a.png", "--live", "--any-frame"]))
        .fails(2, "bad_request", "drop one of them");
    p.gbcli(&args(&[
        "run",
        "--frames",
        "1",
        "--shot",
        "a.png",
        "--live",
        "--any-frame",
    ]))
    .fails(2, "bad_request", "drop one of them");
}

#[test]
fn frames_help_says_what_the_default_budget_covers() {
    let help = Project::new("frames-help").gbcli(&["run", "--help"]);
    let frames = help
        .stdout
        .lines()
        .find(|l| l.starts_with("  --frames"))
        .unwrap();
    assert!(frames.contains("any run without a budget"), "{frames}");
}

#[test]
fn the_watch_recipe_works_as_written() {
    let p = Project::with_session("watch-recipe");
    let watch = p.gbcli(&args(&["watch", "write", "wSpeed"]));
    assert_eq!(watch.code, 0, "{}", watch.stderr);
    let run = p.gbcli(&args(&["run"]));
    assert_eq!(run.code, 0, "{}", run.stderr);
    assert!(run.stdout.starts_with("stop watchpoint"), "{}", run.stdout);
}

/// `--dir` runs the command as if from PATH: the ROM path, the shot's and
/// the keys file's resolve there, and the session is made there (leaf
/// emtFvQ; the racer's agent gave `--dir` on every call and still had to
/// spell the ROM path from the cwd).
#[test]
fn dir_anchors_the_paths_the_command_is_given() {
    let p = Project::new("dir-anchors");
    let game = p.path("game");
    fs::create_dir_all(game.join("build")).unwrap();
    fs::rename(p.path("demo.gb"), game.join("build/demo.gb")).unwrap();
    fs::rename(p.path("demo.sym"), game.join("build/demo.sym")).unwrap();
    fs::write(game.join("keys.txt"), "START@0 -START@1 A@90").unwrap();
    let run = p.gbcli(&args(&[
        "run",
        "build/demo.gb",
        "--dir",
        "game",
        "--frames",
        "2",
        "--keys-file",
        "keys.txt",
        "--shot",
        "two.png",
        "--read",
        "u16[wFrame]",
    ]));
    assert_eq!(run.code, 0, "{}", run.stderr);
    assert!(
        run.stdout.contains("keys_unapplied count=1 first=A@90"),
        "{}",
        run.stdout
    );
    assert!(game.join("two.png").is_file(), "the shot went elsewhere");
    assert!(
        game.join(".gbcli/sessions/demo").is_dir(),
        "no session in game/"
    );
    assert!(!p.path(".gbcli").exists(), "a bundle in the cwd");
}

#[test]
fn dir_beats_gbcli_dir() {
    let p = Project::with_session("dir-beats-env");
    let elsewhere = p.path("elsewhere");
    fs::create_dir_all(elsewhere.join(".gbcli")).unwrap();
    let env = [("GBCLI_DIR", elsewhere.join(".gbcli").display().to_string())];
    let env: Vec<(&str, &str)> = env.iter().map(|(k, v)| (*k, v.as_str())).collect();
    let dir = p.dir.display().to_string();
    let run = p.gbcli_with(&args(&["show", "--dir", &dir]), &env, None);
    assert_eq!(run.code, 0, "--dir lost to GBCLI_DIR: {}", run.stderr);
}
