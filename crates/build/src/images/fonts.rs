//! Fonts by convention (project/2026-11-09-pixel-fonts-design.md, Ben's
//! decisions): every `assets/fonts/NAME.yaff` is a font, read by
//! `gbcli-font`, named after its file as an image is (`main.yaff` is
//! `MainFontGlyphs`, `MAIN_FONT_GLYPHS`). A build emits every glyph of a
//! font of at most 254, in file order; past that it needs a character
//! list, `assets/fonts/NAME.charlist`: text holding the characters the
//! program's strings use (a named glyph as `<name>`), which limits what it
//! emits. `assets` returns the glyphs a request names, with no cap.
//!
//! A requested letter the font doesn't draw (named by a request or a
//! character list, never otherwise) is composed from its base and mark
//! when the font draws both (`gbcli_font::compose`); composed glyphs
//! follow the drawn ones, in the order first asked, and are reported as
//! `GlyphOrigin::Composed`, an overlap of mark and base as a warning in
//! `ImagesMade.font_overlaps`. A build refuses while a font's working copy
//! (`gbcli font edit`) is newer than it.

use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

use gbcli_font::{Composed, Font, Key};
use gbcli_protocol::{FontAsset, FontGlyph, FontOverlap, GlyphOrigin, ImageLimit, ImageProblem};

use super::census;
use super::find;
use crate::paths::quote;
use crate::project::BuildError;

/// The fonts' directory under `assets/`.
pub(crate) const DIR: &str = "fonts";
/// A build's glyphs a font: `$FE` and `$FF` are the charmap's newline and
/// a string's end.
pub(crate) const MAX_GLYPHS: usize = 254;
/// A character list's extension, beside its font.
pub(crate) const CHARLIST: &str = "charlist";

/// One font to read.
#[derive(Clone, Debug)]
pub(crate) struct FontSource {
    /// The `.yaff` file.
    pub path: PathBuf,
    /// Its path under `assets/` (`fonts/main.yaff`), as a request names it.
    pub name: String,
    /// `Main`: the labels' stem.
    pub label: String,
    /// `MAIN`: the constants' stem.
    pub constant: String,
    /// Where its character list is, or would be.
    pub charlist: PathBuf,
}

/// Every `assets/fonts/*.yaff` of `project`, in name order. A character
/// list without its font, or a name the assembler can't use, is a layout
/// error.
pub(crate) fn find(
    project: &Path,
    shown: &dyn Fn(&Path) -> String,
) -> Result<Vec<FontSource>, BuildError> {
    let dir = project.join("assets").join(DIR);
    let Ok(entries) = fs::read_dir(&dir) else {
        return Ok(Vec::new());
    };
    let mut fonts = Vec::new();
    let mut lists = Vec::new();
    for entry in entries {
        let path = entry
            .map_err(|error| BuildError::Io {
                path: dir.clone(),
                error,
            })?
            .path();
        let ext = path
            .extension()
            .map(|e| e.to_string_lossy().to_ascii_lowercase());
        match ext.as_deref() {
            Some(gbcli_font::EXTENSION) if path.is_file() => fonts.push(path),
            Some(CHARLIST) if path.is_file() => lists.push(path),
            _ => {}
        }
    }
    fonts.sort();
    let found: Vec<FontSource> = fonts
        .into_iter()
        .map(|path| {
            let file = path
                .file_name()
                .map(|n| n.to_string_lossy().into_owned())
                .unwrap_or_default();
            source(path.clone(), format!("{DIR}/{file}"), shown)
        })
        .collect::<Result<_, _>>()?;
    for f in &found {
        if let Some(copy) = crate::font_author::newer_copy(&f.path) {
            let font = shown(&f.path);
            return Err(BuildError::Layout {
                message: format!(
                    "{} is a working copy of {font}, newer than it: its edits aren't in the font",
                    shown(&copy)
                ),
                remedy: format!(
                    "`gbcli font save {}` saves them into the font (removing the copy discards \
                     them)",
                    quote(&font)
                ),
            });
        }
    }
    for list in lists {
        if !found.iter().any(|f| f.charlist == list) {
            return Err(BuildError::Layout {
                message: format!(
                    "{} is a character list, but there is no font {} beside it",
                    shown(&list),
                    shown(&list.with_extension(gbcli_font::EXTENSION))
                ),
                remedy: format!("rename it to match its font, or remove {}", shown(&list)),
            });
        }
    }
    Ok(found)
}

