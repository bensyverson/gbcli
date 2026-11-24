//! `gbcli serve --ephemeral`: the session lives in memory, so a harness that
//! drives one serve per test leaves nothing under `.gbcli/`, and every
//! request is answered as it is over a disk bundle.

use std::fs;

use gbcli_bundle::Bundle;
use gbcli_protocol::{ErrorCode, Response, Rev};

use crate::common::Project;

fn responses(stdout: &str) -> Vec<Response> {
    stdout
        .lines()
        .map(|l| serde_json::from_str(l).unwrap_or_else(|e| panic!("{e}: {l}")))
        .collect()
}

/// Head changes, reads, a checkpoint, undo and restore: the ops whose state
/// a disk session writes on every request.
const SCRIPT: &str = concat!(
    r#"{"id":"1","op":"show"}"#,
    "\n",
    r#"{"id":"2","op":"run","args":{"frames":2}}"#,
    "\n",
    r#"{"id":"3","op":"read","args":{"reads":["u16[$C000]","[$FF44]"]}}"#,
    "\n",
    r#"{"id":"4","op":"checkpoint","args":{"name":"a"}}"#,
    "\n",
    r#"{"id":"5","op":"run","args":{"frames":3}}"#,
    "\n",
    r#"{"id":"6","op":"undo"}"#,
    "\n",
    r#"{"id":"7","op":"run","args":{"frames":1}}"#,
    "\n",
    r#"{"id":"8","op":"restore","args":{"name":"a"}}"#,
    "\n",
    r#"{"id":"9","op":"checkpoints"}"#,
    "\n",
    r#"{"id":"10","op":"read","args":{"reads":["frame"]}}"#,
    "\n",
    r#"{"id":"11","op":"run","args":{"frames":1},"rev":"s0"}"#,
    "\n",
    r#"{"id":"12","op":"restore","args":{"name":"nope"}}"#,
    "\n",
);

const ARGS: [&str; 5] = ["serve", "--session", "demo", "--rom", "demo.gb"];

