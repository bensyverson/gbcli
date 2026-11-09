//! The files around the conversions: reading text, resolving a `--chars`
//! spec, writing every output (folders made as needed), and the errors.

use std::path::{Path, PathBuf};

use gbcli_font::{Font, Key, Problem};
use gbcli_protocol::{
    ErrorBody, ErrorCode, FontFile, FontFileKind, FontProblem, ResultBody, TileAt,
};

use crate::paths::display;

/// One file to write.
pub(super) struct Out {
    path: PathBuf,
    kind: FontFileKind,
    bytes: Vec<u8>,
}

impl Out {
    pub fn new(path: &Path, kind: FontFileKind, bytes: Vec<u8>) -> Out {
        Out {
            path: path.to_path_buf(),
            kind,
            bytes,
        }
    }
}

/// `bad_request` with a message and a remedy.
pub(super) fn bad(message: impl Into<String>, remedy: impl Into<String>) -> Box<ErrorBody> {
    Box::new(ErrorBody::new(ErrorCode::BadRequest, message, remedy))
}

pub(super) fn io_failed(shown: &str, e: std::io::Error) -> Box<ErrorBody> {
    Box::new(ErrorBody::new(
        ErrorCode::IoFailed,
        format!("{shown}: {e}"),
        ErrorCode::IoFailed.default_remedy(),
    ))
}

/// The text of `path` (lossy UTF-8: BDF is ASCII, `.chars` UTF-8).
pub(super) fn read_text(path: &Path, shown: &str) -> Result<String, Box<ErrorBody>> {
    let bytes = std::fs::read(path).map_err(|e| {
        Box::new(ErrorBody::new(
            ErrorCode::FontUnreadable,
            format!("{shown}: {e}"),
            "check the path",
        ))
    })?;
    Ok(String::from_utf8_lossy(&bytes).into_owned())
}

/// The keys a `--chars` spec names: a set's name, else a character list
/// file (text; a named glyph as `<name>`; line breaks ignored).
pub(super) fn resolve_chars(cwd: &Path, spec: &str) -> Result<Vec<Key>, Box<ErrorBody>> {
    if let Some(keys) = gbcli_font::named_set(spec) {
        return Ok(keys);
    }
    let path = cwd.join(spec);
    if !path.is_file() {
        return Err(bad(
            format!("--chars {spec} is neither a set nor a file"),
            format!(
                "name a set ({}) or a file of the characters, a named glyph as <name>",
                gbcli_font::SET_NAMES.join(", ")
            ),
        ));
    }
    let text = read_text(&path, spec)?;
    gbcli_font::read_list(&text).map_err(|p| problems_error(spec, spec, &p))
}

/// `font` as its `.yaff` file at `path`, held to the reader's own rules:
/// a font gbcli writes, gbcli reads back unchanged.
pub(super) fn yaff_out(path: &Path, font: &Font) -> Result<Out, Box<ErrorBody>> {
    let text = gbcli_font::write(font);
    match gbcli_font::read(&text) {
        Ok(back) if &back == font => Ok(Out::new(path, FontFileKind::Yaff, text.into_bytes())),
        other => Err(Box::new(ErrorBody::new(
            ErrorCode::Internal,
            format!(
                "the YAFF writer made a file its reader doesn't take back unchanged ({:?})",
                other.err()
            ),
            ErrorCode::Internal.default_remedy(),
        ))),
    }
}

/// Refuses, before anything is written, an output of `verb` (an export,
/// a guide sheet, a diff's PNG) that would land on a font or in a working
/// copies' folder. Any other file it writes over, as `shot` and `pixels`
/// do: running an export again is the everyday loop.
pub(super) fn refuse_sources(cwd: &Path, verb: &str, outs: &[Out]) -> Result<(), Box<ErrorBody>> {
    for o in outs {
        let shown = display(&o.path, cwd);
        let is_font = o
            .path
            .extension()
            .is_some_and(|e| e.eq_ignore_ascii_case(gbcli_font::EXTENSION));
        if is_font {
            return Err(bad(
                format!("{shown} is a font; {verb} never writes over one"),
                "name a path that isn't a .yaff",
            ));
        }
        if super::copies::in_copy_dir(&o.path) {
            return Err(bad(
                format!(
                    "{shown} is in {}, where working copies live; {verb} never writes there",
                    super::copies::COPY_DIR
                ),
                format!("name a path outside {}", super::copies::COPY_DIR),
            ));
        }
    }
    Ok(())
}

/// Writes every output, making folders as needed, and reports them with
/// paths from `cwd`.
pub(super) fn write_files(cwd: &Path, outs: Vec<Out>) -> Result<Vec<FontFile>, Box<ErrorBody>> {
    let mut files = Vec::new();
    for o in outs {
        let shown = display(&o.path, cwd);
        if let Some(dir) = o.path.parent() {
            std::fs::create_dir_all(dir).map_err(|e| io_failed(&shown, e))?;
        }
        std::fs::write(&o.path, &o.bytes).map_err(|e| io_failed(&shown, e))?;
        files.push(FontFile {
            path: shown,
            kind: o.kind,
            bytes: o.bytes.len() as u64,
        });
    }
    Ok(files)
}

/// `font_unreadable` with every problem: one at a cell is the image's
/// (`image`), one at a line the text file's (`text`).
pub(super) fn problems_error(image: &str, text: &str, problems: &[Problem]) -> Box<ErrorBody> {
    let first = &problems[0];
    let path = |p: &Problem| if p.line.is_some() { text } else { image };
    let more = match problems.len() {
        1 => String::new(),
        n => format!(" (and {} more)", n - 1),
    };
    let mut body = ErrorBody::new(
        ErrorCode::FontUnreadable,
        format!("{}: {first}{more}", path(first)),
        first.remedy.clone(),
    );
    body.result = Some(Box::new(ResultBody {
        font_problems: problems
            .iter()
            .map(|p| FontProblem {
                path: path(p).to_string(),
                line: p.line,
                column: p.column,
                cell: p.cell.map(|(x, y)| TileAt { x, y }),
                glyph: p.glyph.as_ref().map(Key::text),
                message: p.message.clone(),
                remedy: p.remedy.clone(),
            })
            .collect(),
        ..ResultBody::default()
    }));
    Box::new(body)
}