/// The font in `path`, named `name` under `assets/`.
pub(crate) fn source(
    path: PathBuf,
    name: String,
    shown: &dyn Fn(&Path) -> String,
) -> Result<FontSource, BuildError> {
    let stem = Path::new(&name)
        .file_stem()
        .map(|s| s.to_string_lossy().into_owned())
        .unwrap_or_default();
    if !find::is_name(&stem) {
        return Err(BuildError::Layout {
            message: format!(
                "{}: a font's name becomes its labels, so it starts with a letter and holds only \
                 letters, digits, `_` and `-`",
                shown(&path)
            ),
            remedy: format!(
                "rename it, e.g. to {}",
                quote(&format!(
                    "{}.{}",
                    find::name_like(&stem),
                    gbcli_font::EXTENSION
                ))
            ),
        });
    }
    let (label, constant) = find::stems(&stem);
    let charlist = path.with_extension(CHARLIST);
    Ok(FontSource {
        path,
        name,
        label,
        constant,
        charlist,
    })
}

/// Which glyphs a conversion takes.
pub(crate) enum Select<'a> {
    /// A build's: every glyph, at most 254, or its character list's.
    Build,
    /// An `assets` request's: by font name, the keys asked for; every
    /// glyph of a font it doesn't name. No cap.
    Request(&'a BTreeMap<String, Vec<String>>),
}

/// A font as a build emits it.
pub(crate) struct Emitted {
    pub source: String,
    pub label: String,
    pub constant: String,
    pub fixed: bool,
    pub glyphs: Vec<gbcli_font::Glyph>,
}

/// What the fonts became.
#[derive(Default)]
pub(crate) struct Fonts {
    pub assets: Vec<FontAsset>,
    pub emitted: Vec<Emitted>,
    pub problems: Vec<ImageProblem>,
    pub overlaps: Vec<FontOverlap>,
}

/// The glyphs taken from a font: drawn ones by number, then composed.
type Picked = (Vec<usize>, Vec<Composed>);

const FORMAT: &str = "`gbcli help fonts` has the format; `gbcli font FILE` checks a font";

