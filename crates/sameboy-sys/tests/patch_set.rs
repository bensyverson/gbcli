//! The patch-set hash folded into the core identity: it changes whenever any
//! file in `patches/` does, and the build reports the hash of the real directory.

#[path = "../patch_set.rs"]
mod patch_set;

use std::fs;
use std::path::{Path, PathBuf};

/// A scratch directory, removed on drop.
struct Dir(PathBuf);

impl Dir {
    fn new(name: &str) -> Dir {
        let path =
            std::env::temp_dir().join(format!("gbcli-patch-set-{}-{name}", std::process::id()));
        let _ = fs::remove_dir_all(&path);
        fs::create_dir_all(&path).unwrap();
        Dir(path)
    }

    fn write(&self, name: &str, text: &str) {
        fs::write(self.0.join(name), text).unwrap();
    }

    fn hash(&self) -> String {
        patch_set::hash(&self.0)
    }
}

impl Drop for Dir {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

fn two_patches(name: &str) -> Dir {
    let dir = Dir::new(name);
    dir.write("a.patch", "first\n");
    dir.write("b.patch", "second\n");
    dir
}

#[test]
fn the_same_files_give_the_same_hash_whatever_the_write_order() {
    let forward = two_patches("forward");
    let backward = Dir::new("backward");
    backward.write("b.patch", "second\n");
    backward.write("a.patch", "first\n");
    assert_eq!(forward.hash(), backward.hash());
    assert_eq!(forward.hash().len(), 16);
}

#[test]
fn editing_a_patch_changes_the_hash() {
    let dir = two_patches("edit");
    let before = dir.hash();
    dir.write("b.patch", "second, rebased\n");
    assert_ne!(dir.hash(), before);
}

#[test]
fn adding_a_file_changes_the_hash() {
    let dir = two_patches("add");
    let before = dir.hash();
    dir.write("c.patch", "");
    assert_ne!(dir.hash(), before);
}

#[test]
fn renaming_a_patch_changes_the_hash() {
    let dir = two_patches("rename");
    let before = dir.hash();
    fs::rename(dir.0.join("b.patch"), dir.0.join("z.patch")).unwrap();
    assert_ne!(dir.hash(), before);
}

#[test]
fn moving_bytes_between_files_changes_the_hash() {
    let a = Dir::new("split-a");
    a.write("a.patch", "ab");
    a.write("b.patch", "c");
    let b = Dir::new("split-b");
    b.write("a.patch", "a");
    b.write("b.patch", "bc");
    assert_ne!(a.hash(), b.hash());
}

#[test]
fn the_build_reports_the_hash_of_the_vendored_patches() {
    let patches = Path::new(env!("CARGO_MANIFEST_DIR")).join("patches");
    assert_eq!(sameboy_sys::PATCH_SET, patch_set::hash(&patches));
    assert_eq!(
        sameboy_sys::CORE_VERSION,
        format!(
            "{}+patches.{}",
            sameboy_sys::SAMEBOY_COMMIT,
            sameboy_sys::PATCH_SET
        )
    );
}
