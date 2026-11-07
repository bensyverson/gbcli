//! The hash of gbcli's patch set over vendored SameBoy (`patches/`), folded
//! into the core identity by `build.rs`. Shared with `tests/patch_set.rs`
//! through `#[path]`, so the build and the test use one implementation.
//!
//! FNV-1a (64-bit): this detects change, it doesn't resist forgery, so a
//! cryptographic hash would be a build dependency for nothing.

use std::fs;
use std::path::Path;

/// 16 lowercase hex digits over every file in `dir`, in name order: each
/// file's name, its length and its bytes, so an edit, an addition, a removal
/// or a rename all change it.
pub fn hash(dir: &Path) -> String {
    let mut files: Vec<_> = fs::read_dir(dir)
        .unwrap_or_else(|e| panic!("read {}: {e}", dir.display()))
        .map(|entry| entry.expect("read a patches/ entry").path())
        .filter(|path| path.is_file())
        .collect();
    files.sort();
    let mut h = Fnv::new();
    for path in files {
        let name = path.file_name().unwrap().as_encoded_bytes();
        let bytes = fs::read(&path).unwrap_or_else(|e| panic!("read {}: {e}", path.display()));
        h.write(&(name.len() as u64).to_le_bytes());
        h.write(name);
        h.write(&(bytes.len() as u64).to_le_bytes());
        h.write(&bytes);
    }
    format!("{:016x}", h.0)
}

struct Fnv(u64);

impl Fnv {
    fn new() -> Fnv {
        Fnv(0xcbf2_9ce4_8422_2325)
    }

    fn write(&mut self, bytes: &[u8]) {
        for &b in bytes {
            self.0 ^= u64::from(b);
            self.0 = self.0.wrapping_mul(0x0000_0100_0000_01b3);
        }
    }
}
