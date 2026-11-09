//! Font authoring (project/2026-11-09-pixel-fonts-design.md, Ben's
//! decisions): `font_edit`, `font_save`, `font_import`, `font_export`,
//! `font_new` and `font_diff`. The conversions are `gbcli-font`'s, data in
//! and data out; this module reads and writes the files around them and
//! keeps the working copy (`copies`). The `.yaff` is the one source: a
//! save takes the glyphs from the copy and the font's comments and
//! properties (and each glyph's comment, by key) from the `.yaff`.

mod copies;
mod io;

use std::path::{Path, PathBuf};

use gbcli_font::{Font, Key, grid};
use gbcli_protocol::{
    ErrorBody, FontDiffArgs, FontDiffMade, FontEditArgs, FontExportArgs, FontFileKind, FontForm,
    FontImportArgs, FontNewArgs, FontPreview, FontSaveArgs, FontWritten, GlyphChange,
    GlyphChangeKind,
};

pub(crate) use copies::newer_copy;
use io::{Out, bad, problems_error, read_text, resolve_chars, write_files};

use crate::font::read_font;
use crate::paths::{display, normalize, quote};

/// `font_edit`: writes a working copy of the font and says where.
pub fn font_edit(cwd: &Path, args: &FontEditArgs) -> Result<FontWritten, Box<ErrorBody>> {
    let font = read_font(cwd, &args.file)?;
    let yaff = normalize(&cwd.join(&args.file));
    let copies = copies::copies(&yaff);
    refuse_existing(cwd, &copies, &args.file)?;
    let outs = match args.form {
        FontForm::Png => {
            let g = grid::to_grid(&font, true);
            vec![
                Out::new(
                    &copies.png,
                    FontFileKind::Png,
                    gbcli_pixels::encode_png(&g.image),
                ),
                Out::new(&copies.chars, FontFileKind::Chars, g.chars.into_bytes()),
            ]
        }
        FontForm::Bdf => vec![Out::new(
            &copies.bdf,
            FontFileKind::Bdf,
            gbcli_font::bdf::write_bdf(&font, &stem(&yaff)).into_bytes(),
        )],
    };
    Ok(FontWritten {
        font: args.file.clone(),
        glyphs: font.glyphs.len() as u32,
        files: write_files(cwd, outs)?,
        removed: Vec::new(),
        missing: Vec::new(),
        skipped: Vec::new(),
    })
}

