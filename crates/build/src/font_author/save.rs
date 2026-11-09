//! `font_save`: the working copy back into its `.yaff`. The decisions
//! take data ([`base::drift`], [`merged`]); this file reads and writes
//! around them.

use std::path::{Path, PathBuf};

use gbcli_font::{Font, Glyph, grid};
use gbcli_protocol::{ErrorBody, FontSaveArgs, FontWritten};

use super::base::{self, Base, Drift};
use super::copies::{self, Copies};
use super::io::{self, bad, problems_error, read_text, write_files};
use crate::font::read_font;
use crate::paths::{display, normalize, quote};

/// `font_save`: converts the working copy back into its `.yaff` and
/// removes it, unless the font changed since the copy was made.
pub fn font_save(cwd: &Path, args: &FontSaveArgs) -> Result<FontWritten, Box<ErrorBody>> {
    let yaff = font_named(cwd, &args.file)?;
    let shown = display(&yaff, cwd);
    let copies = copies::copies(&yaff);
    let existing = copies.existing();
    let png = copies.png.is_file() || copies.chars.is_file();
    if existing.is_empty() {
        return Err(bad(
            format!("{shown} has no working copy to save"),
            format!("`gbcli font edit {}` writes one", quote(&shown)),
        ));
    }
    if png && copies.bdf.is_file() {
        return Err(bad(
            format!(
                "{shown} has two working copies, {} and {}",
                display(&copies.png, cwd),
                display(&copies.bdf, cwd)
            ),
            "remove the one whose edits to discard, then save again",
        ));
    }
    let copy = if png { &copies.png } else { &copies.bdf };
    check_base(cwd, &yaff, &copies, copy)?;
    let glyphs = if png {
        png_glyphs(cwd, &copies)?
    } else {
        bdf_glyphs(cwd, &copies)?
    };
    let font = if yaff.is_file() {
        read_font(cwd, &shown)?
    } else {
        Font::default()
    };
    let font = merged(font, glyphs);
    let files = write_files(cwd, vec![io::yaff_out(&yaff, &font)?])?;
    let mut removed = Vec::new();
    let base = copies.base.is_file().then(|| copies.base.clone());
    for p in existing.into_iter().chain(base) {
        std::fs::remove_file(&p).map_err(|e| io::io_failed(&display(&p, cwd), e))?;
        removed.push(display(&p, cwd));
    }
    Ok(FontWritten {
        font: shown,
        glyphs: font.glyphs.len() as u32,
        files,
        removed,
        missing: Vec::new(),
        skipped: Vec::new(),
    })
}

/// The copy's glyphs, with the font's comments, properties and each
/// glyph's comment (by key): the `.yaff` stays the one source of all but
/// the glyphs.
pub(crate) fn merged(mut font: Font, glyphs: Vec<Glyph>) -> Font {
    let glyphs = glyphs
        .into_iter()
        .map(|mut g| {
            if let Some(i) = font.find(&g.key) {
                g.comment = font.glyphs[i].comment.clone();
            }
            g
        })
        .collect();
    font.glyphs = glyphs;
    font
}

/// The `.yaff` `file` names: itself, or the font of the copy it is.
fn font_named(cwd: &Path, file: &str) -> Result<PathBuf, Box<ErrorBody>> {
    let given = normalize(&cwd.join(file));
    let is_yaff = given
        .extension()
        .is_some_and(|e| e.eq_ignore_ascii_case(gbcli_font::EXTENSION));
    if is_yaff {
        return Ok(given);
    }
    copies::font_of(&given).ok_or_else(|| {
        bad(
            format!("{file} is neither a .yaff font nor a working copy of one"),
            format!(
                "pass the font (assets/fonts/NAME.yaff) or its copy in {}",
                copies::COPY_DIR
            ),
        )
    })
}

/// Refuses the save when the font isn't what the copy was made from.
fn check_base(cwd: &Path, yaff: &Path, copies: &Copies, copy: &Path) -> Result<(), Box<ErrorBody>> {
    let shown = display(yaff, cwd);
    let copy_shown = display(copy, cwd);
    let base_shown = display(&copies.base, cwd);
    let recorded = if copies.base.is_file() {
        let text = read_text(&copies.base, &base_shown)?;
        Some(Base::parse(&text).ok_or_else(|| {
            bad(
                format!("{base_shown} isn't a record of the font {copy_shown} was made from"),
                format!("remove {base_shown} to save {copy_shown} over {shown} without the check"),
            )
        })?)
    } else {
        None
    };
    let now = match std::fs::read(yaff) {
        Ok(bytes) => Base::of(Some(&bytes)),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Base::Absent,
        Err(e) => return Err(io::io_failed(&shown, e)),
    };
    let Some(drift) = base::drift(recorded, now) else {
        return Ok(());
    };
    let what = match drift {
        Drift::Changed => "changed",
        Drift::Made => "was made",
        Drift::Removed => "was removed",
    };
    Err(bad(
        format!(
            "{shown} {what} after its working copy {copy_shown} was; saving would replace the \
             font's glyphs with the copy's"
        ),
        format!(
            "to keep both, move {copy_shown} aside, run `gbcli font edit {}` for a fresh copy \
             and redo the edits there; to write the copy's glyphs over the font anyway, remove \
             {base_shown} and save again",
            quote(&shown)
        ),
    ))
}

fn png_glyphs(cwd: &Path, copies: &Copies) -> Result<Vec<Glyph>, Box<ErrorBody>> {
    let png_shown = display(&copies.png, cwd);
    let chars_shown = display(&copies.chars, cwd);
    let bytes = std::fs::read(&copies.png).map_err(|e| {
        bad(
            format!("{png_shown}: {e}"),
            "the PNG and its .chars go together",
        )
    })?;
    let chars = read_text(&copies.chars, &chars_shown)?;
    let image = gbcli_pixels::decode_png(&bytes)
        .map_err(|e| bad(format!("{png_shown}: {}", e.message), e.remedy))?;
    grid::from_grid(&image, &chars).map_err(|p| problems_error(&png_shown, &chars_shown, &p))
}

fn bdf_glyphs(cwd: &Path, copies: &Copies) -> Result<Vec<Glyph>, Box<ErrorBody>> {
    let bdf_shown = display(&copies.bdf, cwd);
    let text = read_text(&copies.bdf, &bdf_shown)?;
    let imported = gbcli_font::bdf::read_bdf(&text, None)
        .map_err(|p| problems_error(&bdf_shown, &bdf_shown, &p))?;
    if !imported.skipped.is_empty() {
        return Err(bad(
            format!(
                "{bdf_shown} has glyphs gbcli can't key: {}",
                imported.skipped.join(", ")
            ),
            "give each an ENCODING (its Unicode code point), or a STARTCHAR name of two or \
             more ASCII letters, digits, _ and - with ENCODING -1",
        ));
    }
    Ok(imported.font.glyphs)
}
