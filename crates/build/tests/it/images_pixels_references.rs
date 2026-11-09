//! Pixel text drawn with palette references (`PALETTE N`, `dmg N`)
//! converts and builds exactly as the PNG drawn in the palettes' literal
//! colors, which the racer test in images_pixels never exercises.

use crate::common;
use crate::pngs;

use std::fs;
use std::path::Path;

use gbcli_build::{AssetsInput, AssetsOptions, BuildOptions, assets, build};
use gbcli_pixels::{Layout, decode_png, frame_size, from_raster, to_text};
use gbcli_protocol::BuildTarget;

use common::{Temp, rgbds};
use pngs::*;

fn copy_dir(from: &Path, to: &Path) {
    fs::create_dir_all(to).unwrap();
    for entry in fs::read_dir(from).unwrap() {
        let path = entry.unwrap().path();
        let target = to.join(path.file_name().unwrap());
        if path.is_dir() {
            copy_dir(&path, &target);
        } else {
            fs::copy(&path, &target).unwrap();
        }
    }
}

/// Each palette line of palettes.txt: name and its four `#rrggbb` colors.
fn palette_lines(text: &str) -> Vec<(String, Vec<String>)> {
    let mut out = Vec::new();
    for line in text.lines() {
        let words: Vec<&str> = line.split_whitespace().take_while(|w| *w != "#").collect();
        if let [name, colors @ ..] = words.as_slice()
            && let Some(name) = name.strip_suffix(':')
            && colors.len() == 4
        {
            out.push((
                name.to_string(),
                colors.iter().map(|c| c.to_lowercase()).collect(),
            ));
        }
    }
    out
}

/// Every PNG under `dir` replaced by canonical pixel text whose colors
/// name `palette`'s entries (and the dmg line's) wherever they match.
fn to_referencing_pixels(dir: &Path, palettes: &[(String, Vec<String>)], use_dmg: bool) {
    for entry in fs::read_dir(dir).unwrap() {
        let path = entry.unwrap().path();
        if path.is_dir() {
            to_referencing_pixels(&path, palettes, use_dmg);
        } else if path.extension().is_some_and(|e| e == "png") {
            let raster = decode_png(&fs::read(&path).unwrap()).unwrap();
            let name = path.file_name().unwrap().to_string_lossy().into_owned();
            let stem = name.split('.').next().unwrap().to_string();
            let layout = Layout::Rows {
                frame: frame_size(&name),
            };
            let text = to_text(&from_raster(&raster).unwrap(), layout).unwrap();
            let rewritten: Vec<String> = text
                .lines()
                .map(|line| {
                    let words: Vec<&str> = line.split(' ').collect();
                    if let ["color", c, hex] = words.as_slice() {
                        for (pname, colors) in palettes {
                            let wanted = pname == &stem || (use_dmg && pname == "dmg");
                            if !wanted {
                                continue;
                            }
                            if let Some(i) = colors.iter().position(|x| x == hex) {
                                return format!("color {c} {pname} {i}");
                            }
                        }
                    }
                    line.to_string()
                })
                .collect();
            fs::write(path.with_extension("pixels"), rewritten.join("\n") + "\n").unwrap();
            fs::remove_file(&path).unwrap();
        }
    }
}

fn rom(project: &Path, target: BuildTarget) -> Vec<u8> {
    let options = BuildOptions {
        target,
        ..BuildOptions::new(project.to_path_buf(), project.to_path_buf())
    };
    if let Err(e) = build(&rgbds(), &options) {
        panic!("{e:?}");
    }
    let name = project.file_name().unwrap().to_string_lossy();
    fs::read(project.join(format!("build/{name}.gb"))).unwrap()
}

fn racer_with_references(
    tag: &str,
    use_dmg: bool,
) -> (Temp, std::path::PathBuf, std::path::PathBuf, usize) {
    let temp = Temp::new(tag);
    let examples = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../examples");
    let (png, text) = (temp.path("png/racer"), temp.path("text/racer"));
    copy_dir(&examples.join("racer"), &png);
    copy_dir(&examples.join("racer"), &text);
    let palettes_path = text.join("assets/palettes.txt");
    let palettes_text = fs::read_to_string(&palettes_path).unwrap();
    to_referencing_pixels(
        &text.join("assets"),
        &palette_lines(&palettes_text),
        use_dmg,
    );
    fs::write(&palettes_path, palettes_text.replace(".png =", ".pixels =")).unwrap();
    // How many references the rewrite made: a probe that made none tests nothing.
    let mut refs = 0;
    for sub in ["bg", "sprites", "tiles"] {
        for e in fs::read_dir(text.join("assets").join(sub)).unwrap() {
            let body = fs::read_to_string(e.unwrap().path()).unwrap();
            refs += body
                .lines()
                .filter(|l| l.starts_with("color ") && !l.contains('#') && !l.ends_with("clear"))
                .count();
        }
    }
    (temp, png, text, refs)
}

