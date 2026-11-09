//! The images a project has, by convention: `assets/bg/*.png`,
//! `assets/sprites/*.png` and `assets/tiles/*.png` (`.PNG` too, in any
//! case), or pixel text in their place (`*.pixels`), each named after its
//! file so the program can address it (`town.png` is `TownTiles`,
//! `TOWN_WIDTH`). A sprite sheet's name may carry its frame size:
//! `hero.16x24.png` (8x8 objects when it is a multiple of 8 high but not
//! 16, `sheet::object_size`). `<name>.dmg.png` beside `<name>.png` is not
//! an image of its own but that image's DMG version, in every build (plan
//! question 3); one with no color image beside it is a layout error. Either
//! may be pixel text; a PNG and pixel text of one name are a layout error.

use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

use gbcli_protocol::{ImageKind, NamedPalettes};

use super::emit;
use crate::paths::quote;
use crate::project::BuildError;

/// One image to convert.
#[derive(Clone, Debug)]
pub(crate) struct Source {
    pub kind: ImageKind,
    /// The file to read.
    pub path: PathBuf,
    /// Its path under `assets/` (`bg/town.png`), as an assignment names it.
    pub name: String,
    /// The file name without `.png` (and without a frame size).
    pub stem: String,
    /// `Town`: the labels' stem.
    pub label: String,
    /// `TOWN`: the constants' stem.
    pub constant: String,
    /// A sprite sheet's frame size in pixels, when its name gives one.
    pub frame: Option<(u32, u32)>,
    /// `<name>.dmg.png` beside it, when there is one: its DMG version.
    pub dmg: Option<PathBuf>,
}

/// The kinds, in the order their images are converted.
pub(crate) const KINDS: [ImageKind; 3] = [ImageKind::Bg, ImageKind::Sprites, ImageKind::Tiles];

/// Every image under `project/assets/`, backgrounds first, then sheets,
/// then tilesets, each directory in name order. A name the assembler can't
/// use, or two images with one name, is a layout error naming the files.
pub(crate) fn sources(
    project: &Path,
    shown: &dyn Fn(&Path) -> String,
) -> Result<Vec<Source>, BuildError> {
    let mut found = Vec::new();
    for kind in KINDS {
        let dir = project.join("assets").join(kind.dir());
        let Ok(entries) = fs::read_dir(&dir) else {
            continue;
        };
        let mut paths: Vec<PathBuf> = Vec::new();
        for entry in entries {
            let path = entry
                .map_err(|error| BuildError::Io {
                    path: dir.clone(),
                    error,
                })?
                .path();
            let image = path
                .extension()
                .is_some_and(|e| e.eq_ignore_ascii_case("png") || e == PIXELS);
            if path.is_file() && image {
                paths.push(path);
            }
        }
        paths.sort();
        two_forms(&paths, shown)?;
        // `<name>.dmg.png` is `<name>.png`'s DMG version, not an image.
        let (overrides, images): (Vec<PathBuf>, Vec<PathBuf>) =
            paths.into_iter().partition(|p| color_stem(p).is_some());
        let mut here: Vec<Source> = images
            .into_iter()
            .map(|path| source(kind, path, shown))
            .collect::<Result<_, _>>()?;
        for o in overrides {
            let stem = color_stem(&o).expect("partitioned on it");
            let Some(s) = here
                .iter_mut()
                .find(|s| s.path.file_stem().is_some_and(|f| f == stem.as_str()))
            else {
                let image = dir.join(format!("{stem}.png"));
                return Err(BuildError::Layout {
                    message: format!(
                        "{} is a DMG version, but there is no color image {} beside it",
                        shown(&o),
                        shown(&image)
                    ),
                    remedy: format!(
                        "add the color image, rename it to match one, or remove {}",
                        shown(&o)
                    ),
                });
            };
            s.dmg = Some(o);
        }
        found.extend(here);
    }
    collisions(&found, shown)?;
    Ok(found)
}

/// Pixel text's extension.
const PIXELS: &str = gbcli_pixels::EXTENSION;

/// The layout error when one image is both a PNG and pixel text, which
/// would be two sources of truth for it.
fn two_forms(paths: &[PathBuf], shown: &dyn Fn(&Path) -> String) -> Result<(), BuildError> {
    let mut by_stem: BTreeMap<&std::ffi::OsStr, &PathBuf> = BTreeMap::new();
    for path in paths {
        let Some(stem) = path.file_stem() else {
            continue;
        };
        let text = |p: &Path| p.extension().is_some_and(|e| e == PIXELS);
        // Two PNGs of one stem (`town.png`, `town.PNG`) are `collisions`'.
        if let Some(other) = by_stem.insert(stem, path)
            && text(other) != text(path)
        {
            let (png, text) = if text(path) {
                (other, path)
            } else {
                (path, other)
            };
            return Err(BuildError::Layout {
                message: format!(
                    "{} and {} are two forms of one image",
                    shown(png),
                    shown(text)
                ),
                remedy: "keep one: a PNG from any tool, or pixel text".into(),
            });
        }
    }
    Ok(())
}

