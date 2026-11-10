use std::io::ErrorKind;
use std::path::Path;
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Duration;

use super::*;
use crate::bundle::Bundle;
use crate::session::RomSource;
use crate::test_fixture::{ROM, SYM};

/// Host time a test sets by hand.
#[derive(Debug, Default)]
struct HandClock(AtomicU64);

impl HandClock {
    fn set(&self, now: u64) {
        self.0.store(now, Ordering::Relaxed);
    }
}

impl Clock for HandClock {
    fn now_secs(&self) -> u64 {
        self.0.load(Ordering::Relaxed)
    }
}

fn store() -> (MemoryStore, Arc<HandClock>) {
    let clock = Arc::new(HandClock::default());
    (MemoryStore::new("/project", clock.clone()).unwrap(), clock)
}

/// Every host's working directory is absolute, so none could sit under a
/// relative project path.
#[test]
fn a_relative_project_path_is_refused_and_an_absolute_one_kept() {
    let clock = Arc::new(HandClock::default());
    for relative in ["project", ".", ""] {
        let refused = MemoryStore::new(relative, clock.clone()).unwrap_err();
        assert_eq!(refused, RelativeRoot(relative.into()));
        assert!(refused.to_string().contains("isn't absolute"), "{refused}");
    }
    let kept = MemoryStore::new("/project/./a/..", clock).unwrap();
    assert_eq!(kept.locate(&Key::root()), Path::new("/project"));
}

fn key(text: &str) -> Key {
    Key::new(text)
}

#[test]
fn a_blob_reads_back_whole_and_in_part() {
    let (s, _) = store();
    s.put(&key("game.gb"), b"abcdef").unwrap();
    assert_eq!(s.get(&key("game.gb")).unwrap(), b"abcdef");
    assert_eq!(s.get_range(&key("game.gb"), 2, 3).unwrap(), b"cde");
    assert_eq!(s.get_range(&key("game.gb"), 4, 100).unwrap(), b"ef");
    assert_eq!(s.get_range(&key("game.gb"), 100, 1).unwrap(), b"");
    assert_eq!(s.size(&key("game.gb")).unwrap(), 6);
    assert_eq!(s.kind(&key("game.gb")), Some(Kind::Blob));
    assert_eq!(
        s.get(&key("other.gb")).unwrap_err().kind(),
        ErrorKind::NotFound
    );
    assert_eq!(s.kind(&key("other.gb")), None);
}

#[test]
fn no_write_creates_its_parent() {
    let (s, _) = store();
    let inside = key("sessions/demo/session.json");
    assert_eq!(
        s.put(&inside, b"{}").unwrap_err().kind(),
        ErrorKind::NotFound
    );
    assert_eq!(
        s.append(&inside, b"{}").unwrap_err().kind(),
        ErrorKind::NotFound
    );
    assert_eq!(
        s.make_dir(&key("sessions/demo")).unwrap_err().kind(),
        ErrorKind::NotFound
    );
    assert_eq!(s.kind(&key("sessions")), None, "nothing was made");
    s.make_dirs(&key("sessions/demo")).unwrap();
    s.put(&inside, b"{}").unwrap();
    assert_eq!(s.get(&inside).unwrap(), b"{}");
}

#[test]
fn directories_are_tracked_and_listed_with_their_blobs() {
    let (s, _) = store();
    s.make_dir(&key(".gbcli")).unwrap();
    s.make_dir(&key(".gbcli")).unwrap();
    s.make_dir(&key(".gbcli/sessions")).unwrap();
    s.put(&key(".gbcli/gaps.jsonl"), b"").unwrap();
    s.put(&key("game.gb"), b"rom").unwrap();
    assert_eq!(s.kind(&key(".gbcli")), Some(Kind::Dir));
    assert_eq!(s.kind(&Key::root()), Some(Kind::Dir));
    let mut listed = s.list(&key(".gbcli")).unwrap();
    listed.sort_by(|a, b| a.name.cmp(&b.name));
    assert_eq!(
        listed,
        vec![
            Entry {
                name: "gaps.jsonl".into(),
                kind: Kind::Blob
            },
            Entry {
                name: "sessions".into(),
                kind: Kind::Dir
            },
        ]
    );
    let mut root: Vec<String> = s
        .list(&Key::root())
        .unwrap()
        .into_iter()
        .map(|e| e.name)
        .collect();
    root.sort();
    assert_eq!(root, vec![".gbcli", "game.gb"]);
    assert!(s.list(&key(".gbcli/sessions")).unwrap().is_empty());
    assert_eq!(
        s.list(&key("missing")).unwrap_err().kind(),
        ErrorKind::NotFound
    );
    assert!(s.list(&key("game.gb")).is_err(), "a blob isn't listed");
    assert!(s.make_dir(&key("game.gb")).is_err(), "a blob has the name");
}