fn with_flag(flag: &'static str) -> Vec<&'static str> {
    let mut args = ARGS.to_vec();
    args.push(flag);
    args
}

#[test]
fn every_op_answers_as_it_does_over_a_disk_bundle() {
    let disk = Project::new("eph-same-disk");
    let memory = Project::new("eph-same-memory");
    let on_disk = disk.gbcli_with(&ARGS, &[], Some(SCRIPT));
    let in_memory = memory.gbcli_with(&with_flag("--ephemeral"), &[], Some(SCRIPT));
    assert_eq!(on_disk.code, 0, "{}", on_disk.stderr);
    assert_eq!(in_memory.code, 0, "{}", in_memory.stderr);
    assert!(in_memory.stderr.is_empty(), "{}", in_memory.stderr);
    let name = |run: &crate::common::Run, p: &Project| {
        run.stdout.replace(&p.dir.display().to_string(), "<dir>")
    };
    assert_eq!(name(&on_disk, &disk), name(&in_memory, &memory));
    // The script reaches its refusals, so the comparison covers errors too.
    let got = responses(&in_memory.stdout);
    assert_eq!(got.len(), 12);
    assert_eq!(
        got[10].error().map(|e| e.code),
        Some(ErrorCode::RevMismatch)
    );
    assert!(got[11].error().is_some(), "a missing checkpoint is refused");
}

#[test]
fn nothing_is_written_under_the_project() {
    let p = Project::new("eph-writes-nothing");
    let before = listing(&p);
    let run = p.gbcli_with(&with_flag("--ephemeral"), &[], Some(SCRIPT));
    assert_eq!(run.code, 0, "{}", run.stderr);
    assert!(!p.path(".gbcli").exists(), "a .gbcli/ was made");
    assert_eq!(listing(&p), before, "the project changed");
}

fn listing(p: &Project) -> Vec<String> {
    let mut names: Vec<String> = fs::read_dir(&p.dir)
        .unwrap()
        .map(|e| e.unwrap().file_name().to_string_lossy().into_owned())
        .collect();
    names.sort();
    names
}

#[test]
fn the_event_log_and_listing_are_kept_in_memory() {
    let p = Project::new("eph-events");
    let input = concat!(
        r#"{"id":"1","op":"run","args":{"frames":1}}"#,
        "\n",
        r#"{"id":"2","op":"sessions"}"#,
        "\n",
        r#"{"id":"3","op":"events"}"#,
        "\n",
    );
    let run = p.gbcli_with(&with_flag("--ephemeral"), &[], Some(input));
    assert_eq!(run.code, 0, "{}", run.stderr);
    let got = responses(&run.stdout);
    let sessions = got[1]
        .result()
        .expect("sessions")
        .sessions
        .as_ref()
        .unwrap();
    assert_eq!(sessions.len(), 1);
    assert_eq!(sessions[0].name, "demo");
    assert_eq!(sessions[0].rev, Rev(1));
    let events = got[2].result().expect("events").events.as_ref().unwrap();
    assert!(!events.is_empty(), "the run was logged");
}

#[test]
fn a_disk_bundle_is_neither_read_nor_changed() {
    let p = Project::with_session("eph-disk-untouched");
    let own = p.gbcli(&["run", "--session", "demo", "--frames", "5"]);
    assert_eq!(own.code, 0, "{}", own.stderr);
    let run = p.gbcli_with(&with_flag("--ephemeral"), &[], Some(SCRIPT));
    assert_eq!(run.code, 0, "{}", run.stderr);
    // The memory session started at power-on, not at the disk session's head.
    let got = responses(&run.stdout);
    assert_eq!(got[0].rev, Some(Rev(0)));
    assert!(got[0].session_created.is_some(), "{:?}", got[0]);
    let rev = Bundle::open_in(&p.dir, None)
        .unwrap()
        .session("demo")
        .unwrap()
        .read_meta()
        .unwrap()
        .rev;
    assert_eq!(rev, Rev(1), "the disk session was left at its own head");
}

#[test]
fn a_second_ephemeral_serve_does_not_find_the_first_ones_session() {
    let p = Project::new("eph-gone");
    let first = p.gbcli_with(&with_flag("--ephemeral"), &[], Some(SCRIPT));
    assert_eq!(first.code, 0, "{}", first.stderr);
    let second = p.gbcli_with(
        &["serve", "--session", "demo", "--ephemeral"],
        &[],
        Some("{\"id\":\"1\",\"op\":\"show\"}\n"),
    );
    // As with a disk bundle that lacks the named session, serve refuses to
    // start.
    assert_ne!(second.code, 0, "{}", second.stdout);
    assert!(second.stderr.contains("no_session"), "{}", second.stderr);
    assert!(second.stdout.is_empty(), "{}", second.stdout);
}

#[test]
fn gbcli_dir_does_not_move_an_ephemeral_session_onto_disk() {
    let p = Project::new("eph-gbcli-dir");
    let elsewhere = p.path("elsewhere");
    let env = [("GBCLI_DIR", elsewhere.to_str().unwrap())];
    let run = p.gbcli_with(&with_flag("--ephemeral"), &env, Some(SCRIPT));
    assert_eq!(run.code, 0, "{}", run.stderr);
    assert!(!elsewhere.exists(), "GBCLI_DIR was created");
    assert!(!p.path(".gbcli").exists());
}

/// The boot cache is the one thing worth sharing between processes, so it
/// stays on disk where `GBCLI_BOOT_CACHE` names it, and is in memory when
/// nothing does.
#[test]
fn the_shared_boot_cache_is_still_written_to_disk() {
    let p = Project::new("eph-boot-cache");
    let cache = p.path("shared-cache");
    let env = [("GBCLI_BOOT_CACHE", cache.to_str().unwrap())];
    let run = p.gbcli_with(&with_flag("--ephemeral"), &env, Some(SCRIPT));
    assert_eq!(run.code, 0, "{}", run.stderr);
    let entries = fs::read_dir(&cache).expect("the cache dir").count();
    assert!(entries > 0, "no boot state was cached");
    assert!(!p.path(".gbcli").exists());
}

#[test]
fn the_files_a_request_writes_still_reach_the_project() {
    let p = Project::new("eph-shot");
    let input = concat!(
        r#"{"id":"1","op":"run","args":{"frames":2,"shot":{"path":"out.png"}}}"#,
        "\n",
    );
    let run = p.gbcli_with(&with_flag("--ephemeral"), &[], Some(input));
    assert_eq!(run.code, 0, "{}", run.stderr);
    assert!(responses(&run.stdout)[0].is_ok(), "{}", run.stdout);
    assert!(p.path("out.png").is_file(), "the shot was not written");
    assert!(!p.path(".gbcli").exists());
}

#[test]
fn a_serve_with_no_session_makes_no_bundle_and_still_answers() {
    let p = Project::new("eph-bare");
    let input = concat!(
        r#"{"id":"v","op":"version"}"#,
        "\n",
        r#"{"id":"r","op":"run","args":{"frames":1}}"#,
        "\n",
    );
    let run = p.gbcli_with(&["serve", "--ephemeral"], &[], Some(input));
    assert_eq!(run.code, 0, "{}", run.stderr);
    let got = responses(&run.stdout);
    assert!(got[0].is_ok(), "{:?}", got[0]);
    assert_eq!(got[1].error().map(|e| e.code), Some(ErrorCode::NoSession));
    assert!(!p.path(".gbcli").exists());
}

#[test]
fn the_flag_is_serves_alone() {
    let p = Project::new("eph-only-serve");
    let run = p.gbcli(&["show", "--ephemeral"]);
    assert_ne!(run.code, 0);
    assert!(run.stderr.contains("--ephemeral"), "{}", run.stderr);
}
