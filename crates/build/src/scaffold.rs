//! `new`: a starter project that builds and runs on the first try. Its
//! layout is the one `build` assumes, so it needs no build file.

use std::fs;
use std::path::{Path, PathBuf};

use gbcli_protocol::{ErrorBody, ErrorCode, NewMade};

use crate::paths::{display, normalize};

/// gbdev's hardware.inc, pinned (`data/hardware.inc/SOURCE`).
pub const HARDWARE_INC: &str = include_str!("../data/hardware.inc/hardware.inc");

/// Why `new` wrote nothing.
#[derive(Debug)]
pub enum NewError {
    /// The directory name can't name a ROM and a session.
    BadName { name: String },
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
];

impl NewError {
    pub fn body(&self) -> ErrorBody {
        match self {
            NewError::BadName { name } => ErrorBody::new(
                ErrorCode::BadRequest,
                format!(
                    "`{name}` can't name a project: it names the ROM and the session, so it \
                     takes only letters, digits, `-`, `_` and `.`"
                ),
                "pick another directory name, e.g. `gbcli new my-game`",
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
    let plain = !name.is_empty()
        && !name.starts_with('.')
        && name
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '-' | '_' | '.'));
    if !plain {
        return Err(NewError::BadName { name });
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
    let shown = display(&dir, &base);
    let (build, rom) = if shown == "." {
        ("gbcli build".to_string(), format!("build/{name}.gb"))
    } else {
        (
            format!("gbcli build {shown}"),
            format!("{shown}/build/{name}.gb"),
        )
    };
    Ok(NewMade {
        next: vec![
            build,
            format!("gbcli run {rom} --session {name} --frames 120 --shot first.png"),
        ],
        name,
        dir: shown,
        files,
    })
}