#[test]
fn removing_a_tree_takes_everything_under_it_and_later_writes_there_fail() {
    let (s, _) = store();
    s.make_dirs(&key("s/demo/history")).unwrap();
    s.put(&key("s/demo/session.json"), b"{}").unwrap();
    s.put(&key("s/demo/history/s1.gbck"), b"h").unwrap();
    s.put(&key("s/demo-2"), b"a sibling, not under it").unwrap();
    s.remove_tree(&key("s/demo")).unwrap();
    assert_eq!(s.kind(&key("s/demo")), None);
    assert_eq!(s.kind(&key("s/demo/history/s1.gbck")), None);
    assert_eq!(s.kind(&key("s/demo-2")), Some(Kind::Blob));
    assert_eq!(
        s.put(&key("s/demo/session.json"), b"{}")
            .unwrap_err()
            .kind(),
        ErrorKind::NotFound
    );
    assert_eq!(
        s.remove_tree(&key("s/demo")).unwrap_err().kind(),
        ErrorKind::NotFound
    );
    s.remove(&key("s/demo-2")).unwrap();
    assert_eq!(
        s.remove(&key("s/demo-2")).unwrap_err().kind(),
        ErrorKind::NotFound
    );
}

#[test]
fn renaming_moves_a_directory_with_everything_under_it_and_replaces_a_blob() {
    let (s, _) = store();
    s.make_dirs(&key("sessions/demo/history")).unwrap();
    s.make_dir(&key("removed")).unwrap();
    s.put(&key("sessions/demo/history/s1.gbck"), b"h").unwrap();
    s.rename(&key("sessions/demo"), &key("removed/demo-1"))
        .unwrap();
    assert_eq!(s.kind(&key("sessions/demo")), None);
    assert_eq!(s.kind(&key("removed/demo-1/history")), Some(Kind::Dir));
    assert_eq!(s.get(&key("removed/demo-1/history/s1.gbck")).unwrap(), b"h");
    s.put(&key("a"), b"new").unwrap();
    s.put(&key("b"), b"old").unwrap();
    s.rename(&key("a"), &key("b")).unwrap();
    assert_eq!(s.get(&key("b")).unwrap(), b"new");
    assert_eq!(s.kind(&key("a")), None);
    assert_eq!(
        s.rename(&key("a"), &key("c")).unwrap_err().kind(),
        ErrorKind::NotFound
    );
    assert!(
        s.rename(&key("removed"), &key("removed/inside")).is_err(),
        "a directory can't move under itself"
    );
}

#[test]
fn put_all_writes_every_blob_or_none() {
    let (s, _) = store();
    s.put(&key("a.png"), b"old").unwrap();
    let blobs: Vec<(Key, &[u8])> = vec![(key("a.png"), b"new"), (key("gone/b.png"), b"b")];
    let err = s.put_all(&blobs).unwrap_err();
    assert_eq!(err.index, 1);
    assert_eq!(err.error.kind(), ErrorKind::NotFound);
    assert_eq!(s.get(&key("a.png")).unwrap(), b"old", "nothing was written");
    let blobs: Vec<(Key, &[u8])> = vec![(key("a.png"), b"new"), (key("b.png"), b"b")];
    s.put_all(&blobs).unwrap();
    assert_eq!(s.get(&key("a.png")).unwrap(), b"new");
    assert_eq!(s.get(&key("b.png")).unwrap(), b"b");
}

#[test]
fn appending_creates_the_blob_and_extends_it() {
    let (s, _) = store();
    s.append(&key("events.jsonl"), b"one\n").unwrap();
    s.append(&key("events.jsonl"), b"two\n").unwrap();
    assert_eq!(s.get(&key("events.jsonl")).unwrap(), b"one\ntwo\n");
}