/// The layout error when two of `found` would define one symbol.
pub(crate) fn collisions(
    found: &[Source],
    shown: &dyn Fn(&Path) -> String,
) -> Result<(), BuildError> {
    let mut by_label: BTreeMap<&str, &Source> = BTreeMap::new();
    for s in found {
        if let Some(other) = by_label.insert(&s.label, s) {
            return Err(BuildError::Layout {
                message: format!(
                    "{} and {} would both be `{}` in the program",
                    shown(&other.path),
                    shown(&s.path),
                    s.label
                ),
                remedy: "rename one of them; an image's labels come from its file name".into(),
            });
        }
    }
    // Labels can differ while constants don't: `hero-frame.png` and a
    // sheet `hero.png` both define HERO_FRAME_WIDTH, `hero.png` and
    // `HERO.png` both HERO_TILES.
    let mut by_symbol: BTreeMap<String, &Source> = BTreeMap::new();
    for s in found {
        for symbol in emit::symbols(s.kind, &s.label, &s.constant) {
            if let Some(other) = by_symbol.insert(symbol.clone(), s) {
                return Err(BuildError::Layout {
                    message: format!(
                        "{} and {} would both define `{symbol}` in the program",
                        shown(&other.path),
                        shown(&s.path),
                    ),
                    remedy: "rename one of them; an image's labels and constants come from its \
                             file name (`hero-frame.png` is HeroFrame and HERO_FRAME)"
                        .into(),
                });
            }
        }
    }
    Ok(())
}

/// What defines a symbol, for `palette_collision`'s message.
enum Owner<'a> {
    Image(&'a Source),
    Palette(&'a str),
    /// A sheet's copy of a named palette (`emit::swap_symbols`).
    Swap(&'a Source, &'a str),
}

/// The layout error when a palette of `named` (in the file shown as
/// `file`), or a sheet's copy of one, would define a symbol an image of
/// `sources`, another palette or another copy defines: their labels come
/// from different names, so only together can they collide. A sheet's
/// `<Label>Frame<n>` counts too, for any `n`, since frames can be added.
pub(crate) fn palette_collision(
    sources: &[Source],
    named: &NamedPalettes,
    file: &str,
    shown: &dyn Fn(&Path) -> String,
) -> Result<(), BuildError> {
    let say = |owner: &Owner| match owner {
        Owner::Image(s) => shown(&s.path),
        Owner::Palette(name) => format!("the palette `{name}` in {file}"),
        Owner::Swap(s, name) => format!(
            "{}'s copy of the palette `{name}` in {file}",
            shown(&s.path)
        ),
    };
    let mut by_symbol: BTreeMap<String, Owner> = BTreeMap::new();
    for s in sources {
        for symbol in emit::symbols(s.kind, &s.label, &s.constant) {
            by_symbol.insert(symbol, Owner::Image(s));
        }
    }
    let sheets: Vec<&Source> = sources
        .iter()
        .filter(|s| s.kind == ImageKind::Sprites)
        .collect();
    let palettes = named.palettes.iter().flat_map(|p| {
        emit::palette_symbols(&p.name)
            .into_iter()
            .map(|symbol| (symbol, Owner::Palette(&p.name)))
    });
    let swaps = sheets.iter().flat_map(|&s| {
        named.palettes.iter().flat_map(move |p| {
            emit::swap_symbols(&s.label, &p.name)
                .into_iter()
                .map(move |symbol| (symbol, Owner::Swap(s, &p.name)))
        })
    });
    for (symbol, owner) in palettes.chain(swaps) {
        let other = match by_symbol.get(&symbol) {
            Some(other) => Some(say(other)),
            None => frame_of(&symbol, &sheets).map(|s| format!("{}'s frames", shown(&s.path))),
        };
        if let Some(other) = other {
            return Err(BuildError::Layout {
                message: format!(
                    "{other} and {} would both define `{symbol}` in the program",
                    say(&owner)
                ),
                remedy: "rename the image or the palette; a palette `forest` is PaletteForest \
                         and PALETTE_FOREST, a sheet `hero.png`'s copy of it \
                         HeroPaletteForest, an image `forest.png` ForestTiles and FOREST_TILES"
                    .into(),
            });
        }
        by_symbol.insert(symbol, owner);
    }
    Ok(())
}

/// The sheet of `sheets` whose frame label `symbol` would be:
/// `<Label>Frame<n>`, `n` as the frame lists write it.
fn frame_of<'a>(symbol: &str, sheets: &[&'a Source]) -> Option<&'a Source> {
    sheets.iter().copied().find(|s| {
        symbol
            .strip_prefix(s.label.as_str())
            .and_then(|rest| rest.strip_prefix("Frame"))
            .and_then(|n| n.parse::<usize>().ok().map(|v| v.to_string() == n))
            .unwrap_or(false)
    })
}