/// `font_save`: converts the working copy back into its `.yaff` and
/// removes it.
pub fn font_save(cwd: &Path, args: &FontSaveArgs) -> Result<FontWritten, Box<ErrorBody>> {
    let given = normalize(&cwd.join(&args.file));
    let is_yaff = given
        .extension()
        .is_some_and(|e| e.eq_ignore_ascii_case(gbcli_font::EXTENSION));
    let yaff = if is_yaff {
        given.clone()
    } else {
        copies::font_of(&given).ok_or_else(|| {
            bad(
                format!(
                    "{} is neither a .yaff font nor a working copy of one",
                    args.file
                ),
                format!(
                    "pass the font (assets/fonts/NAME.yaff) or its copy in {}",
                    copies::COPY_DIR
                ),
            )
        })?
    };
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
    let glyphs = if png {
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
        grid::from_grid(&image, &chars).map_err(|p| problems_error(&png_shown, &chars_shown, &p))?
    } else {
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
        imported.font.glyphs
    };
    let mut font = if yaff.is_file() {
        read_font(cwd, &shown)?
    } else {
        Font::default()
    };
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
    let files = write_files(cwd, vec![io::yaff_out(&yaff, &font)?])?;
    let mut removed = Vec::new();
    for p in existing {
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

/// `font_import`: a BDF font as a `.yaff`, never over an existing file.
pub fn font_import(cwd: &Path, args: &FontImportArgs) -> Result<FontWritten, Box<ErrorBody>> {
    let path = cwd.join(&args.file);
    let text = read_text(&path, &args.file)?;
    let only = match &args.chars {
        Some(spec) => Some(resolve_chars(cwd, spec)?),
        None => None,
    };
    let imported = gbcli_font::bdf::read_bdf(&text, only.as_deref())
        .map_err(|p| problems_error(&args.file, &args.file, &p))?;
    let out = match &args.out {
        Some(o) => cwd.join(o),
        None => path.with_extension(gbcli_font::EXTENSION),
    };
    let shown = display(&out, cwd);
    if out.exists() {
        return Err(bad(
            format!("{shown} exists; an import never writes over a font"),
            "remove it first, or name another file with --out",
        ));
    }
    let files = write_files(cwd, vec![io::yaff_out(&out, &imported.font)?])?;
    Ok(FontWritten {
        font: shown,
        glyphs: imported.font.glyphs.len() as u32,
        files,
        removed: Vec::new(),
        missing: imported.missing.iter().map(Key::text).collect(),
        skipped: imported.skipped,
    })
}

/// `font_export`: the font as a PNG grid (with its `.chars`) or BDF,
/// where asked.
pub fn font_export(cwd: &Path, args: &FontExportArgs) -> Result<FontWritten, Box<ErrorBody>> {
    if args.png.is_none() && args.bdf.is_none() {
        return Err(bad(
            "export needs somewhere to write",
            "add --png PATH (with its .chars beside it) or --bdf PATH",
        ));
    }
    if args.guides && args.png.is_none() {
        return Err(bad(
            "--guides draws into the PNG, but nothing asks for one",
            "add --png PATH, or drop --guides",
        ));
    }
    let font = read_font(cwd, &args.file)?;
    let yaff = cwd.join(&args.file);
    let mut outs = Vec::new();
    if let Some(png) = &args.png {
        let g = grid::to_grid(&font, args.guides);
        let png = cwd.join(png);
        let chars = png.with_extension(gbcli_font::CHARS_EXTENSION);
        outs.push(Out::new(
            &png,
            FontFileKind::Png,
            gbcli_pixels::encode_png(&g.image),
        ));
        outs.push(Out::new(&chars, FontFileKind::Chars, g.chars.into_bytes()));
    }
    if let Some(bdf) = &args.bdf {
        let text = gbcli_font::bdf::write_bdf(&font, &stem(&yaff));
        outs.push(Out::new(
            &cwd.join(bdf),
            FontFileKind::Bdf,
            text.into_bytes(),
        ));
    }
    Ok(FontWritten {
        font: args.file.clone(),
        glyphs: font.glyphs.len() as u32,
        files: write_files(cwd, outs)?,
        removed: Vec::new(),
        missing: Vec::new(),
        skipped: Vec::new(),
    })
}

/// `font_new`: an empty, guided working copy for a font not yet made.
pub fn font_new(cwd: &Path, args: &FontNewArgs) -> Result<FontWritten, Box<ErrorBody>> {
    let yaff = new_path(cwd, &args.name)?;
    let shown = display(&yaff, cwd);
    if yaff.exists() {
        return Err(bad(
            format!("{shown} exists"),
            format!("`gbcli font edit {}` edits it", quote(&shown)),
        ));
    }
    let copies = copies::copies(&yaff);
    refuse_existing(cwd, &copies, &shown)?;
    let keys = resolve_chars(cwd, &args.chars)?;
    let g = grid::new_grid(&keys);
    let mut outs = vec![
        Out::new(
            &copies.png,
            FontFileKind::Png,
            gbcli_pixels::encode_png(&g.image),
        ),
        Out::new(
            &copies.chars,
            FontFileKind::Chars,
            g.chars.clone().into_bytes(),
        ),
    ];
    if let Some(sheet) = &args.guide_sheet {
        let png = grid::guide_sheet(&g.image, &g.chars);
        outs.push(Out::new(&cwd.join(sheet), FontFileKind::GuideSheet, png));
    }
    Ok(FontWritten {
        font: shown,
        glyphs: 0,
        files: write_files(cwd, outs)?,
        removed: Vec::new(),
        missing: Vec::new(),
        skipped: Vec::new(),
    })
}

/// `font_diff`: what differs from font A to font B.
pub fn font_diff(cwd: &Path, args: &FontDiffArgs) -> Result<FontDiffMade, Box<ErrorBody>> {
    let a = read_font(cwd, &args.a)?;
    let b = read_font(cwd, &args.b)?;
    let d = gbcli_font::diff::diff(&a, &b);
    let png = match &args.png {
        Some(out) => {
            let bytes = gbcli_font::diff::diff_png(&d);
            let len = bytes.len() as u64;
            write_files(
                cwd,
                vec![Out::new(&cwd.join(out), FontFileKind::Png, bytes)],
            )?;
            Some(FontPreview {
                path: out.clone(),
                bytes: len,
            })
        }
        None => None,
    };
    let glyphs = d
        .changes
        .iter()
        .map(|c| GlyphChange {
            key: c.key.text(),
            change: match (&c.before, &c.after) {
                (Some(_), Some(_)) => GlyphChangeKind::Changed,
                (None, _) => GlyphChangeKind::Added,
                (_, None) => GlyphChangeKind::Removed,
            },
            width_a: c.before.as_ref().map(|g| g.width),
            width_b: c.after.as_ref().map(|g| g.width),
        })
        .collect();
    Ok(FontDiffMade {
        a: args.a.clone(),
        b: args.b.clone(),
        glyphs,
        reordered: d.reordered,
        png,
    })
}

/// Refuses a second working copy of one font.
fn refuse_existing(cwd: &Path, copies: &copies::Copies, font: &str) -> Result<(), Box<ErrorBody>> {
    let existing = copies.existing();
    let Some(first) = existing.first() else {
        return Ok(());
    };
    Err(bad(
        format!("{font} already has a working copy, {}", display(first, cwd)),
        format!(
            "`gbcli font save {}` keeps its edits; removing it discards them",
            quote(font)
        ),
    ))
}

/// The `.yaff` `font new NAME` makes: NAME itself when it ends in
/// `.yaff`, else `assets/fonts/NAME.yaff`.
fn new_path(cwd: &Path, name: &str) -> Result<PathBuf, Box<ErrorBody>> {
    if name.ends_with(&format!(".{}", gbcli_font::EXTENSION)) {
        return Ok(normalize(&cwd.join(name)));
    }
    let ok = name.starts_with(|c: char| c.is_ascii_alphabetic())
        && name
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-');
    if !ok {
        return Err(bad(
            format!(
                "`{name}` isn't a font name: it becomes the font's labels, so it starts with a \
                 letter and holds only letters, digits, _ and -"
            ),
            "name it like talk or hud_small, or pass a path ending in .yaff",
        ));
    }
    Ok(cwd
        .join("assets")
        .join(crate::images::fonts::DIR)
        .join(name)
        .with_extension(gbcli_font::EXTENSION))
}

/// A font file's stem, its BDF `FONT` name.
fn stem(yaff: &Path) -> String {
    yaff.file_stem()
        .map(|s| s.to_string_lossy().into_owned())
        .unwrap_or_default()
}
