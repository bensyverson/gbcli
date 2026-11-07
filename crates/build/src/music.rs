//! `build`'s music: every song file directly under `assets/music` (a
//! hUGETracker `.uge`, or gbcli's song text as `.song`) is converted to
//! hUGEDriver data, `build/music/<name>.asm` declaring `song_<name>`, and
//! assembled with the sources. The driver itself is the project's
//! (`gbcli new` vendors it), found by its `hUGE.inc` on the include path.

use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

use gbcli_music::{SongError, descriptor_for, global_symbols, read_song, rgbds_asm};
use gbcli_protocol::{ErrorBody, ErrorCode, ResultBody, SongBuilt, SongProblem};

use crate::project::BuildError;

/// Where songs are found, relative to the project.
pub const MUSIC_DIR: &str = "assets/music";
/// Where their `.asm` is written, relative to the project.
pub const MUSIC_OUT: &str = "build/music";
/// The song file extensions `build` converts.
const EXTENSIONS: [&str; 2] = ["song", "uge"];

/// The songs converted, and the `.asm` files to assemble with the sources.
pub(crate) struct Songs {
    pub made: Vec<SongBuilt>,
    pub sources: Vec<PathBuf>,
}

/// Converts the project's songs. `include` is the include path, relative to
/// `project`, which must hold hUGEDriver's `hUGE.inc` when there are songs.
pub(crate) fn convert(
    project: &Path,
    include: &[PathBuf],
    shown: &dyn Fn(&Path) -> String,
) -> Result<Songs, BuildError> {
    let files = song_files(&project.join(MUSIC_DIR)).map_err(|error| BuildError::Io {
        path: project.join(MUSIC_DIR),
        error,
    })?;
    let mut songs = Songs {
        made: Vec::new(),
        sources: Vec::new(),
    };
    if files.is_empty() {
        return Ok(songs);
    }
    if !include
        .iter()
        .any(|d| project.join(d).join("hUGE.inc").is_file())
    {
        return Err(BuildError::Layout {
            message: format!(
                "{} holds songs, but no include directory holds hUGE.inc: the songs play on \
                 hUGEDriver, which the project doesn't have",
                shown(&project.join(MUSIC_DIR))
            ),
            remedy: "copy src/hUGEDriver.asm, src/music.asm and include/hUGE*.inc from a \
                     project `gbcli new DIR` makes, or move the songs out of assets/music"
                .into(),
        });
    }
    // Every global symbol a song's `.asm` defines (its descriptor, its
    // timer constants, its routines) is named after its file, so two files
    // can clash on any of them, not only the descriptor.
    let mut by_symbol: BTreeMap<String, Vec<&PathBuf>> = BTreeMap::new();
    for file in &files {
        for symbol in global_symbols(&descriptor(file)) {
            by_symbol.entry(symbol).or_default().push(file);
        }
    }
    let clash = files.iter().find_map(|file| {
        global_symbols(&descriptor(file))
            .into_iter()
            .find_map(|s| by_symbol.get_key_value(&s).filter(|(_, f)| f.len() > 1))
    });
    if let Some((symbol, clash)) = clash {
        let names: Vec<String> = clash.iter().map(|f| shown(f)).collect();
        return Err(BuildError::Layout {
            message: format!(
                "{} would all define {symbol}: a song's labels come from its file name",
                names.join(" and ")
            ),
            remedy: "rename all but one, or move the others out of assets/music".into(),
        });
    }
    let out_dir = project.join(MUSIC_OUT);
    fs::create_dir_all(&out_dir).map_err(|error| BuildError::Io {
        path: out_dir.clone(),
        error,
    })?;
    for file in files {
        let bytes = fs::read(&file).map_err(|error| BuildError::Io {
            path: file.clone(),
            error,
        })?;
        let loaded = read_song(&bytes).map_err(|error| BuildError::Song {
            path: shown(&file),
            error,
        })?;
        let name = descriptor(&file);
        // `with_extension` would take a dotted stem's last part for the
        // extension (theme.v2 → theme.asm).
        let stem = file.file_stem().expect("a song file has a name");
        let asm = out_dir.join(format!("{}.asm", stem.to_string_lossy()));
        fs::write(&asm, rgbds_asm(&loaded.song, &name)).map_err(|error| BuildError::Io {
            path: asm.clone(),
            error,
        })?;
        let song = &loaded.song;
        songs.made.push(SongBuilt {
            path: shown(&file),
            asm: shown(&asm),
            descriptor: name,
            timer: (song.timer_enabled != 0).then_some((song.timer_divider & 0xFF) as u8),
        });
        songs.sources.push(asm);
    }
    Ok(songs)
}

fn descriptor(file: &Path) -> String {
    descriptor_for(&file.file_stem().unwrap_or_default().to_string_lossy())
}

/// The song files directly in `dir`, sorted; none when there is no `dir`.
fn song_files(dir: &Path) -> std::io::Result<Vec<PathBuf>> {
    if !dir.is_dir() {
        return Ok(Vec::new());
    }
    let mut files = Vec::new();
    for entry in fs::read_dir(dir)? {
        let path = entry?.path();
        let ext = path.extension().and_then(|e| e.to_str()).unwrap_or("");
        if path.is_file() && EXTENSIONS.contains(&ext) {
            files.push(path);
        }
    }
    files.sort();
    Ok(files)
}

/// A song that didn't load, as `song_unreadable` with the problem located:
/// the line and column of song text, the byte of a `.uge`.
pub fn song_problem(path: &str, e: &SongError) -> ErrorBody {
    let (line, column, offset, message, remedy) = match e {
        SongError::Text(t) => (Some(t.line), Some(t.column), None, &t.message, &t.remedy),
        SongError::Uge(u) => (None, None, Some(u.offset as u64), &u.message, &u.remedy),
    };
    let separator = if line.is_some() { ":" } else { ": " };
    let mut body = ErrorBody::new(
        ErrorCode::SongUnreadable,
        format!("{path}{separator}{e}"),
        e.remedy(),
    );
    body.result = Some(Box::new(ResultBody {
        song_problem: Some(SongProblem {
            path: path.to_string(),
            line,
            column,
            offset,
            message: message.clone(),
            remedy: remedy.clone(),
        }),
        ..ResultBody::default()
    }));
    body
}
