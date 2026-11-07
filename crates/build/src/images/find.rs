//! The images a project has, by convention: `assets/bg/*.png` and
//! `assets/sprites/*.png` (`.PNG` too, in any case), each named after its
//! file so the program can address it (`town.png` is `TownTiles`,
//! `TOWN_WIDTH`). A sprite sheet's name may carry its frame size:
//! `hero.16x24.png`.

use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

use gbcli_protocol::ImageKind;

use super::emit;
use crate::paths::quote;
use crate::project::BuildError;

/// One image to convert.
#[derive(Clone, Debug)]
pub(crate) struct Source {
    pub kind: ImageKind,
    pub path: PathBuf,
    /// The file name without `.png` (and without a frame size).
    pub stem: String,
    /// `Town`: the labels' stem.
    pub label: String,
    /// `TOWN`: the constants' stem.
    pub constant: String,
    /// A sprite sheet's frame size in pixels, when its name gives one.
    pub frame: Option<(u32, u32)>,
}

/// Every image under `project/assets/`, backgrounds first, each directory
/// in name order. A name the assembler can't use, or two images with one
/// name, is a layout error naming the files.
pub(crate) fn sources(
    project: &Path,
    shown: &dyn Fn(&Path) -> String,
) -> Result<Vec<Source>, BuildError> {
    let mut found = Vec::new();
    for kind in [ImageKind::Bg, ImageKind::Sprites] {
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
            let png = path
                .extension()
                .is_some_and(|e| e.eq_ignore_ascii_case("png"));
            if path.is_file() && png {
                paths.push(path);
            }
        }
        paths.sort();
        for path in paths {
            found.push(source(kind, path, shown)?);
        }
    }
    let mut by_label: BTreeMap<&str, &Source> = BTreeMap::new();
    for s in &found {
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
    for s in &found {
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
    Ok(found)
}

fn source(
    kind: ImageKind,
    path: PathBuf,
    shown: &dyn Fn(&Path) -> String,
) -> Result<Source, BuildError> {
    let name = path
        .file_stem()
        .map(|s| s.to_string_lossy().into_owned())
        .unwrap_or_default();
    let (stem, frame) = match name.split_once('.') {
        Some((stem, size)) if kind == ImageKind::Sprites => match frame_size(size) {
            Some(frame) => (stem.to_string(), Some(frame)),
            None => {
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
        stem,
        label,
        constant,
        frame,
    })
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
mod tests {
    use super::*;

    fn named(kind: ImageKind, file: &str) -> Result<Source, String> {
        source(kind, PathBuf::from(file), &|p| p.display().to_string()).map_err(|e| e.body().remedy)
    }

    #[test]
    fn a_name_becomes_pascal_labels_and_upper_constants() {
        let s = named(ImageKind::Bg, "big-tree_2.png").unwrap();
        assert_eq!(
            (s.label.as_str(), s.constant.as_str()),
            ("BigTree2", "BIG_TREE_2")
        );
        assert_eq!(s.frame, None);
    }

    #[test]
    fn a_sheet_name_may_carry_its_frame_size_and_a_background_name_may_not() {
        let s = named(ImageKind::Sprites, "hero.16x24.png").unwrap();
        assert_eq!((s.label.as_str(), s.frame), ("Hero", Some((16, 24))));
        let remedy = named(ImageKind::Sprites, "hero.big.png").unwrap_err();
        assert!(remedy.contains("hero.16x16.png"), "{remedy}");
        let remedy = named(ImageKind::Bg, "town.16x16.png").unwrap_err();
        assert!(remedy.contains("town_16x16.png"), "{remedy}");
    }

    #[test]
    fn a_name_that_cant_be_a_label_is_offered_one_that_can() {
        assert!(
            named(ImageKind::Bg, "2 town.png")
                .unwrap_err()
                .contains("town.png")
        );
        assert!(
            named(ImageKind::Bg, "café.png")
                .unwrap_err()
                .contains("caf_.png")
        );
    }
}
