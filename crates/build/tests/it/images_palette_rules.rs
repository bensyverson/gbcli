//! `assets/palettes.txt` against the protocol's one rule set for named
//! palettes, and the old `assets/palette.txt` (the review of 81cf3d2,
//! leaf jJjaK9).

use crate::common;
use crate::pngs;

use std::fs;

use gbcli_build::{BuildError, build};
use gbcli_protocol::{
    BuildMade, ImageLimit, ImageProblem, NamedPalettes, PaletteFault, Severity, ShadeRule,
};

use common::{Temp, rgbds, starter};
use pngs::*;

const PALETTES: &str = "dmg: #ffffff #aaaaaa #555555 #000000\nforest: #fff #8c4 #363 #021\n";

fn sky(temp: &Temp) {
    write_png(&temp.path("demo/assets/bg/sky.png"), 16, 8, |x, y| {
        [WHITE, LIGHT, DARK, BLACK][((x + y) % 4) as usize]
    });
}

fn with(name: &str, text: &str) -> Result<BuildMade, BuildError> {
    let (temp, options) = starter(name);
    sky(&temp);
    fs::write(temp.path("demo/assets/palettes.txt"), text).unwrap();
    build(&rgbds(), &options)
}

fn palette_problem(r: Result<BuildMade, BuildError>) -> ImageProblem {
    match r {
        Err(BuildError::Failed { made, .. }) => made
            .images
            .and_then(|i| {
                i.problems
                    .into_iter()
                    .find(|p| p.limit == ImageLimit::PaletteFile)
            })
            .expect("a palette_file problem"),
        other => panic!("{other:?}"),
    }
}

/// A path that escapes `assets/` is refused at its line, with the message
/// the wire gives for the same struct.
#[test]
fn the_file_refuses_a_path_as_the_wire_does() {
    let p = palette_problem(with(
        "rules-escape",
        &format!("{PALETTES}../bg/sky.png = forest\n"),
    ));
    assert_eq!(p.line, Some(3));
    let wire = PaletteFault::BadImage {
        image: "../bg/sky.png".into(),
    };
    assert!(p.message.contains(&wire.to_string()), "{}", p.message);
    // The struct a client sends (gbcli-protocol's tests read it from JSON).
    let sent = NamedPalettes {
        assignments: [("../bg/sky.png".to_string(), "forest".to_string())].into(),
        ..NamedPalettes::default()
    };
    assert_eq!(sent.check(), Err(wire));
}

/// Of two images the project lacks, the earlier line is named.
#[test]
fn missing_images_are_reported_in_file_order() {
    let p = palette_problem(with(
        "rules-missing-order",
        &format!("{PALETTES}bg/zz.png = forest\nbg/aa.png = forest\n"),
    ));
    assert_eq!(p.line, Some(3), "{}", p.message);
    assert!(p.message.contains("bg/zz.png"), "{}", p.message);
}

/// An old `assets/palette.txt` is no longer read; the build says so and
/// names `assets/palettes.txt`, with or without images.
#[test]
fn an_old_palette_txt_warns_naming_palettes_txt() {
    for images in [true, false] {
        let (temp, options) = starter(&format!("rules-old-{images}"));
        if images {
            sky(&temp);
        }
        fs::create_dir_all(temp.path("demo/assets")).unwrap();
        fs::write(
            temp.path("demo/assets/palette.txt"),
            "#ffffff\n#aaaaaa\n#555555\n#000000\n",
        )
        .unwrap();
        let made = build(&rgbds(), &options).unwrap();
        let problems = made.images.map(|i| i.problems).unwrap_or_default();
        let warning = problems
            .iter()
            .find(|p| p.limit == ImageLimit::PaletteFile)
            .unwrap_or_else(|| panic!("images {images}: {problems:?}"));
        assert_eq!(warning.severity, Severity::Warning);
        assert!(
            warning.message.contains("assets/palette.txt")
                && warning.message.contains("assets/palettes.txt"),
            "{}",
            warning.message
        );
    }
}

/// The new file still drives the shades beside an old one.
#[test]
fn palettes_txt_still_reads_beside_an_old_palette_txt() {
    let (temp, options) = starter("rules-old-and-new");
    sky(&temp);
    fs::write(temp.path("demo/assets/palette.txt"), "garbage\n").unwrap();
    fs::write(temp.path("demo/assets/palettes.txt"), PALETTES).unwrap();
    let made = build(&rgbds(), &options).unwrap();
    assert_eq!(
        made.images.unwrap().shades.unwrap().rule,
        ShadeRule::Declared
    );
}
