//! A host that denies host files (a memory store's caller) refuses every op
//! that would reach them outside the store, before it touches the disk,
//! and still answers everything else.

use std::path::{Path, PathBuf};
use std::sync::Arc;

use gbcli_bundle::{Bundle, Clock, Key, MemoryStore, ProjectStore, RomSource};
use gbcli_protocol::{
    AssetsArgs, BuildArgs, ErrorCode, FontArgs, FontDiffArgs, FontEditArgs, FontExportArgs,
    FontImportArgs, FontNewArgs, FontSaveArgs, NewArgs, Op, OpcodeArgs, PixelsArgs, RunArgs,
    SongArgs,
};

use super::Handler;
use super::test_support::request;
use crate::fixture::V1;
use crate::serve::Answer;
use crate::{Host, HostFiles, InterruptFlag, Tooling};

#[derive(Debug)]
struct Noon;

impl Clock for Noon {
    fn now_secs(&self) -> u64 {
        1_700_000_000
    }
}

/// A real, empty directory: what a denied host must leave empty.
fn empty_dir(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("gbcli-hostfiles-{}-{name}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::canonicalize(&dir).unwrap()
}

fn on_disk(dir: &Path) -> Vec<PathBuf> {
    std::fs::read_dir(dir)
        .unwrap()
        .map(|e| e.unwrap().path())
        .collect()
}

/// A `demo` session in a memory store whose project is `dir`, and a
/// handler whose host works there and denies host files.
fn denied_handler(dir: &Path) -> Handler {
    let clock = Arc::new(Noon);
    let store = Arc::new(MemoryStore::new(dir, clock.clone()).unwrap());
    store.put(&Key::new("demo.gb"), V1).unwrap();
    store.make_dir(&Key::new(".gbcli")).unwrap();
    let bundle = Bundle::new(store, Key::new(".gbcli"), clock);
    let source = RomSource {
        rom: "demo.gb".into(),
        sym: None,
        model: None,
        seed: Some(0),
        flash: None,
    };
    bundle.open_session("demo", Some(&source)).unwrap();
    let host = Host::new(dir, HostFiles::Denied).unwrap();
    Handler::new(host, bundle, Box::new(InterruptFlag::new()))
        .with_defaults(Some("demo".into()), None)
}

/// Every op that reaches host files, each with arguments that would write
/// on an allowed host where they could.
fn host_ops() -> Vec<Op> {
    let yaff = || "main.yaff".to_string();
    vec![
        Op::New(NewArgs { dir: "game".into() }),
        Op::Doctor,
        Op::Build(BuildArgs::default()),
        Op::Assets(AssetsArgs::default()),
        Op::Pixels(PixelsArgs {
            file: "hero.pixels".into(),
            ..PixelsArgs::default()
        }),
        Op::Song(SongArgs {
            path: "theme.song".into(),
            descriptor: "song_theme".into(),
        }),
        Op::Font(FontArgs {
            file: yaff(),
            ..FontArgs::default()
        }),
        Op::FontEdit(FontEditArgs {
            file: yaff(),
            ..FontEditArgs::default()
        }),
        Op::FontSave(FontSaveArgs::default()),
        Op::FontImport(FontImportArgs::default()),
        Op::FontExport(FontExportArgs::default()),
        Op::FontNew(FontNewArgs {
            name: "mine".into(),
            chars: "ascii".into(),
            guide_sheet: None,
        }),
        Op::FontDiff(FontDiffArgs {
            a: yaff(),
            b: yaff(),
            png: None,
        }),
    ]
}

fn assert_refused(answer: &mut dyn FnMut(&Op) -> gbcli_protocol::Response) {
    for op in host_ops() {
        let name = op.name();
        let error = answer(&op).outcome.expect_err(name);
        assert_eq!(error.code, ErrorCode::Unsupported, "{name}: {error:?}");
        assert!(error.message.contains(name), "{}", error.message);
        assert!(
            error.message.contains("denies host files"),
            "{}",
            error.message
        );
        assert!(error.remedy.contains("gbcli serve"), "{}", error.remedy);
    }
}

#[test]
fn a_handler_whose_host_denies_host_files_refuses_them_and_writes_nothing() {
    let dir = empty_dir("handler");
    let mut h = denied_handler(&dir);
    assert_refused(&mut |op| h.handle(&request(op.clone())));
    // What touches no host file still answers.
    let opcode = Op::Opcode(OpcodeArgs {
        query: "$20".into(),
    });
    for op in [opcode, Op::Version, Op::Sessions, Op::Gaps] {
        let answered = h.handle(&request(op.clone()));
        assert!(answered.is_ok(), "{}: {:?}", op.name(), answered.error());
    }
    let ran = h.handle(&request(Op::Run(RunArgs {
        frames: Some(1),
        ..RunArgs::default()
    })));
    assert!(ran.is_ok(), "{:?}", ran.error());
    let left = on_disk(&dir);
    let _ = std::fs::remove_dir_all(&dir);
    assert!(left.is_empty(), "{left:?}");
}

#[test]
fn the_bundleless_side_on_a_denied_host_refuses_them_too() {
    let dir = empty_dir("tooling");
    let mut tooling = Tooling::new(Host::new(&dir, HostFiles::Denied).unwrap());
    assert_refused(&mut |op| tooling.answer(&request(op.clone())));
    let version = tooling.answer(&request(Op::Version));
    assert!(version.is_ok(), "{:?}", version.error());
    let left = on_disk(&dir);
    let _ = std::fs::remove_dir_all(&dir);
    assert!(left.is_empty(), "{left:?}");
}

/// The control: on an allowed host the same `font_new` writes its file, so
/// the refusal above is what kept the disk empty.
#[test]
fn an_allowed_host_still_reaches_host_files() {
    let dir = empty_dir("allowed");
    let host = Host::new(&dir, HostFiles::Allowed { path: None }).unwrap();
    let made = Tooling::new(host).answer(&request(Op::FontNew(FontNewArgs {
        name: "mine.yaff".into(),
        chars: "ascii".into(),
        guide_sheet: None,
    })));
    let left = on_disk(&dir);
    let _ = std::fs::remove_dir_all(&dir);
    assert!(made.is_ok(), "{:?}", made.error());
    assert!(!left.is_empty(), "font_new wrote nothing");
}
