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
use crate::{Host, InterruptFlag};

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
    let store = Arc::new(MemoryStore::new(PROJECT, clock.clone()));
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
    let host = Host::new(cwd, None).unwrap();
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
