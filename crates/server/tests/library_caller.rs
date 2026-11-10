//! **The library entry point.** A Rust caller (an agent's tool, a web
//! page's worker) drives gbcli in-process through [`Handler::handle`]: typed
//! [`Request`]s in, typed responses out, the same protocol `gbcli serve`
//! speaks, with no CLI, no subprocess and no `PATH`. The test below is the
//! worked example; the construction it needs is:
//!
//! 1. **A store.** [`MemoryStore::new`] takes the project's path (absolute,
//!    or it is refused; nothing need exist there) and a [`Clock`]. The
//!    caller seeds it with the project's files under project keys
//!    ([`ProjectStore::put`] of the ROM, and its `.sym`, which a session
//!    finds beside the ROM) and makes the bundle's directory (`.gbcli`). A
//!    filesystem caller uses `Bundle::open_or_create` instead, which builds a
//!    `FileStore`.
//! 2. **A [`Bundle`]** over the store: [`Bundle::new`] with the bundle's key
//!    and the clock, the host time that logs, listing and pruning read
//!    (emulation never does).
//! 3. **The session**, created from the ROM's key with
//!    [`Bundle::open_session`], as the CLI and `gbcli serve` create theirs
//!    before their first request: no protocol op creates one.
//! 4. **A [`Host`]** whose working directory sits under the store's project
//!    path, so a request's relative paths (a shot's PNG) are project keys,
//!    and which denies host files ([`HostFiles::Denied`]). [`Host::new`]
//!    refuses a relative directory.
//! 5. **An interrupt.** [`InterruptFlag`] starts no thread: the caller sets
//!    its flag to stop the request in flight, once the request is running
//!    (arming each request clears it). That stop answers `wall_guard` with
//!    a `user_break` stop and a message naming the caller. A native caller
//!    that wants `wall_guard_s` enforced passes a [`gbcli_server::Watchdog`]
//!    instead.
//! 6. **[`Handler::new`]** with the three, and the default session and actor
//!    ([`Handler::with_defaults`]); then [`Handler::handle`] per request.
//!
//! **What a memory caller can do:** every session op (run, step, read,
//! write, shot, view, sym, measure, equiv, checkpoint, restore, undo,
//! power_cycle, sav, events, the debugger's break, watch and delete), the
//! bundle ops (`sessions`, `gaps`, `prune`, `rm`), `opcode` and `version`.
//! Every file those read or write (the ROM, a shot, a WAV, a memory dump, a
//! `.sav`, the bundle) is a key in the store, where the caller reads it
//! back; nothing reaches the disk.
//!
//! **What it can't:** the ops that go to the host's files or run a program
//! outside the store: `build`, `doctor`, `new`, `assets`, `pixels`, `font`
//! and the font authoring ops (`font_edit`, `font_save`, `font_import`,
//! `font_export`, `font_new`, `font_diff`). A host that denies host files
//! refuses each with `unsupported` before it touches anything.

use std::path::{Path, PathBuf};
use std::sync::Arc;

use gbcli_bundle::{Bundle, Clock, Key, MemoryStore, ProjectStore, RomSource};
use gbcli_protocol::{
    Button, CheckpointArgs, ErrorCode, KeyAction, KeyEvent, NewArgs, Op, ReadArgs, Request,
    Response, RestoreArgs, ResultBody, RunArgs, ShotArgs, ShotReadArgs,
};
use gbcli_server::{Handler, Host, HostFiles, InterruptFlag};

/// A block moved by the d-pad: `wX` counts the frames Right was held.
const ROM: &[u8] = include_bytes!("../../engine/tests/roms/equiv_a.gb");
const SYM: &[u8] = include_bytes!("../../engine/tests/roms/equiv_a.sym");

/// The caller's own clock: host time, which no decision here depends on.
#[derive(Debug)]
struct CallerClock;

impl Clock for CallerClock {
    fn now_secs(&self) -> u64 {
        1_700_000_000
    }
}

