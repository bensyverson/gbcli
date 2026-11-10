//! The filesystem backing against a scratch directory.

use super::*;

struct Scratch(PathBuf);

impl Scratch {
    fn new(name: &str) -> Scratch {
        let dir = std::env::temp_dir().join(format!("gbcli-files-{}-{name}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        Scratch(dir)
    }

    fn store(&self) -> FileStore {
        FileStore::new(self.0.clone())
    }
}

impl Drop for Scratch {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

#[test]
fn a_key_is_a_path_under_the_root_or_an_absolute_one() {
    let s = Scratch::new("locate");
    let store = s.store();
    assert_eq!(store.locate(&Key::root()), s.0);
    assert_eq!(
        store.locate(&Key::new(".gbcli/sessions")),
        s.0.join(".gbcli").join("sessions")
    );
    assert_eq!(
        store.locate(&Key::new("/elsewhere/game.gb")),
        PathBuf::from("/elsewhere/game.gb")
    );
}

#[test]
fn a_path_inside_the_project_is_a_relative_key_and_one_outside_stays_absolute() {
    let s = Scratch::new("key-of");
    let store = s.store();
    fs::create_dir_all(s.0.join("build")).unwrap();
    fs::write(s.0.join("build").join("game.gb"), b"rom").unwrap();
    assert_eq!(
        store.key_of(&s.0.join("build").join("game.gb")),
        Key::new("build/game.gb")
    );
    assert_eq!(
        store.key_of(Path::new("./build/../build/game.gb")),
        Key::new("build/game.gb")
    );
    // Not there yet: its directory decides.
    assert_eq!(
        store.key_of(&s.0.join("build").join("shot.png")),
        Key::new("build/shot.png")
    );
    let outside = Path::new("/nowhere-gbcli/x.gb");
    assert_eq!(store.key_of(outside), Key::new("/nowhere-gbcli/x.gb"));
}

#[test]
fn a_symlinked_file_keeps_its_own_name_and_resolves_only_canonically() {
    let s = Scratch::new("key-link");
    let store = s.store();
    fs::create_dir_all(s.0.join("build")).unwrap();
    fs::write(s.0.join("build").join("v2.gb"), b"rom").unwrap();
    std::os::unix::fs::symlink("build/v2.gb", s.0.join("game.gb")).unwrap();
    assert_eq!(store.key_of(&s.0.join("game.gb")), Key::new("game.gb"));
    assert_eq!(
        store.canonical_key(&s.0.join("game.gb")),
        Key::new("build/v2.gb")
    );
    // The directory is still resolved: a linked directory into the
    // project gives a key inside it.
    std::os::unix::fs::symlink(&s.0, s.0.join("build").join("up")).unwrap();
    assert_eq!(
        store.key_of(&s.0.join("build").join("up").join("game.gb")),
        Key::new("game.gb")
    );
}

fn backdate(path: &Path) {
    File::options()
        .write(true)
        .open(path)
        .unwrap()
        .set_modified(SystemTime::now() - Duration::from_secs(3600))
        .unwrap();
}

#[test]
fn an_old_untouched_file_has_one_settled_token() {
    let s = Scratch::new("token-same");
    let store = s.store();
    fs::write(s.0.join("a.gb"), b"rom").unwrap();
    backdate(&s.0.join("a.gb"));
    let key = Key::new("a.gb");
    let a = store.change_token(&key).unwrap();
    assert!(store.change_token(&key).unwrap().vouches_for(&a));
}

#[test]
fn a_file_written_just_now_has_an_unsettled_token() {
    let s = Scratch::new("token-racy");
    let store = s.store();
    fs::write(s.0.join("a.gb"), b"rom").unwrap();
    let key = Key::new("a.gb");
    let a = store.change_token(&key).unwrap();
    assert!(!store.change_token(&key).unwrap().vouches_for(&a));
}

#[test]
fn a_rewrite_a_touch_or_a_retargeted_link_changes_the_token() {
    let s = Scratch::new("token-change");
    let store = s.store();
    let key = Key::new("a.gb");
    let settled = |path: &Path| {
        backdate(path);
        store.change_token(&key).unwrap()
    };
    fs::write(s.0.join("a.gb"), b"rom").unwrap();
    let before = settled(&s.0.join("a.gb"));
    fs::write(s.0.join("a.gb"), b"ROM").unwrap();
    // Same length, an hour back again: only the time differs.
    assert!(!settled(&s.0.join("a.gb")).vouches_for(&before));

    let before = settled(&s.0.join("a.gb"));
    File::options()
        .write(true)
        .open(s.0.join("a.gb"))
        .unwrap()
        .set_modified(SystemTime::now() - Duration::from_secs(7200))
        .unwrap();
    assert!(!store.change_token(&key).unwrap().vouches_for(&before));

    // Two files with one length and one time differ by inode.
    fs::write(s.0.join("v2.gb"), b"rom").unwrap();
    fs::write(s.0.join("v3.gb"), b"rom").unwrap();
    let at = SystemTime::now() - Duration::from_secs(3600);
    for name in ["v2.gb", "v3.gb"] {
        File::options()
            .write(true)
            .open(s.0.join(name))
            .unwrap()
            .set_modified(at)
            .unwrap();
    }
    fs::remove_file(s.0.join("a.gb")).unwrap();
    std::os::unix::fs::symlink("v2.gb", s.0.join("a.gb")).unwrap();
    let before = store.change_token(&key).unwrap();
    fs::remove_file(s.0.join("a.gb")).unwrap();
    std::os::unix::fs::symlink("v3.gb", s.0.join("a.gb")).unwrap();
    assert!(!store.change_token(&key).unwrap().vouches_for(&before));
}

#[test]
fn a_missing_blob_has_no_token() {
    let s = Scratch::new("token-missing");
    let err = s.store().change_token(&Key::new("nope")).unwrap_err();
    assert_eq!(err.kind(), io::ErrorKind::NotFound);
}

#[test]
fn a_symlink_is_sized_as_itself_not_its_target() {
    let s = Scratch::new("link-size");
    let store = s.store();
    fs::write(s.0.join("big.bin"), vec![0u8; 4096]).unwrap();
    std::os::unix::fs::symlink(s.0.join("big.bin"), s.0.join("l")).unwrap();
    assert!(store.size(&Key::new("l")).unwrap() < 4096);
}

#[test]
fn put_replaces_a_blob_and_never_creates_its_parent() {
    let s = Scratch::new("put");
    let store = s.store();
    let key = Key::new("a.bin");
    store.put(&key, b"one").unwrap();
    store.put(&key, b"two").unwrap();
    assert_eq!(store.get(&key).unwrap(), b"two");
    assert_eq!(store.size(&key).unwrap(), 3);
    assert_eq!(store.get_range(&key, 1, 8).unwrap(), b"wo");
    let orphan = Key::new("gone/session.json");
    assert!(store.put(&orphan, b"{}").is_err());
    assert!(store.append(&orphan, b"{}").is_err());
    assert_eq!(store.kind(&Key::new("gone")), None);
    let missing = store.get(&Key::new("nope")).unwrap_err();
    assert_eq!(missing.kind(), io::ErrorKind::NotFound);
}

#[test]
fn put_all_writes_nothing_when_one_blob_fails() {
    let s = Scratch::new("put-all");
    let store = s.store();
    store.put(&Key::new("a.sav"), b"old").unwrap();
    let blobs: [(Key, &[u8]); 2] = [
        (Key::new("a.sav"), b"new"),
        (Key::new("missing-dir/a.flash.gb"), b"chip"),
    ];
    let err = store.put_all(&blobs).unwrap_err();
    assert_eq!(err.index, 1);
    assert_eq!(store.get(&Key::new("a.sav")).unwrap(), b"old");
    let names: Vec<String> = store
        .list(&Key::root())
        .unwrap()
        .into_iter()
        .map(|e| e.name)
        .collect();
    assert_eq!(names, vec!["a.sav".to_string()], "a temp file is left");
}

#[test]
fn directories_list_their_children_and_move_and_go_as_a_tree() {
    let s = Scratch::new("tree");
    let store = s.store();
    let dir = Key::new("sessions");
    assert_eq!(
        store.list(&dir).unwrap_err().kind(),
        io::ErrorKind::NotFound
    );
    store.make_dir(&dir).unwrap();
    store.make_dir(&dir).unwrap();
    assert!(store.make_dir(&Key::new("x/y")).is_err(), "a parent made");
    store.make_dir(&dir.join("demo")).unwrap();
    store
        .put(&dir.join("demo").join("session.json"), b"{}")
        .unwrap();
    store.put(&dir.join("note"), b"").unwrap();
    let mut listed = store.list(&dir).unwrap();
    listed.sort_by(|a, b| a.name.cmp(&b.name));
    assert_eq!(
        listed,
        vec![
            Entry {
                name: "demo".into(),
                kind: Kind::Dir
            },
            Entry {
                name: "note".into(),
                kind: Kind::Blob
            },
        ]
    );
    store.make_dirs(&Key::new("removed/deep")).unwrap();
    store
        .rename(&dir.join("demo"), &Key::new("removed/demo-1"))
        .unwrap();
    assert_eq!(store.kind(&dir.join("demo")), None);
    assert_eq!(
        store.kind(&Key::new("removed/demo-1/session.json")),
        Some(Kind::Blob)
    );
    store.remove_tree(&Key::new("removed")).unwrap();
    assert_eq!(store.kind(&Key::new("removed")), None);
    store.remove(&dir.join("note")).unwrap();
    assert!(store.list(&dir).unwrap().is_empty());
}

#[test]
fn a_lock_is_exclusive_records_no_use_until_touched_and_frees_on_drop() {
    let s = Scratch::new("lock");
    let store = s.store();
    let key = Key::new("demo.lock");
    let held = store.lock(&key, Duration::from_millis(1)).unwrap();
    assert_eq!(store.last_used(&key), Some(0), "a new lock is unused");
    let Err(LockError::Busy { .. }) = store.lock(&key, Duration::from_millis(10)) else {
        panic!("a second holder got the lock");
    };
    store.touch(&key, 1_700_000_000).unwrap();
    assert_eq!(store.last_used(&key), Some(1_700_000_000));
    drop(held);
    assert!(store.lock(&key, Duration::from_millis(10)).is_ok());
}

#[test]
fn incarnations_differ_call_to_call() {
    let s = Scratch::new("incarnation");
    let store = s.store();
    let a = store.new_incarnation();
    let b = store.new_incarnation();
    assert_ne!(a, b);
}

#[test]
fn the_system_clock_is_after_2026() {
    assert!(SystemClock.now_secs() > 1_767_225_600);
}
