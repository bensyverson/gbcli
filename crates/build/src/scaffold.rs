//! `new`: a starter project that builds and runs on the first try. Its
//! layout is the one `build` assumes, so it needs no build file.

use std::fs;
use std::path::{Path, PathBuf};

use gbcli_protocol::{ErrorBody, ErrorCode, NewMade, SESSION_NAME_RULE, is_session_name};

use crate::paths::{display, normalize, quote};

/// gbdev's hardware.inc, pinned (`data/hardware.inc/SOURCE`).
pub const HARDWARE_INC: &str = include_str!("../data/hardware.inc/hardware.inc");

/// hUGEDriver, pinned (`data/hUGEDriver/SOURCE`): each file's path in a new
/// project, then its text.
pub const HUGEDRIVER: [(&str, &str); 3] = [
    (
        "include/hUGE.inc",
        include_str!("../data/hUGEDriver/hUGE.inc"),
    ),
    (
        "include/hUGE_note_table.inc",
        include_str!("../data/hUGEDriver/hUGE_note_table.inc"),
    ),
    (
        "src/hUGEDriver.asm",
        include_str!("../data/hUGEDriver/hUGEDriver.asm"),
    ),
];

/// Why `new` wrote nothing.
#[derive(Debug)]
pub enum NewError {
    /// The directory name can't name a session; `offer` is the directory
    /// with a name that can, as the caller reads it.
    BadName { name: String, offer: String },
    /// The directory exists and holds something.
    NotEmpty { dir: PathBuf },
    Io {
        path: PathBuf,
        error: std::io::Error,
    },
}

/// The starter's files: path in the project, then contents, with `{{name}}`
/// standing for the project's name.
const FILES: &[(&str, &str)] = &[
    (".gitignore", include_str!("../templates/gitignore")),
    ("README.md", include_str!("../templates/README.md")),
    ("include/hardware.inc", HARDWARE_INC),
    ("src/main.asm", include_str!("../templates/main.asm")),
    ("src/oam_dma.asm", include_str!("../templates/oam_dma.asm")),
    HUGEDRIVER[0],
    HUGEDRIVER[1],
    HUGEDRIVER[2],
    ("src/music.asm", include_str!("../templates/music.asm")),
    ("assets/music/theme.song", gbcli_music::EXAMPLE),
    ("src/text.asm", include_str!("../templates/text.asm")),
    (
        "assets/fonts/main.yaff",
        include_str!("../templates/main.yaff"),
    ),
];

impl NewError {
    pub fn body(&self) -> ErrorBody {
        match self {
            NewError::BadName { name, offer } => ErrorBody::new(
                ErrorCode::BadRequest,
                format!(
                    "`{name}` can't name a project: it names the ROM and the session, and a \
                     session name is {SESSION_NAME_RULE}"
                ),
                format!(
                    "name the directory like a session: `gbcli new {}`",
                    quote(offer)
                ),
            ),
            NewError::NotEmpty { dir } => ErrorBody::new(
                ErrorCode::BadRequest,
                format!(
                    "{} already holds files; `new` writes only into a new or empty directory",
                    dir.display()
                ),
                "name a directory that doesn't exist yet, e.g. `gbcli new my-game`",
            ),
            NewError::Io { path, error } => {
                ErrorBody::plain(ErrorCode::IoFailed, format!("{}: {error}", path.display()))
            }
        }
    }
}

/// Writes the starter project into `dir`, which must not exist or be empty.
/// Its name, and so its ROM's and its suggested session's, is the
/// directory's.
pub fn new_project(dir: &Path, display_base: &Path) -> Result<NewMade, NewError> {
    let dir = normalize(dir);
    let name = dir
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_default();
    // The name is the ROM's and the session's, and the printed next
    // commands use it as both, so it must be a session name.
    if !is_session_name(&name) {
        let offer = display(&dir.with_file_name(session_name_like(&name)), display_base);
        return Err(NewError::BadName { name, offer });
    }
    let io = |path: &Path| {
        let path = path.to_path_buf();
        move |error| NewError::Io { path, error }
    };
    if dir.exists() {
        let mut entries = fs::read_dir(&dir).map_err(io(&dir))?;
        if entries.next().is_some() {
            return Err(NewError::NotEmpty { dir });
        }
    }
    let base = normalize(display_base);
    let mut files = Vec::new();
    for (rel, text) in FILES {
        let path = dir.join(rel);
        fs::create_dir_all(path.parent().expect("a file has a directory")).map_err(io(&path))?;
        fs::write(&path, text.replace("{{name}}", &name)).map_err(io(&path))?;
        files.push(display(&path, &base));
    }
    // The project's own bundle: sessions run inside it are found here first,
    // never in a `.gbcli/` above it (the starter's .gitignore lists it).
    let bundle = dir.join(".gbcli");
    fs::create_dir_all(&bundle).map_err(io(&bundle))?;
    files.push(format!("{}/", display(&bundle, &base)));
    let shown = display(&dir, &base);
    // Into the project first: the bundle is found from the working
    // directory, so a run from here would put its session in a `.gbcli/`
    // here (or make one), not in the project's, where everything run from
    // inside the project (and the help's loop) looks.
    let mut next = Vec::new();
    if shown != "." {
        next.push(format!("cd {}", quote(&shown)));
    }
    next.push("gbcli build".to_string());
    next.push(format!(
        "gbcli run build/{name}.gb --session {name} --frames 120 --shot first.png"
    ));
    Ok(NewMade {
        next,
        name,
        dir: shown,
        files,
    })
}

/// The session name nearest `name`: lowercase, with each run of other
/// characters as one `-`, cut to 40.
fn session_name_like(name: &str) -> String {
    let mut out = String::new();
    for c in name.chars().map(|c| c.to_ascii_lowercase()) {
        if c.is_ascii_lowercase() || c.is_ascii_digit() {
            out.push(c);
        } else if !out.is_empty() && !out.ends_with('-') {
            out.push('-');
        }
    }
    out.truncate(40);
    let out = out.trim_end_matches('-');
    if out.is_empty() {
        "game".into()
    } else {
        out.into()
    }
}

#[cfg(test)]
mod tests {
    /// An agent put its `.inc` in `src/` and `INCLUDE` could not find it
    /// (F-11): the starter's README says where they go.
    #[test]
    fn the_starter_readme_says_where_a_projects_inc_files_go() {
        let readme = include_str!("../templates/README.md");
        let flat = readme.split_whitespace().collect::<Vec<_>>().join(" ");
        assert!(
            flat.contains(
                "Put your own `.inc` files in `include/`, the only folder `INCLUDE` searches"
            ),
            "{flat}"
        );
    }
}