/// Where the project is, as the host names it: a real, empty directory, so
/// anything that reached the disk instead of the store would show there.
fn empty_project() -> PathBuf {
    let dir = std::env::temp_dir().join(format!("gbcli-library-caller-{}", std::process::id()));
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

fn request(op: Op) -> Request {
    Request {
        id: "1".into(),
        session: None,
        actor: None,
        rev: None,
        allow_rom_change: false,
        wall_guard_s: None,
        op,
    }
}

fn ok(response: Response) -> ResultBody {
    match response.outcome {
        Ok(body) => body,
        Err(error) => panic!("{error:?}"),
    }
}

/// Runs `frames` frames holding Right from the first to the last.
fn run_right(handler: &mut Handler, frames: u64) -> ResultBody {
    let key = |at, action| KeyEvent {
        at,
        button: Button::Right,
        action,
    };
    ok(handler.handle(&request(Op::Run(RunArgs {
        frames: Some(frames),
        keys: vec![key(0, KeyAction::Press), key(frames, KeyAction::Release)],
        ..RunArgs::default()
    }))))
}

fn read_x(handler: &mut Handler) -> i64 {
    let read = ok(handler.handle(&request(Op::Read(ReadArgs {
        reads: vec!["[wX]".into()],
        since: None,
        allow_uninit: false,
    }))));
    read.reads.expect("reads")[0].value.expect("a value")
}

#[test]
fn a_library_caller_drives_a_session_through_handle_alone() {
    let project = empty_project();

    // 1-2. The store, seeded, and the bundle over it.
    let clock = Arc::new(CallerClock);
    let store = Arc::new(MemoryStore::new(&project, clock.clone()).unwrap());
    store.put(&Key::new("game.gb"), ROM).unwrap();
    store.put(&Key::new("game.sym"), SYM).unwrap();
    store.make_dir(&Key::new(".gbcli")).unwrap();
    let bundle = Bundle::new(store.clone(), Key::new(".gbcli"), clock);

    // 3. The session, from the ROM's key.
    let source = RomSource {
        rom: "game.gb".into(),
        sym: None,
        model: None,
        seed: None,
        flash: None,
    };
    bundle.open_session("play", Some(&source)).unwrap();

    // 4-6. The host, the interrupt, the handler.
    let host = Host::new(&project, HostFiles::Denied).unwrap();
    let interrupt = InterruptFlag::new();
    let mut handler = Handler::new(host, bundle, Box::new(interrupt))
        .with_defaults(Some("play".into()), Some("tool".into()));

    // Run with keys, and read what they did: the block moves a cell a frame
    // while Right is held, up to column 19.
    run_right(&mut handler, 5);
    let x = read_x(&mut handler);
    assert!((1..=5).contains(&x), "wX = {x}");

    // A shot, its PNG read back from the store as a key.
    let shot = ok(handler.handle(&request(Op::Shot(ShotReadArgs {
        shot: ShotArgs {
            path: "shot.png".into(),
            ..ShotArgs::default()
        },
        since: None,
    }))));
    assert!(shot.shot.is_some());
    let png = store
        .get(&Key::new("shot.png"))
        .expect("the shot, as a key");
    assert_eq!(&png[..8], b"\x89PNG\r\n\x1a\n");

    // Checkpoint, move on, restore, undo.
    ok(handler.handle(&request(Op::Checkpoint(CheckpointArgs {
        name: "here".into(),
    }))));
    run_right(&mut handler, 4);
    let moved = read_x(&mut handler);
    assert!(moved > x, "{moved} after {x}");
    ok(handler.handle(&request(Op::Restore(RestoreArgs {
        name: "here".into(),
    }))));
    assert_eq!(read_x(&mut handler), x, "restored");
    ok(handler.handle(&request(Op::Undo)));
    assert_eq!(read_x(&mut handler), moved, "the restore undone");

    // An op that would write host files is refused, not run.
    let new = handler.handle(&request(Op::New(NewArgs { dir: "game".into() })));
    let refused = new.outcome.expect_err("a denied host makes no project");
    assert_eq!(refused.code, ErrorCode::Unsupported);

    // The bundle is in the store too, and nothing reached the disk.
    assert!(
        store
            .get(&Key::new(".gbcli/sessions/play/session.json"))
            .is_ok()
    );
    let left = on_disk(&project);
    let _ = std::fs::remove_dir_all(&project);
    assert!(left.is_empty(), "written to the disk: {left:?}");
}