#[test]
fn a_held_lock_answers_busy_at_once_and_drop_releases_it() {
    let (s, _) = store();
    s.make_dir(&key("locks")).unwrap();
    let held = s
        .lock(&key("locks/demo.lock"), Duration::from_secs(60))
        .unwrap();
    let busy = s
        .lock(&key("locks/demo.lock"), Duration::from_secs(60))
        .unwrap_err();
    let LockError::Busy { waited } = busy else {
        panic!("{busy:?}");
    };
    assert_eq!(waited, Duration::ZERO, "no thread waits");
    assert!(
        s.lock(&key("locks/other.lock"), Duration::ZERO).is_ok(),
        "one lock per key"
    );
    drop(held);
    assert!(s.lock(&key("locks/demo.lock"), Duration::ZERO).is_ok());
    assert!(matches!(
        s.lock(&key("nowhere/demo.lock"), Duration::ZERO),
        Err(LockError::Io(e)) if e.kind() == ErrorKind::NotFound
    ));
}

#[test]
fn a_new_lock_records_no_use_and_a_touch_records_one() {
    let (s, clock) = store();
    clock.set(500);
    s.make_dir(&key("locks")).unwrap();
    drop(s.lock(&key("locks/demo.lock"), Duration::ZERO).unwrap());
    assert_eq!(s.last_used(&key("locks/demo.lock")), Some(0));
    s.touch(&key("locks/demo.lock"), 1234).unwrap();
    assert_eq!(s.last_used(&key("locks/demo.lock")), Some(1234));
    assert_eq!(
        s.touch(&key("locks/missing.lock"), 1).unwrap_err().kind(),
        ErrorKind::NotFound
    );
    assert_eq!(s.last_used(&key("locks/missing.lock")), None);
}

/// The fallbacks a session's last use reads when no use is recorded:
/// `session.json`'s last write, then its directory's.
#[test]
fn a_write_records_its_time_on_the_blob_and_its_directory() {
    let (s, clock) = store();
    clock.set(100);
    s.make_dir(&key("demo")).unwrap();
    assert_eq!(s.last_used(&key("demo")), Some(100));
    clock.set(200);
    s.put(&key("demo/session.json"), b"{}").unwrap();
    assert_eq!(s.last_used(&key("demo/session.json")), Some(200));
    assert_eq!(s.last_used(&key("demo")), Some(200));
    clock.set(300);
    s.append(&key("demo/events.jsonl"), b"x").unwrap();
    assert_eq!(s.last_used(&key("demo/events.jsonl")), Some(300));
    assert_eq!(s.last_used(&key("demo/session.json")), Some(200));
}

#[test]
fn a_change_token_is_settled_and_moves_with_every_write() {
    let (s, _) = store();
    s.put(&key("game.gb"), b"v1").unwrap();
    let first = s.change_token(&key("game.gb")).unwrap();
    assert!(first.vouches_for(&s.change_token(&key("game.gb")).unwrap()));
    s.put(&key("other.gb"), b"x").unwrap();
    assert!(
        s.change_token(&key("game.gb")).unwrap().vouches_for(&first),
        "another key's write leaves it"
    );
    s.put(&key("game.gb"), b"v1").unwrap();
    let rewritten = s.change_token(&key("game.gb")).unwrap();
    assert!(!rewritten.vouches_for(&first), "same bytes, still a write");
    s.append(&key("game.gb"), b"!").unwrap();
    assert!(
        !s.change_token(&key("game.gb"))
            .unwrap()
            .vouches_for(&rewritten)
    );
    assert_eq!(
        s.change_token(&key("missing.gb")).unwrap_err().kind(),
        ErrorKind::NotFound
    );
}

#[test]
fn incarnations_never_repeat() {
    let (s, _) = store();
    let a = s.new_incarnation();
    let b = s.new_incarnation();
    assert_ne!(a, b);
}

