//! `pixels` (project/2026-11-08-pixel-text-design.md): an image, a PNG or
//! pixel text, read through `gbcli-pixels`, its palette references resolved
//! through the project's `assets/palettes.txt` (or `--palettes`), and
//! written as canonical text, a PNG or a preview. Everything is made
//! before anything is written, so a failure to make or write an output
//! writes nothing (`write_all`).

use std::path::{Path, PathBuf};

use gbcli_pixels::{Layout, Loaded, Palettes, Picture, PixelsError};
use gbcli_protocol::{
    ErrorBody, ErrorCode, PixelsArgs, PixelsForm, PixelsMade, PixelsOutput, PixelsOutputKind,
    PixelsProblem, ResultBody,
};

use crate::images::palettes;

/// Reads, checks and converts the image `args` names; paths start at `cwd`.
pub fn pixels(cwd: &Path, args: &PixelsArgs) -> Result<PixelsMade, Box<ErrorBody>> {
    if args.tiles && args.text.is_none() {
        return Err(Box::new(ErrorBody::new(
            ErrorCode::BadRequest,
            "--tiles lays out the text, but nothing asks for text",
            "add --text PATH, or drop --tiles",
        )));
    }
    let shown = &args.file;
    let path = cwd.join(shown);
    let bytes = std::fs::read(&path).map_err(|e| {
        Box::new(ErrorBody::new(
            ErrorCode::PixelsUnreadable,
            format!("{shown}: {e}"),
            "check the path; an image file is a PNG or pixel text (gbcli help pixels)",
        ))
    })?;
    let Loaded { form, picture } = gbcli_pixels::read(&bytes).map_err(|e| unreadable(shown, &e))?;
    let raster = if picture.needs_palettes() {
        let palettes = project_palettes(cwd, &path, args.palettes.as_deref())?;
        picture.raster(&palettes)
    } else {
        picture.raster(&Palettes::default())
    }
    .map_err(|mut e| {
        // The verb can also name another palette file.
        e.remedy += "; or name a palette file with --palettes";
        unreadable(shown, &PixelsError::Text(e))
    })?;
    let file_name = path
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_default();
    let frame = gbcli_pixels::frame_size(&file_name);
    let mut files: Vec<(PixelsOutputKind, &String, Vec<u8>)> = Vec::new();
    if let Some(out) = &args.text {
        let layout = if args.tiles {
            Layout::Tiles
        } else {
            Layout::Rows { frame }
        };
        let text = gbcli_pixels::to_text(&picture, layout)
            .map_err(|e| Box::new(ErrorBody::new(ErrorCode::BadRequest, e.message, e.remedy)))?;
        files.push((PixelsOutputKind::Text, out, text.into_bytes()));
    }
    if let Some(out) = &args.png {
        files.push((
            PixelsOutputKind::Png,
            out,
            gbcli_pixels::encode_png(&raster),
        ));
    }
    if let Some(out) = &args.preview {
        files.push((
            PixelsOutputKind::Preview,
            out,
            gbcli_pixels::preview_png(&raster),
        ));
    }
    let wrote = write_all(cwd, files)?;
    Ok(PixelsMade {
        path: shown.clone(),
        form: match form {
            gbcli_pixels::Form::Png => PixelsForm::Png,
            gbcli_pixels::Form::Text => PixelsForm::Text,
        },
        width: picture.width,
        height: picture.height,
        colors: picture.colors(),
        clear: picture.has_clear(),
        frames: frames(&picture, frame),
        tiles: picture.tiles(),
        wrote,
    })
}