/// The image at `path`, of `kind`, named after its file.
pub(crate) fn source(
    kind: ImageKind,
    path: PathBuf,
    shown: &dyn Fn(&Path) -> String,
) -> Result<Source, BuildError> {
    let file = path
        .file_name()
        .map(|s| s.to_string_lossy().into_owned())
        .unwrap_or_default();
    let name = path
        .file_stem()
        .map(|s| s.to_string_lossy().into_owned())
        .unwrap_or_default();
    let (stem, frame) = match name.split_once('.') {
        Some((stem, size)) if kind == ImageKind::Sprites => match frame_size(size) {
            Some(frame) => (stem.to_string(), Some(frame)),
            None => {
                // The size is checked first, so the stem may be no name either.
                let stem = if is_name(stem) {
                    stem.to_string()
                } else {
                    name_like(stem)
                };
                return Err(BuildError::Layout {
                    message: format!(
                        "{}: `{size}` is not a frame size; a sheet's name may end in its \
                         frame's width and height in pixels",
                        shown(&path)
                    ),
                    remedy: format!(
                        "rename it to {}.WxH.png, e.g. {}.16x16.png, or drop the size to make \
                         the whole sheet one frame",
                        stem, stem
                    ),
                });
            }
        },
        _ => (name.clone(), None),
    };
    if !is_name(&stem) {
        let offer = name_like(&stem);
        let suffix = frame.map_or(String::new(), |(w, h)| format!(".{w}x{h}"));
        return Err(BuildError::Layout {
            message: format!(
                "{}: an image's name becomes its labels, so it starts with a letter and holds \
                 only letters, digits, `_` and `-`",
                shown(&path)
            ),
            remedy: format!(
                "rename it, e.g. to {}",
                quote(&format!("{offer}{suffix}.png"))
            ),
        });
    }
    let words: Vec<&str> = stem.split(['_', '-']).filter(|w| !w.is_empty()).collect();
    let label = words
        .iter()
        .map(|w| {
            let mut c = w.chars();
            let first = c.next().expect("words are non-empty").to_ascii_uppercase();
            std::iter::once(first).chain(c).collect::<String>()
        })
        .collect();
    let constant = words
        .iter()
        .map(|w| w.to_ascii_uppercase())
        .collect::<Vec<_>>()
        .join("_");
    Ok(Source {
        kind,
        path,
        name: format!("{}/{file}", kind.dir()),
        stem,
        label,
        constant,
        frame,
        dmg: None,
    })
}

/// The file stem of the color image `path` is the DMG version of, when its
/// name ends `.dmg.png` (in any case): `town` for `town.dmg.png`.
fn color_stem(path: &Path) -> Option<String> {
    let stem = path.file_stem()?.to_string_lossy();
    let (color, suffix) = stem.rsplit_once('.')?;
    suffix
        .eq_ignore_ascii_case(DMG_SUFFIX)
        .then(|| color.to_string())
}

/// What a DMG version's name adds to its color image's, before `.png`.
const DMG_SUFFIX: &str = "dmg";

/// Where `name.dmg.png` (or `name.dmg.pixels` for pixel text), the DMG
/// version of the color image at `image`, would be.
pub(crate) fn dmg_path(image: &Path) -> PathBuf {
    let stem = image
        .file_stem()
        .map(|s| s.to_string_lossy().into_owned())
        .unwrap_or_default();
    let ext = match image.extension() {
        Some(e) if e == PIXELS => PIXELS,
        _ => "png",
    };
    image.with_file_name(format!("{stem}.{DMG_SUFFIX}.{ext}"))
}

/// `16x24` as (16, 24).
fn frame_size(text: &str) -> Option<(u32, u32)> {
    let (w, h) = text.split_once('x')?;
    let digits = |s: &str| !s.is_empty() && s.bytes().all(|b| b.is_ascii_digit());
    if !digits(w) || !digits(h) {
        return None;
    }
    Some((w.parse().ok()?, h.parse().ok()?))
}

fn is_name(stem: &str) -> bool {
    let mut chars = stem.chars();
    chars.next().is_some_and(|c| c.is_ascii_alphabetic())
        && chars.all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-')
}

/// The name nearest `stem` that is one: other characters become `_`, and
/// anything before the first letter goes.
fn name_like(stem: &str) -> String {
    let start = stem.find(|c: char| c.is_ascii_alphabetic());
    let rest = start.map_or("image", |i| &stem[i..]);
    rest.chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || c == '-' {
                c
            } else {
                '_'
            }
        })
        .collect()
}

#[cfg(test)]
mod tests;