/// Reads `fonts` and takes the glyphs `select` asks for.
pub(crate) fn convert(
    fonts: &[FontSource],
    select: &Select,
    shown: &dyn Fn(&Path) -> String,
) -> Result<Fonts, BuildError> {
    if let Select::Request(asked) = select
        && let Some(name) = asked.keys().find(|n| !fonts.iter().any(|f| &&f.name == n))
    {
        let have: Vec<&str> = fonts.iter().map(|f| f.name.as_str()).collect();
        return Err(BuildError::Layout {
            message: format!(
                "the request asks for glyphs from {name}, which isn't among the fonts ({})",
                if have.is_empty() {
                    "there are none".to_string()
                } else {
                    have.join(", ")
                }
            ),
            remedy: "name a font by its path under assets/ (fonts/NAME.yaff)".into(),
        });
    }
    let mut out = Fonts::default();
    for s in fonts {
        let image = shown(&s.path);
        let text = fs::read(&s.path).map_err(|error| BuildError::Io {
            path: s.path.clone(),
            error,
        })?;
        let font = match std::str::from_utf8(&text)
            .map_err(|e| {
                let line = text[..e.valid_up_to()]
                    .iter()
                    .filter(|b| **b == b'\n')
                    .count();
                gbcli_font::FontError {
                    line: line as u32 + 1,
                    column: 1,
                    message: "this line is not UTF-8; a YAFF file is UTF-8 text".into(),
                    remedy: FORMAT.into(),
                }
            })
            .and_then(gbcli_font::read)
        {
            Ok(font) => font,
            Err(e) => {
                out.problems.push(ImageProblem {
                    line: Some(e.line),
                    column: std::num::NonZeroU32::new(e.column),
                    ..census::error(&image, ImageLimit::FontText, 1, 1, e.message, &e.remedy)
                });
                continue;
            }
        };
        let picked = match select {
            Select::Build => build_glyphs(s, &font, &image, shown, &mut out.problems)?,
            Select::Request(asked) => match asked.get(&s.name) {
                Some(keys) => requested(&font, keys, &image, &mut out.problems),
                None => Some(((0..font.glyphs.len()).collect(), Vec::new())),
            },
        };
        let Some(picked) = picked else {
            continue;
        };
        let (drawn, composed) = picked;
        let glyphs: Vec<gbcli_font::Glyph> = drawn
            .iter()
            .map(|&i| font.glyphs[i].clone())
            .chain(composed.iter().map(|c| c.glyph.clone()))
            .collect();
        for c in composed.iter().filter(|c| !c.overlap.is_empty()) {
            out.overlaps.push(FontOverlap {
                source: image.clone(),
                key: c.glyph.key.text(),
                base: c.base.to_string(),
                mark: c.mark.to_string(),
                rows: c.overlap.iter().map(|&r| r as u8).collect(),
            });
        }
        out.assets.push(FontAsset {
            source: image.clone(),
            label: s.label.clone(),
            fixed: font.fixed(),
            glyphs: glyphs
                .iter()
                .enumerate()
                .map(|(n, g)| FontGlyph {
                    key: g.key.text(),
                    width: g.width,
                    rows: g.rows,
                    origin: if n < drawn.len() {
                        GlyphOrigin::Drawn
                    } else {
                        GlyphOrigin::Composed
                    },
                })
                .collect(),
        });
        out.emitted.push(Emitted {
            source: image,
            label: s.label.clone(),
            constant: s.constant.clone(),
            fixed: font.fixed(),
            glyphs,
        });
    }
    Ok(out)
}

/// A build's glyphs of `font`: all of them, or its character list's, in
/// the font's order; none, with a problem, past 254 or for a character
/// the font lacks.
fn build_glyphs(
    s: &FontSource,
    font: &Font,
    image: &str,
    shown: &dyn Fn(&Path) -> String,
    problems: &mut Vec<ImageProblem>,
) -> Result<Option<Picked>, BuildError> {
    let list = shown(&s.charlist);
    let mut composed = Vec::new();
    let picked: Vec<usize> = if s.charlist.is_file() {
        let text = fs::read_to_string(&s.charlist).map_err(|error| BuildError::Io {
            path: s.charlist.clone(),
            error,
        })?;
        let mut picked = Vec::new();
        let mut missing = Vec::new();
        for line in text.lines() {
            let set = gbcli_font::set(font, line);
            picked.extend(set.glyphs);
            for m in set.missing {
                compose_or_miss(font, m, &mut composed, &mut missing);
            }
        }
        if let Some(problem) = missing_problem(image, &missing, &format!("{list} lists")) {
            problems.push(problem);
            return Ok(None);
        }
        picked.sort_unstable();
        picked.dedup();
        picked
    } else {
        (0..font.glyphs.len()).collect()
    };
    let count = picked.len() + composed.len();
    if count > MAX_GLYPHS {
        let whose = if s.charlist.is_file() {
            format!("its character list, {list}, names (with the letters it composes)")
        } else {
            "it holds".into()
        };
        problems.push(census::error(
            image,
            ImageLimit::FontGlyphs,
            count,
            MAX_GLYPHS,
            format!(
                "a build emits at most {MAX_GLYPHS} glyphs of a font ($FE and $FF are its \
                 charmap's newline and end), and {whose} {}; gbcli can't see which characters the \
                 program's strings use",
                count
            ),
            &format!(
                "list the characters the program uses in {list}, as text (a named glyph as \
                 <name>): the build then emits only those, in the font's order"
            ),
        ));
        return Ok(None);
    }
    Ok(Some((picked, composed)))
}

