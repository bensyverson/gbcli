//! The two backings of [`ProjectStore`] answer alike: the same sequence of
//! calls, edge cases included, run against a [`FileStore`] on a real
//! directory and a [`MemoryStore`], step by step.

use std::io;
use std::path::Path;
use std::sync::Arc;
use std::time::Duration;

use crate::{Clock, FileStore, Guard, Key, Kind, LockError, MemoryStore, ProjectStore};

#[derive(Debug)]
struct Fixed;

impl Clock for Fixed {
    fn now_secs(&self) -> u64 {
        100
    }
}

fn outcome<T>(r: io::Result<T>) -> String {
    match r {
        Ok(_) => "Ok".into(),
        Err(e) => format!("Err({:?})", e.kind()),
    }
}

fn bytes(r: io::Result<Vec<u8>>) -> String {
    match r {
        Ok(v) => format!("Ok({v:?})"),
        Err(e) => format!("Err({:?})", e.kind()),
    }
}

fn locked(r: Result<Guard, LockError>) -> (String, Option<Guard>) {
    match r {
        Ok(g) => ("Ok".into(), Some(g)),
        Err(LockError::Busy { .. }) => ("Busy".into(), None),
        Err(LockError::Io(e)) => (format!("Io({:?})", e.kind()), None),
    }
}

/// Each step's name and what the store answered.
fn steps(s: &dyn ProjectStore) -> Vec<(&'static str, String)> {
    let k = Key::new;
    let mut log = Vec::new();
    let mut step = |name, v: String| log.push((name, v));
    step("make_dir d", outcome(s.make_dir(&k("d"))));
    step("put d/a", outcome(s.put(&k("d/a"), b"x")));
    step("make_dir over a blob", outcome(s.make_dir(&k("d/a"))));
    step("kind d/a after", format!("{:?}", s.kind(&k("d/a"))));
    step("touch a directory", outcome(s.touch(&k("d"), 5)));
    step("remove a directory", outcome(s.remove(&k("d"))));
    step("remove_tree a blob", outcome(s.remove_tree(&k("d/a"))));
    step(
        "rename a blob onto a full directory",
        outcome(s.rename(&k("d/a"), &k("d"))),
    );
    step("make_dir e", outcome(s.make_dir(&k("e"))));
    step(
        "rename d onto the empty e",
        outcome(s.rename(&k("d"), &k("e"))),
    );
    step("kind d", format!("{:?}", s.kind(&k("d"))));
    step("kind e/a", format!("{:?}", s.kind(&k("e/a"))));
    step("list a blob", outcome(s.list(&k("e/a"))));
    step("list a missing key", outcome(s.list(&k("zz"))));
    step("get a directory", outcome(s.get(&k("e"))));
    step("get_range a directory", outcome(s.get_range(&k("e"), 0, 1)));
    step("size of a directory", outcome(s.size(&k("e"))));
    step(
        "change_token of a directory",
        outcome(s.change_token(&k("e"))),
    );
    step("append a new e/n", outcome(s.append(&k("e/n"), b"1")));
    step("append again", outcome(s.append(&k("e/n"), b"2")));
    step("get e/n", bytes(s.get(&k("e/n"))));
    step(
        "rename e under itself",
        outcome(s.rename(&k("e"), &k("e/sub"))),
    );
    step(
        "rename a missing key to itself",
        outcome(s.rename(&k("m"), &k("m"))),
    );
    step(
        "rename to a missing parent",
        outcome(s.rename(&k("e/a"), &k("nope/a"))),
    );
    step(
        "rename a blob onto itself",
        outcome(s.rename(&k("e/a"), &k("e/a"))),
    );
    step("put over a directory", outcome(s.put(&k("e"), b"x")));
    step("append over a directory", outcome(s.append(&k("e"), b"x")));
    step(
        "make_dirs through a blob",
        outcome(s.make_dirs(&k("e/a/b"))),
    );
    step("make_dirs onto a blob", outcome(s.make_dirs(&k("e/a"))));
    step("make_dirs fresh", outcome(s.make_dirs(&k("f/g/h"))));
    step("remove_tree f", outcome(s.remove_tree(&k("f"))));
    step("kind f/g", format!("{:?}", s.kind(&k("f/g"))));
    step("put under the removed f", outcome(s.put(&k("f/g/x"), b"x")));
    step(
        "lock a directory",
        locked(s.lock(&k("e"), Duration::ZERO)).0,
    );
    let (first, held) = locked(s.lock(&k("e/l"), Duration::ZERO));
    step("lock e/l", first);
    step(
        "lock e/l again",
        locked(s.lock(&k("e/l"), Duration::ZERO)).0,
    );
    step(
        "last_used of a new lock",
        format!("{:?}", s.last_used(&k("e/l"))),
    );
    drop(held);
    step(
        "lock e/l after the drop",
        locked(s.lock(&k("e/l"), Duration::ZERO)).0,
    );
    step(
        "lock under a missing parent",
        locked(s.lock(&k("x/l"), Duration::ZERO)).0,
    );
    step("remove a missing key", outcome(s.remove(&k("e/zz"))));
    step(
        "remove_tree a missing key",
        outcome(s.remove_tree(&k("e/zz"))),
    );
    step("touch a missing key", outcome(s.touch(&k("e/zz"), 1)));
    step(
        "last_used of the project is known",
        format!("{}", s.last_used(&Key::root()).is_some()),
    );
    step(
        "last_used of a missing key",
        format!("{:?}", s.last_used(&k("e/zz"))),
    );
    step("kind of the project", format!("{:?}", s.kind(&Key::root())));
    step("key_of x/../y", s.key_of(Path::new("x/../y")).to_string());
    step("key_of ./x", s.key_of(Path::new("./x")).to_string());
    step(
        "key_of ../out is outside",
        format!("{}", s.key_of(Path::new("../out")).is_absolute()),
    );
    step("key_of the empty path", s.key_of(Path::new("")).to_string());
    let mut names: Vec<(String, Kind)> = s
        .list(&k("e"))
        .unwrap_or_default()
        .into_iter()
        .map(|e| (e.name, e.kind))
        .collect();
    names.sort_by(|a, b| a.0.cmp(&b.0));
    step("list e", format!("{names:?}"));
    s.put(&k("e/p"), b"p").unwrap();
    s.put(&k("e/q"), b"q").unwrap();
    step(
        "rename a blob over a blob",
        outcome(s.rename(&k("e/p"), &k("e/q"))),
    );
    step("get e/q", bytes(s.get(&k("e/q"))));
    log
}