/// Every file under `dir`, path relative to it, with its bytes.
fn tree(dir: &Path, base: &Path, out: &mut Vec<(String, Vec<u8>)>) {
    let mut entries: Vec<_> = fs::read_dir(dir)
        .unwrap()
        .map(|e| e.unwrap().path())
        .collect();
    entries.sort();
    for path in entries {
        if path.is_dir() {
            tree(&path, base, out);
        } else {
            let rel = path
                .strip_prefix(base)
                .unwrap()
                .to_string_lossy()
                .into_owned();
            out.push((rel, fs::read(&path).unwrap()));
        }
    }
}

/// Color art and indexed art drawn as `sea N` / `dmg N` (palettes whose
/// colors aren't RGB555-exact) convert to the same data and previews as
/// PNGs drawn in the palettes' literal colors. (The racer has no color
/// in a named palette's literal spelling, so its test can't show this.)
#[test]
fn named_palette_references_convert_like_the_literal_png() {
    const SEA: [[u8; 4]; 4] = [
        [0xf0, 0xf8, 0xff, 0xff],
        [0xa8, 0xd8, 0xf0, 0xff],
        [0x58, 0xa0, 0x40, 0xff],
        [0x60, 0x40, 0x20, 0xff],
    ];
    let shade = |x: u32, y: u32| ((x / 2 + y * 3) % 4) as usize;
    let mut results = Vec::new();
    for target in [BuildTarget::Dmg, BuildTarget::Gbc] {
        let mut trees = Vec::new();
        for form in ["png", "pixels"] {
            let temp = Temp::new(&format!("pixels-refs-sea-{form}-{target:?}"));
            let assets_dir = temp.path("demo/assets");
            fs::create_dir_all(assets_dir.join("bg")).unwrap();
            fs::write(
                assets_dir.join("palettes.txt"),
                format!(
                    "dmg: #ffffff #aaaaaa #555555 #000000\nsea: #f0f8ff #a8d8f0 #58a040 #604020\n\
                     bg/indexed.{form} = sea\n"
                ),
            )
            .unwrap();
            for name in ["indexed", "color"] {
                // Indexed art draws in the dmg shades; color art in sea's colors.
                let indexed = name == "indexed";
                if form == "png" {
                    write_png(&assets_dir.join(format!("bg/{name}.png")), 16, 8, |x, y| {
                        if indexed {
                            let g = [0xff, 0xaa, 0x55, 0x00][shade(x, y)];
                            [g, g, g, 0xff]
                        } else {
                            SEA[shade(x, y)]
                        }
                    });
                } else {
                    let mut text = String::from("format gbcli-pixels 1\nsize 16 8\n");
                    for i in 0..4 {
                        let p = if indexed { "dmg" } else { "sea" };
                        text += &format!("color {i} {p} {i}\n");
                    }
                    text += "rows\n";
                    for y in 0..8 {
                        for x in 0..16 {
                            text += &shade(x, y).to_string();
                        }
                        text += "\n";
                    }
                    fs::write(assets_dir.join(format!("bg/{name}.pixels")), text).unwrap();
                }
            }
            let keep = temp.path("kept");
            let options = AssetsOptions {
                target,
                keep: Some(keep.clone()),
                ..AssetsOptions::new(AssetsInput::Project(temp.path("demo")), temp.0.clone())
            };
            let made = assets(&options).unwrap_or_else(|e| panic!("{form} {target:?}: {e:?}"));
            let mut t = Vec::new();
            tree(&keep, &keep, &mut t);
            trees.push((t, made.images.problems.len()));
        }
        let (a, b) = (&trees[0], &trees[1]);
        let names = |t: &Vec<(String, Vec<u8>)>| t.iter().map(|f| f.0.clone()).collect::<Vec<_>>();
        assert_eq!(names(&a.0), names(&b.0), "{target:?}: different files");
        let differ: Vec<String> =
            a.0.iter()
                .zip(&b.0)
                // A preview's label names its source file, which differs.
                .filter(|(x, y)| x.1 != y.1 && !x.0.ends_with(".preview.png"))
                .map(|(x, _)| x.0.clone())
                .collect();
        results.push((target, differ, a.1, b.1));
    }
    let bad: Vec<_> = results
        .iter()
        .filter(|(_, differ, pa, pb)| !differ.is_empty() || pa != pb)
        .collect();
    assert!(
        bad.is_empty(),
        "files differ (target, files, problems png, problems text): {bad:?}"
    );
}

/// The racer with every color its own palette's entry (`car N`) or the dmg
/// line's (`dmg N`) where one matches builds the same ROM as its PNGs.
#[test]
fn dmg_and_palette_references_build_the_same_rom_as_the_png() {
    let (_temp, png, text, refs) = racer_with_references("pixels-refs-dmg", true);
    assert!(refs > 0, "the rewrite made no references");
    for target in [BuildTarget::Dmg, BuildTarget::Gbc] {
        assert!(
            rom(&png, target) == rom(&text, target),
            "{target:?}: the ROMs differ ({refs} refs)"
        );
    }
}