/// The server resolves a request's paths against the host's working
/// directory before the store sees them, so a path under the store's root
/// must come back as a project key, and any other as a key outside it.
#[test]
fn a_path_under_the_root_is_a_project_key_with_dot_and_dot_dot_resolved() {
    let (s, _) = store();
    assert_eq!(s.key_of(Path::new("/project/game.gb")), key("game.gb"));
    assert_eq!(s.key_of(Path::new("game.gb")), key("game.gb"));
    assert_eq!(s.key_of(Path::new("./a/./b.png")), key("a/b.png"));
    assert_eq!(s.key_of(Path::new("/project/a/../b.png")), key("b.png"));
    assert_eq!(s.key_of(Path::new("/project")), Key::root());
    assert_eq!(
        s.key_of(Path::new("/elsewhere/x.gb")),
        key("/elsewhere/x.gb")
    );
    assert_eq!(s.key_of(Path::new("../x.gb")), key("/x.gb"));
    assert_eq!(s.key_of(Path::new("/projectile/x")), key("/projectile/x"));
    assert_eq!(
        s.canonical_key(Path::new("/project/a/../game.gb")),
        s.key_of(Path::new("game.gb"))
    );
    assert_eq!(s.locate(&key("game.gb")), Path::new("/project/game.gb"));
    assert_eq!(s.locate(&Key::root()), Path::new("/project"));
    assert_eq!(
        s.locate(&key("/elsewhere/x.gb")),
        Path::new("/elsewhere/x.gb")
    );
}

/// `Key::new` normalizes nothing, so `a/../b` and `b` would be two blobs in
/// a map: the store refuses a key that isn't normal.
#[test]
fn a_key_with_dot_dot_or_an_empty_part_is_refused_and_a_normal_one_still_works() {
    let (s, _) = store();
    s.make_dir(&key("a")).unwrap();
    for bad in ["a/../b", "./b", "a//b", "a/", "/x/../y"] {
        assert_eq!(
            s.put(&key(bad), b"x").unwrap_err().kind(),
            ErrorKind::InvalidInput,
            "{bad}"
        );
        assert_eq!(
            s.get(&key(bad)).unwrap_err().kind(),
            ErrorKind::InvalidInput,
            "{bad}"
        );
        assert_eq!(s.kind(&key(bad)), None, "{bad}");
    }
    s.put(&key("a/b"), b"x").unwrap();
    assert_eq!(s.get(&key("a/b")).unwrap(), b"x");
}

/// A key outside the project is a key like any other, under `/`.
#[test]
fn a_key_outside_the_project_is_held_too_once_its_directory_is_made() {
    let (s, _) = store();
    assert_eq!(
        s.put(&key("/roms/x.gb"), b"x").unwrap_err().kind(),
        ErrorKind::NotFound
    );
    s.make_dirs(&key("/roms")).unwrap();
    s.put(&key("/roms/x.gb"), b"x").unwrap();
    assert_eq!(s.get(&key("/roms/x.gb")).unwrap(), b"x");
    let root: Vec<String> = s
        .list(&Key::root())
        .unwrap()
        .into_iter()
        .map(|e| e.name)
        .collect();
    assert!(root.is_empty(), "the project holds nothing: {root:?}");
}

/// A bundle in memory, seeded with the ROM and its `.sym` under project
/// keys: a session is created from the ROM's key, finds the `.sym` beside
/// it, and records both as project keys, whether the ROM is named by its
/// key or by its path under the root.
#[test]
fn a_session_is_created_from_seeded_bytes_and_finds_its_sym() {
    let clock = Arc::new(HandClock::default());
    clock.set(1_000);
    let store = Arc::new(MemoryStore::new("/project", clock.clone()).unwrap());
    store.put(&key("demo.gb"), ROM).unwrap();
    store.put(&key("demo.sym"), SYM.as_bytes()).unwrap();
    store.make_dir(&key(".gbcli")).unwrap();
    let bundle = Bundle::new(store.clone(), key(".gbcli"), clock);
    assert_eq!(bundle.project(), Path::new("/project"));
    for (name, rom) in [("by-key", "demo.gb"), ("by-path", "/project/demo.gb")] {
        let session = bundle
            .open_session(
                name,
                Some(&RomSource {
                    rom: rom.into(),
                    sym: None,
                    model: None,
                    seed: None,
                    flash: None,
                }),
            )
            .unwrap();
        let meta = session.read_meta().unwrap();
        assert_eq!(meta.rom, "demo.gb", "{name}");
        assert_eq!(meta.sym.as_deref(), Some("demo.sym"), "{name}");
        assert!(session.machine(&meta).is_ok(), "{name}");
    }
    assert_eq!(bundle.session_names().unwrap(), vec!["by-key", "by-path"]);
    let rows = bundle.list().unwrap();
    assert_eq!(rows[0].last_used, 1_000);
    bundle
        .session("by-key")
        .unwrap()
        .remove(true, Duration::ZERO)
        .unwrap();
    assert_eq!(bundle.session_names().unwrap(), vec!["by-path"]);
}
