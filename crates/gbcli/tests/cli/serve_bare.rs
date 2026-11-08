//! What a bare `serve` (no bundle at start) says when a bundle op arrives:
//! the cause the store gave for having none, not just that there is none.

use gbcli_protocol::{ErrorCode, Response};

use crate::common::Project;

fn responses(stdout: &str) -> Vec<Response> {
    stdout
        .lines()
        .map(|l| serde_json::from_str(l).unwrap_or_else(|e| panic!("{e}: {l}")))
        .collect()
}

const SHOW: &str = "{\"id\":\"s\",\"op\":\"show\"}\n";

/// A mistyped `GBCLI_DIR` still starts `serve` (the ops that need no bundle
/// stay reachable), but each bundle op says what is wrong with the variable.
#[test]
fn a_mistyped_gbcli_dir_is_named_in_each_bundle_ops_refusal() {
    let p = Project::with_session("bare-gbcli-dir");
    let missing = p.path("no-such-dir");
    let env = [("GBCLI_DIR", missing.to_str().unwrap())];
    let serve = p.gbcli_with(&["serve", "--session", "demo"], &env, Some(SHOW));
    assert_eq!(serve.code, 0, "{}", serve.stderr);
    let got = responses(&serve.stdout);
    let e = got[0].error().expect("refused");
    assert_eq!(e.code, ErrorCode::NoSession);
    assert!(e.message.contains("GBCLI_DIR names"), "{}", e.message);
    assert!(e.message.contains("\"demo\""), "{}", e.message);
    assert!(e.remedy.contains("GBCLI_DIR"), "{}", e.remedy);
}

/// A `.gbcli` that is a regular file is no bundle either; the refusal says
/// where the search looked.
#[test]
fn a_gbcli_file_shows_where_the_search_looked() {
    let p = Project::new("bare-gbcli-file");
    std::fs::write(p.path(".gbcli"), "not a directory").unwrap();
    let serve = p.gbcli_with(&["serve"], &[], Some(SHOW));
    assert_eq!(serve.code, 0, "{}", serve.stderr);
    let e = responses(&serve.stdout)[0]
        .error()
        .cloned()
        .expect("refused");
    assert_eq!(e.code, ErrorCode::NoSession);
    assert!(e.message.contains("no .gbcli/ in"), "{}", e.message);
}

/// A named session in a fresh directory is named in the refusal, with where
/// the search looked.
#[test]
fn a_named_session_is_named_when_there_is_no_bundle() {
    let p = Project::new("bare-session-named");
    let serve = p.gbcli_with(&["serve", "--session", "x"], &[], Some(SHOW));
    assert_eq!(serve.code, 0, "{}", serve.stderr);
    let e = responses(&serve.stdout)[0]
        .error()
        .cloned()
        .expect("refused");
    assert_eq!(e.code, ErrorCode::NoSession);
    assert!(e.message.contains("\"x\""), "{}", e.message);
    assert!(e.message.contains("no .gbcli/ in"), "{}", e.message);
}

/// With no session named and nothing wrong but absence, the refusal keeps
/// its short form and remedy.
#[test]
fn a_fresh_directory_keeps_the_plain_refusal_and_remedy() {
    let p = Project::new("bare-plain");
    let serve = p.gbcli_with(&["serve"], &[], Some(SHOW));
    let e = responses(&serve.stdout)[0]
        .error()
        .cloned()
        .expect("refused");
    assert!(
        e.message.contains("no project bundle here"),
        "{}",
        e.message
    );
    assert!(
        e.remedy.contains("gbcli run game.gb --session"),
        "{}",
        e.remedy
    );
}
