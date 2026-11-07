//! The patch-set hash folded into the core identity: it changes whenever any
//! file in `patches/` does, and the build reports the hash of the real directory.
//! The identity hashes `patches/`, not the compiled tree, so the vendored
//! `Core/` must be upstream's plus exactly those patches: an edit made in
//! `Core/` alone would change the layout without changing the identity.

#[path = "../patch_set.rs"]
mod patch_set;

use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

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

/// Copies `from` into `to`, recursively.
fn copy_tree(from: &Path, to: &Path) {
    fs::create_dir_all(to).unwrap();
    for entry in fs::read_dir(from).unwrap() {
        let path = entry.unwrap().path();
        let dest = to.join(path.file_name().unwrap());
        if path.is_dir() {
            copy_tree(&path, &dest);
        } else {
            fs::copy(&path, &dest).unwrap();
        }
    }
}

fn run(command: &mut Command) -> String {
    let out = command
        .output()
        .unwrap_or_else(|e| panic!("{command:?}: {e}"));
    assert!(
        out.status.success(),
        "{command:?}: {}{}",
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr)
    );
    String::from_utf8(out.stdout).unwrap()
}

#[test]
fn the_vendored_core_is_upstream_plus_exactly_the_patch_set() {
    let sys = Path::new(env!("CARGO_MANIFEST_DIR"));
    let script = sys.join("../../scripts/sameboy-core-hash.sh");
    let recorded = fs::read_to_string(sys.join("SameBoy/CORE_SHA256"))
        .expect("SameBoy/CORE_SHA256, written by scripts/vendor-sameboy.sh");
    let core = Dir::new("core");
    copy_tree(&sys.join("SameBoy/Core"), &core.0);
    let mut patches: Vec<PathBuf> = fs::read_dir(sys.join("patches"))
        .unwrap()
        .map(|e| e.unwrap().path())
        .filter(|p| p.extension().is_some_and(|x| x == "patch"))
        .collect();
    patches.sort();
    for patch in patches.iter().rev() {
        // a/crates/sameboy-sys/SameBoy/Core/<file>: five components.
        run(Command::new("patch")
            .args(["-R", "-s", "-p5", "-d"])
            .arg(&core.0)
            .arg("-i")
            .arg(patch));
    }
    let upstream = run(Command::new(&script).arg(&core.0));
    assert_eq!(
        upstream.trim(),
        recorded.trim(),
        "SameBoy/Core with every patch undone isn't upstream's Core: an edit in \
         Core/ needs a patch in patches/ (which changes the core identity), \
         or a re-vendor (project/2026-11-06-sameboy-vendoring.md)"
    );
}
