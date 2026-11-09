//! A temporary directory per test, the process's RGBDS, and fake tools.

use std::ffi::OsString;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU32, Ordering};

use gbcli_build::{BuildError, BuildOptions, Toolchain, install_remedy, new_project};
use gbcli_protocol::{BuildMade, Tool};

/// A directory removed on drop.
pub struct Temp(pub PathBuf);

/// Makes each [`Temp`] in this process unique: two tests that pick the same
/// name (a sample's warm and whole comparisons) run in parallel.
static NEXT_TEMP: AtomicU32 = AtomicU32::new(0);

impl Temp {
    pub fn new(name: &str) -> Temp {
        let n = NEXT_TEMP.fetch_add(1, Ordering::Relaxed);
        let dir =
            std::env::temp_dir().join(format!("gbcli-build-{}-{n}-{name}", std::process::id()));
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
    Toolchain::discover(path.as_deref(), &std::env::current_dir().unwrap()).unwrap_or_else(|e| {
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

/// The body of a fake tool that runs the real `tool` with `prefix` (an
/// environment assignment, or nothing) in front of it.
pub fn real(tool: Tool, prefix: &str) -> String {
    let path = rgbds().path(tool).display().to_string();
    format!("{prefix} exec '{path}' \"$@\"")
}

/// A fresh starter project at `<temp>/demo`, printed relative to `<temp>`.
pub fn starter(name: &str) -> (Temp, BuildOptions) {
    let temp = Temp::new(name);
    new_project(&temp.path("demo"), &temp.0).unwrap();
    let options = BuildOptions::new(temp.path("demo"), temp.0.clone());
    (temp, options)
}

/// Takes the music out of a starter at `project`: the song, the player,
/// hUGEDriver and main.asm's calls into the player.
pub fn without_music(project: &Path) {
    fs::remove_dir_all(project.join("assets/music")).unwrap();
    for file in ["src/music.asm", "src/hUGEDriver.asm"] {
        fs::remove_file(project.join(file)).unwrap();
    }
    let main = project.join("src/main.asm");
    let text: String = fs::read_to_string(&main)
        .unwrap()
        .lines()
        .filter(|l| !l.contains("call Music"))
        .map(|l| format!("{l}\n"))
        .collect();
    fs::write(&main, text).unwrap();
}

/// Takes the text out of a starter at `project`: its font, text.asm and
/// main.asm's line of text, for a project with no font (or a test's own
/// `main.yaff`, whose glyphs the starter's string would miss).
pub fn without_text(project: &Path) {
    fs::remove_dir_all(project.join("assets/fonts")).unwrap();
    fs::remove_file(project.join("src/text.asm")).unwrap();
    let main = project.join("src/main.asm");
    let mut text = fs::read_to_string(&main).unwrap();
    for (from, to) in [
        ("include \"images.inc\"", "MAIN_FONT_CHARMAP text, 0\n"),
        ("\t; A line of text below the faces", "\tcall TextDraw\n"),
        ("; The line, in the charmap", "\tPOPC\n"),
    ] {
        let start = text
            .find(from)
            .unwrap_or_else(|| panic!("{from:?} in main.asm"));
        let end = start + text[start..].find(to).unwrap() + to.len();
        text.replace_range(start..end, "");
    }
    assert!(!text.contains("Text") && !text.contains("FONT"), "{text}");
    fs::write(&main, text).unwrap();
}

pub fn failed(result: Result<BuildMade, BuildError>) -> BuildMade {
    match result {
        Err(BuildError::Failed { made, .. }) => *made,
        other => panic!("expected a failed build, got {other:?}"),
    }
}

#[cfg(unix)]
pub fn set_mode(path: &Path, mode: u32) -> std::io::Result<()> {
    use std::os::unix::fs::PermissionsExt;
    fs::set_permissions(path, fs::Permissions::from_mode(mode))
}
