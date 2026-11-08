//! The images a project has, by convention: `assets/bg/*.png` and
//! `assets/sprites/*.png` (`.PNG` too, in any case), each named after its
//! file so the program can address it (`town.png` is `TownTiles`,
//! `TOWN_WIDTH`). A sprite sheet's name may carry its frame size:
//! `hero.16x24.png`. `<name>.dmg.png` beside `<name>.png` is not an image
//! of its own but that image's DMG version for `build --gbc` (plan
//! question 3); one with no color image beside it is a layout error.

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
    /// `<name>.dmg.png` beside it, when there is one: its DMG version.
    pub dmg: Option<PathBuf>,
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

/// Where `name.dmg.png`, the DMG version of the color image at `image`,
/// would be.
pub(crate) fn dmg_path(image: &Path) -> PathBuf {
    let stem = image
        .file_stem()
        .map(|s| s.to_string_lossy().into_owned())
        .unwrap_or_default();
    image.with_file_name(format!("{stem}.{DMG_SUFFIX}.png"))
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
        let remedy = named(ImageKind::Sprites, "2 hero.big.png").unwrap_err();
        assert!(remedy.contains(" hero.16x16.png"), "{remedy}");
        let remedy = named(ImageKind::Bg, "town.16x16.png").unwrap_err();
        assert!(remedy.contains("town_16x16.png"), "{remedy}");
    }

    /// A project directory with these (empty) files under `assets/`.
    fn project(name: &str, files: &[&str]) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("gbcli-find-{}-{name}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        for f in files {
            let path = dir.join("assets").join(f);
            fs::create_dir_all(path.parent().unwrap()).unwrap();
            fs::write(path, b"").unwrap();
        }
        dir
    }

    fn found(dir: &Path) -> Result<Vec<Source>, BuildError> {
        sources(dir, &|p| {
            p.strip_prefix(dir).unwrap_or(p).display().to_string()
        })
    }

    #[test]
    fn a_dmg_png_is_its_color_images_dmg_version_not_an_image() {
        let dir = project(
            "dmg-version",
            &[
                "bg/town.png",
                "bg/town.dmg.png",
                "bg/sky.png",
                "sprites/hero.16x16.png",
                "sprites/hero.16x16.DMG.PNG",
            ],
        );
        let found = found(&dir).unwrap();
        let named: Vec<(&str, Option<PathBuf>)> = found
            .iter()
            .map(|s| {
                let dmg = s.dmg.as_ref().map(|p| p.strip_prefix(&dir).unwrap().into());
                (s.stem.as_str(), dmg)
            })
            .collect();
        assert_eq!(
            named,
            [
                ("sky", None),
                ("town", Some(PathBuf::from("assets/bg/town.dmg.png"))),
                (
                    "hero",
                    Some(PathBuf::from("assets/sprites/hero.16x16.DMG.PNG"))
                ),
            ]
        );
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_dmg_png_without_its_color_image_is_a_layout_error() {
        let dir = project("dmg-orphan", &["bg/town.dmg.png", "bg/sky.png"]);
        let body = found(&dir).unwrap_err().body();
        assert!(
            body.message.contains("assets/bg/town.dmg.png"),
            "{}",
            body.message
        );
        assert!(
            body.message.contains("assets/bg/town.png"),
            "{}",
            body.message
        );
        let _ = fs::remove_dir_all(&dir);
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
