//! A handler over a memory store. The server resolves a request's paths
//! against its host's working directory before the store sees them, so a
//! host must sit under the store's project path for those paths to be
//! project keys; anywhere else they are keys outside the project, which a
//! memory store holds only where the caller made a directory.

use std::sync::Arc;

use gbcli_bundle::{Bundle, Clock, Key, MemoryStore, ProjectStore, RomSource};
use gbcli_core::Model;
use gbcli_protocol::{Op, RunArgs, ShotArgs};

use super::Handler;
use super::test_support::request;
use crate::fixture::V1;
use crate::{Host, HostFiles, InterruptFlag};

const PROJECT: &str = "/mem/project";

#[derive(Debug)]
struct Noon;

impl Clock for Noon {
    fn now_secs(&self) -> u64 {
        1_700_000_000
    }
}

/// A `demo` session over V1 in a memory store at [`PROJECT`], with a
/// `shots/` directory, and a handler whose host works in `cwd`.
fn memory_handler(cwd: &str) -> (Handler, Arc<MemoryStore>) {
    let clock = Arc::new(Noon);
    let store = Arc::new(MemoryStore::new(PROJECT, clock.clone()).unwrap());
    store.put(&Key::new("demo.gb"), V1).unwrap();
    store.make_dir(&Key::new("shots")).unwrap();
    store.make_dir(&Key::new(".gbcli")).unwrap();
    let bundle = Bundle::new(store.clone(), Key::new(".gbcli"), clock);
    let source = RomSource {
        rom: "demo.gb".into(),
        sym: None,
        model: Some(Model::DmgB),
        seed: Some(0),
        flash: None,
    };
    bundle.open_session("demo", Some(&source)).unwrap();
    let host = Host::new(cwd, HostFiles::Denied).unwrap();
    let handler = Handler::new(host, bundle, Box::new(InterruptFlag::new()))
        .with_defaults(Some("demo".into()), None);
    (handler, store)
}

fn run_with_shot(h: &mut Handler, path: &str) -> gbcli_protocol::Response {
    h.handle(&request(Op::Run(RunArgs {
        frames: Some(3),
        shot: Some(Box::new(ShotArgs {
            path: path.into(),
            ..ShotArgs::default()
        })),
        ..RunArgs::default()
    })))
}

#[test]
fn a_host_under_the_stores_project_writes_a_relative_shot_as_a_project_key() {
    let (mut h, store) = memory_handler("/mem/project/shots");
    let response = run_with_shot(&mut h, "a.png");
    assert!(response.is_ok(), "{:?}", response.error());
    let png = store.get(&Key::new("shots/a.png")).expect("a project key");
    assert_eq!(&png[..8], b"\x89PNG\r\n\x1a\n");
    let absolute = run_with_shot(&mut h, "/mem/project/b.png");
    assert!(absolute.is_ok(), "{:?}", absolute.error());
    assert!(store.get(&Key::new("b.png")).is_ok());
}

#[test]
fn a_host_outside_the_stores_project_names_keys_outside_it_and_writes_nothing() {
    let (mut h, store) = memory_handler("/mem/elsewhere");
    let response = run_with_shot(&mut h, "a.png");
    assert!(response.error().is_some(), "{response:?}");
    assert_eq!(store.kind(&Key::new("/mem/elsewhere/a.png")), None);
    assert_eq!(store.kind(&Key::new("a.png")), None);
    // The session still runs: only the shot's path is outside.
    let ran = h.handle(&request(Op::Run(RunArgs {
        frames: Some(1),
        ..RunArgs::default()
    })));
    assert!(ran.is_ok(), "{:?}", ran.error());
}

/// Every session op that writes a request's own file, and the bundle ops,
/// through a memory store whose project is a real, existing, empty
/// directory: each answers, its files land in the store as keys, and the
/// directory stays empty.
#[test]
fn session_ops_in_memory_write_their_files_as_keys_and_nothing_to_the_disk() {
    let dir = std::env::temp_dir().join(format!("gbcli-memsession-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let dir = std::fs::canonicalize(&dir).unwrap();
    let clock = Arc::new(Noon);
    let store = Arc::new(MemoryStore::new(&dir, clock.clone()).unwrap());
    store.put(&Key::new("demo.gb"), V1).unwrap();
    store
        .put(&Key::new("demo.sym"), crate::fixture::V1_SYM)
        .unwrap();
    store.make_dir(&Key::new(".gbcli")).unwrap();
    let bundle = Bundle::new(store.clone(), Key::new(".gbcli"), clock);
    let source = RomSource {
        rom: "demo.gb".into(),
        sym: None,
        model: Some(Model::DmgB),
        seed: Some(0),
        flash: None,
    };
    bundle.open_session("demo", Some(&source)).unwrap();
    let host = Host::new(&dir, HostFiles::Denied).unwrap();
    let mut h = Handler::new(host, bundle, Box::new(InterruptFlag::new()))
        .with_defaults(Some("demo".into()), Some("tool".into()));
    let lines = [
        r#"{"id":"1","op":"run","args":{"frames":4,"shot":{"path":"a.png"},"filmstrip":{"path":"strip.png","every":1},"audio":{"wav":{"path":"a.wav"}}}}"#,
        r#"{"id":"2","op":"view","args":{"what":"oam","png":{"path":"o.png"}}}"#,
        r#"{"id":"3","op":"view","args":{"what":"memory","addr":"$C000","len":2,"out":"m.bin","allow_uninit":true}}"#,
        r#"{"id":"4","op":"shot","args":{"shot":{"path":"s.png"}}}"#,
        r#"{"id":"5","op":"checkpoint","args":{"name":"c"}}"#,
        r#"{"id":"6","op":"sessions"}"#,
        r#"{"id":"7","op":"events"}"#,
        r#"{"id":"8","op":"gaps"}"#,
        r#"{"id":"9","op":"checkpoints"}"#,
        r#"{"id":"10","op":"prune","args":{"unused_for":"1d"}}"#,
        r#"{"id":"11","op":"power_cycle"}"#,
    ];
    for line in lines {
        let request = gbcli_protocol::parse_request(line).expect(line);
        let response = h.handle(&request);
        assert!(response.is_ok(), "{line}: {:?}", response.error());
    }
    let left: Vec<_> = std::fs::read_dir(&dir).unwrap().collect();
    let _ = std::fs::remove_dir_all(&dir);
    assert!(left.is_empty(), "{left:?}");
    for written in ["a.png", "strip.png", "a.wav", "o.png", "m.bin", "s.png"] {
        assert!(store.get(&Key::new(written)).is_ok(), "{written}");
    }
}