/// Writes every output: each goes to a temporary file beside it first, and
/// only when all are written are they renamed into place, so an output
/// that can't be written, or whose path is a directory, leaves the others
/// unwritten. (A rename failing for another reason, midway, can leave the
/// outputs before it in place.) Two outputs of one path write the later.
fn write_all(
    cwd: &Path,
    mut files: Vec<(PixelsOutputKind, &String, Vec<u8>)>,
) -> Result<Vec<PixelsOutput>, Box<ErrorBody>> {
    // The later of two outputs of one path is the one written.
    let mut seen = std::collections::HashSet::new();
    files.reverse();
    files.retain(|(_, out, _)| seen.insert(cwd.join(out)));
    files.reverse();
    if let Some((_, out, _)) = files.iter().find(|(_, out, _)| cwd.join(out).is_dir()) {
        return Err(Box::new(ErrorBody::new(
            ErrorCode::IoFailed,
            format!("writing {out}: it is a directory"),
            "name a file to write, not a directory".to_string(),
        )));
    }
    let failed = |out: &str, e: std::io::Error| {
        Box::new(ErrorBody::new(
            ErrorCode::IoFailed,
            format!("writing {out}: {e}"),
            ErrorCode::IoFailed.default_remedy(),
        ))
    };
    let mut staged: Vec<(PathBuf, PathBuf)> = Vec::new();
    let mut wrote = Vec::new();
    for (kind, out, bytes) in &files {
        let path = cwd.join(out);
        let name = path
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_default();
        let temp = path.with_file_name(format!(".{name}.gbcli-tmp"));
        if let Err(e) = std::fs::write(&temp, bytes) {
            for (t, _) in &staged {
                let _ = std::fs::remove_file(t);
            }
            return Err(failed(out, e));
        }
        staged.push((temp, path));
        wrote.push(PixelsOutput {
            kind: *kind,
            path: (*out).clone(),
            bytes: bytes.len() as u64,
        });
    }
    for (i, (temp, path)) in staged.iter().enumerate() {
        if let Err(e) = std::fs::rename(temp, path) {
            for (t, _) in &staged[i..] {
                let _ = std::fs::remove_file(t);
            }
            return Err(failed(&wrote[i].path, e));
        }
    }
    Ok(wrote)
}

/// A sheet's frames, when its name gives a size it is whole frames of.
fn frames(picture: &Picture, frame: Option<(u32, u32)>) -> Option<u32> {
    let (fw, fh) = frame?;
    let whole =
        fw > 0 && fh > 0 && picture.width.is_multiple_of(fw) && picture.height.is_multiple_of(fh);
    whole.then(|| (picture.width / fw) * (picture.height / fh))
}

/// The palettes `PALETTE N` and `dmg N` resolve through: the file `flag`
/// names, else the `assets/palettes.txt` of the project `image` sits in
/// (the nearest folder above it holding `assets/`), else none.
fn project_palettes(
    cwd: &Path,
    image: &Path,
    flag: Option<&str>,
) -> Result<Palettes, Box<ErrorBody>> {
    let (path, shown): (PathBuf, String) = match flag {
        Some(given) => (cwd.join(given), given.to_string()),
        None => {
            let project = image
                .parent()
                .into_iter()
                .flat_map(Path::ancestors)
                .find(|dir| dir.join("assets").is_dir());
            let Some(project) = project else {
                return Ok(Palettes::default());
            };
            let path = project.join(palettes::PATH);
            if !path.is_file() {
                return Ok(Palettes::default());
            }
            let shown = path
                .strip_prefix(cwd)
                .unwrap_or(&path)
                .to_string_lossy()
                .into_owned();
            (path, shown)
        }
    };
    let text = std::fs::read_to_string(&path).map_err(|e| {
        Box::new(ErrorBody::new(
            ErrorCode::PixelsUnreadable,
            format!("{shown}: {e}"),
            "check the path; --palettes names a palette file like assets/palettes.txt",
        ))
    })?;
    let parsed = palettes::parse(&text).map_err(|bad| {
        let at = bad.line.map_or(String::new(), |l| format!(":{l}"));
        let remedy = palettes::REMEDY;
        problem_body(
            format!("{shown}{at}: {}", bad.message),
            remedy,
            PixelsProblem {
                path: shown.clone(),
                line: bad.line,
                column: None,
                x: None,
                y: None,
                message: bad.message.clone(),
                remedy: remedy.into(),
            },
        )
    })?;
    Ok(palettes::eight_bit(&parsed.named))
}

fn problem_body(message: String, remedy: &str, problem: PixelsProblem) -> Box<ErrorBody> {
    let mut body = ErrorBody::new(ErrorCode::PixelsUnreadable, message, remedy);
    body.result = Some(Box::new(ResultBody {
        pixels_problem: Some(problem),
        ..ResultBody::default()
    }));
    Box::new(body)
}

/// An image that didn't load, as `pixels_unreadable` with the problem
/// located.
fn unreadable(path: &str, e: &PixelsError) -> Box<ErrorBody> {
    let (line, column, pixel, message) = match e {
        PixelsError::Text(t) => (Some(t.line), Some(t.column), None, &t.message),
        PixelsError::Png(p) => (None, None, p.pixel, &p.message),
    };
    let separator = if line.is_some() { ":" } else { ": " };
    problem_body(
        format!("{path}{separator}{e}"),
        e.remedy(),
        PixelsProblem {
            path: path.to_string(),
            line,
            column,
            x: pixel.map(|p| p.0),
            y: pixel.map(|p| p.1),
            message: message.clone(),
            remedy: e.remedy().to_string(),
        },
    )
}