/// `m`, a character the font has no glyph for, composed into `composed`
/// when its base and mark are drawn, else added to `missing`; each once.
fn compose_or_miss(
    font: &Font,
    m: String,
    composed: &mut Vec<Composed>,
    missing: &mut Vec<String>,
) {
    let mut chars = m.chars();
    let one = match (chars.next(), chars.next()) {
        (Some(c), None) => Some(c),
        _ => None,
    };
    if let Some(c) = one {
        if composed.iter().any(|x| x.glyph.key == Key::Char(c)) {
            return;
        }
        if let Some(x) = gbcli_font::compose(font, c) {
            composed.push(x);
            return;
        }
    }
    if !missing.contains(&m) {
        missing.push(m);
    }
}

/// The glyphs of `font` that `keys` name, in the font's order; none, with
/// a problem, when one isn't there.
fn requested(
    font: &Font,
    keys: &[String],
    image: &str,
    problems: &mut Vec<ImageProblem>,
) -> Option<Picked> {
    let mut picked = Vec::new();
    let mut missing = Vec::new();
    let mut composed = Vec::new();
    for k in keys {
        match Key::from_wire(k).ok().and_then(|key| font.find(&key)) {
            Some(i) => picked.push(i),
            None => compose_or_miss(font, k.clone(), &mut composed, &mut missing),
        }
    }
    if let Some(p) = missing_problem(image, &missing, "the request asks for") {
        problems.push(p);
        return None;
    }
    picked.sort_unstable();
    picked.dedup();
    Some((picked, composed))
}

/// The problem naming every character `missing` from the font, each with
/// its code point.
fn missing_problem(image: &str, missing: &[String], who: &str) -> Option<ImageProblem> {
    if missing.is_empty() {
        return None;
    }
    let said: Vec<String> = missing
        .iter()
        .map(|m| match Key::from_wire(m) {
            Ok(Key::Char(c)) => format!("'{c}' (u+{:04X})", u32::from(c)),
            _ => format!("\"{m}\""),
        })
        .collect();
    Some(census::error(
        image,
        ImageLimit::FontMissing,
        missing.len(),
        0,
        format!("{who} {}, which the font has no glyph for", said.join(", ")),
        "draw each in the font, or take it out of what asks for it",
    ))
}

/// Every symbol a font labeled `label` (constants `constant`) defines.
pub(crate) fn symbols(label: &str, constant: &str) -> Vec<String> {
    let labels = ["FontGlyphs", "FontGlyphsEnd", "FontWidths", "FontWidthsEnd"]
        .map(|s| format!("{label}{s}"));
    let constants =
        ["GLYPHS", "FIXED", "NEWLINE", "END", "CHARMAP"].map(|s| format!("{constant}_FONT_{s}"));
    labels.into_iter().chain(constants).collect()
}

/// The layout error when two fonts, or a font and an image, would define
/// one symbol.
pub(crate) fn collisions(
    fonts: &[FontSource],
    images: &[find::Source],
    shown: &dyn Fn(&Path) -> String,
) -> Result<(), BuildError> {
    let mut by_symbol: BTreeMap<String, &Path> = BTreeMap::new();
    for s in images {
        for symbol in super::emit::symbols(s.kind, &s.label, &s.constant) {
            by_symbol.insert(symbol, &s.path);
        }
    }
    for f in fonts {
        for symbol in symbols(&f.label, &f.constant) {
            if let Some(other) = by_symbol.insert(symbol.clone(), &f.path) {
                return Err(BuildError::Layout {
                    message: format!(
                        "{} and {} would both define `{symbol}` in the program",
                        shown(other),
                        shown(&f.path)
                    ),
                    remedy: "rename one of them; a font's labels and constants come from its \
                             file name (`main.yaff` is MainFontGlyphs and MAIN_FONT_GLYPHS)"
                        .into(),
                });
            }
        }
    }
    Ok(())
}