#[test]
fn the_file_and_memory_backings_answer_every_step_alike() {
    let dir = std::env::temp_dir().join(format!("gbcli-parity-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let file = FileStore::new(std::fs::canonicalize(&dir).unwrap());
    let memory = MemoryStore::new("/parity/project", Arc::new(Fixed)).unwrap();
    let on_file = steps(&file);
    let in_memory = steps(&memory);
    let _ = std::fs::remove_dir_all(&dir);
    let differ: Vec<String> = on_file
        .iter()
        .zip(&in_memory)
        .filter(|(f, m)| f.1 != m.1)
        .map(|((name, f), (_, m))| format!("{name}: file {f}, memory {m}"))
        .collect();
    assert!(differ.is_empty(), "{differ:#?}");
}

/// A new lock is a new entry in its directory, so it dates the directory
/// to now, as a file's creation moves its directory's mtime, though the
/// lock itself records no use.
#[test]
fn a_new_lock_dates_its_directory_to_now() {
    let memory = MemoryStore::new("/parity/project", Arc::new(Fixed)).unwrap();
    memory.make_dir(&Key::new("locks")).unwrap();
    let _held = memory
        .lock(&Key::new("locks/a.lock"), Duration::ZERO)
        .unwrap();
    assert_eq!(memory.last_used(&Key::new("locks")), Some(100));
    assert_eq!(memory.last_used(&Key::new("locks/a.lock")), Some(0));
}
