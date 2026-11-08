//! A sheet drawn for GB Studio: its transparent pixels are the key green
//! #65ff00, opaque, which gbcli doesn't key. The problems such a sheet
//! raises name the green (agent eval run 4, leaf YI5gQ6), and nothing is
//! keyed automatically.

use crate::common;
use crate::pngs;

use gbcli_protocol::{BuildTarget, ImageLimit, ImageProblem};

use common::{rgbds, starter};
use pngs::*;

use gbcli_build::build;

/// GB Studio's key green.
const KEY: Rgba = [0x65, 0xFF, 0x00, 0xFF];

/// The first of `problems` for the image at `source`.
fn for_image(problems: &[ImageProblem], source: &str) -> ImageProblem {
    problems
        .iter()
        .find(|p| p.image.as_deref().is_some_and(|i| i.ends_with(source)))
        .unwrap_or_else(|| panic!("no problem for {source} in {problems:#?}"))
        .clone()
}

fn names_the_key(p: &ImageProblem) -> bool {
    p.remedy.contains("#65ff00") && p.remedy.contains("GB Studio")
}

/// An opaque 8x16 sheet: `back` around a black stroke.
fn sheet(back: Rgba) -> impl Fn(u32, u32) -> Rgba {
    move |x, _| if x == 3 { BLACK } else { back }
}

#[test]
fn an_opaque_sheet_in_gb_studios_key_green_is_told_to_make_it_transparent() {
    let (temp, options) = starter("key-green-opaque");
    // Inside the match (g > 249, r < 180, b < 20), and just outside it on
    // each channel.
    let cases: [(&str, Rgba, bool); 5] = [
        ("key.png", KEY, true),
        ("edge.png", [179, 250, 19, 0xFF], true),
        ("red.png", [180, 255, 0, 0xFF], false),
        ("green.png", [0x65, 249, 0, 0xFF], false),
        ("blue.png", [0x65, 0xFF, 20, 0xFF], false),
    ];
    for (file, back, _) in cases {
        write_png(
            &temp.path(&format!("demo/assets/sprites/{file}")),
            8,
            16,
            sheet(back),
        );
    }
    let made = build(&rgbds(), &options).unwrap();
    let found = problems(&made, ImageLimit::ObjNoTransparency);
    for (file, _, hinted) in cases {
        let p = for_image(&found, file);
        assert_eq!(names_the_key(&p), hinted, "{file}: {p:#?}");
        assert!(p.remedy.contains("alpha 0"), "{file}: {p:#?}");
    }
}

#[test]
fn a_gb_studio_sheet_in_four_colors_is_told_its_green_is_the_fourth() {
    let (temp, options) = starter("key-green-four");
    // GB Studio's sprite colors on its key green: 4 colors, 3 + the key.
    let gb_studio = |x: u32, y: u32| match (x, y % 4) {
        (0, _) => [0xE0, 0xF8, 0xCF, 0xFF],
        (1, _) => [0x86, 0xC0, 0x6C, 0xFF],
        (2, _) => [0x07, 0x18, 0x21, 0xFF],
        _ => KEY,
    };
    write_png(
        &temp.path("demo/assets/sprites/studio.png"),
        8,
        16,
        gb_studio,
    );
    // The same with one pixel already keyed out: its green is a color.
    write_png(
        &temp.path("demo/assets/sprites/keyed.png"),
        8,
        16,
        |x, y| {
            if (x, y) == (7, 15) {
                CLEAR
            } else {
                gb_studio(x, y)
            }
        },
    );
    let made = refused(&options);
    let found = problems(&made, ImageLimit::DmgPalette);
    assert!(
        names_the_key(&for_image(&found, "studio.png")),
        "{found:#?}"
    );
    assert!(
        !names_the_key(&for_image(&found, "keyed.png")),
        "{found:#?}"
    );
}

#[test]
fn under_gbc_a_color_sheet_on_the_key_green_is_told_too() {
    let (temp, mut options) = starter("key-green-gbc");
    options.target = BuildTarget::Gbc;
    write_png(
        &temp.path("demo/assets/sprites/hero.png"),
        8,
        16,
        sheet(KEY),
    );
    // Four colors that fall to four shades: the derived version can't be
    // one object palette.
    write_png(
        &temp.path("demo/assets/sprites/four.png"),
        8,
        16,
        |x, _| match x {
            0 => WHITE,
            1 => DARK,
            2 => BLACK,
            _ => KEY,
        },
    );
    let made = refused(&options);
    let p = for_image(&problems(&made, ImageLimit::ObjNoTransparency), "hero.png");
    assert!(names_the_key(&p), "{p:#?}");
    let p = for_image(&problems(&made, ImageLimit::DmgPalette), "four.png");
    assert!(names_the_key(&p), "{p:#?}");
}
