//! A temporary directory per test, the process's RGBDS, and fake tools.

#![allow(dead_code)]

use std::ffi::OsString;
use std::fs;
use std::path::{Path, PathBuf};

use gbcli_build::{Toolchain, install_remedy};

/// A directory removed on drop.
pub struct Temp(pub PathBuf);

impl Temp {
    pub fn new(name: &str) -> Temp {
        let dir = std::env::temp_dir().join(format!("gbcli-build-{}-{name}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        Temp(dir)
    }

    pub fn path(&self, rel: &str) -> PathBuf {
        self.0.join(rel)
    }
}

impl Drop for Temp {
    fn drop(&mut self) {
        // A test that made a directory read-only gives it back first.
        let _ = set_mode(&self.0, 0o755);
        let _ = fs::remove_dir_all(&self.0);
    }
}

/// The pinned RGBDS from `PATH`. A machine without it fails here, naming the
/// install command: the build tests never skip silently.
pub fn rgbds() -> Toolchain {
    let path = std::env::var_os("PATH");
    Toolchain::discover(path.as_deref()).unwrap_or_else(|e| {
        panic!(
            "these tests need the pinned RGBDS on PATH: {}\n{}",
            e.body().message,
            install_remedy()
        )
    })
}

/// A directory of fake tools: each name maps to a shell script body run
/// after the `--version` answer (`rgbasm v1.0.4`).
pub fn fake_tools(temp: &Temp, scripts: &[(&str, &str)]) -> OsString {
    let dir = temp.path("fake-bin");
    fs::create_dir_all(&dir).unwrap();
    for (name, body) in scripts {
        let path = dir.join(name);
        fs::write(
            &path,
            format!(
                "#!/bin/sh\nif [ \"$1\" = --version ]; then echo '{name} v1.0.4'; exit 0; fi\n{body}\n"
            ),
        )
        .unwrap();
        set_mode(&path, 0o755).unwrap();
    }
    dir.into_os_string()
}

#[cfg(unix)]
pub fn set_mode(path: &Path, mode: u32) -> std::io::Result<()> {
    use std::os::unix::fs::PermissionsExt;
    fs::set_permissions(path, fs::Permissions::from_mode(mode))
}
