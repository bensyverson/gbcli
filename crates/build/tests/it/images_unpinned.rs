//! Indexed art whose shades no `dmg` line pins (Ben's ruling, plan
//! question 3, leaf d8FHRl): the cascade then chooses its shades from every
//! color in the project, so other art can recolor it on the GBC. Under
//! `--gbc` the conversion warns once for the project, `shades_unpinned`.
//! Run through `assets`, which shares the build's conversion and needs no
//! rgbasm.

use crate::common;
use crate::pngs;

use std::fs;

use gbcli_build::{AssetsInput, AssetsOptions, assets};
use gbcli_protocol::{BuildTarget, GbcPalette, ImageLimit, ImageProblem, ImagesMade, Severity};

use common::{Temp, rgbds, starter};
use pngs::*;

const EMBER: &str = "ember: #ffe #f80 #a20 #200\n";
const DMG: &str = "dmg: #ffffff #aaaaaa #555555 #000000\n";

/// An 8x16 sheet in two grays on transparent.
fn sheet(temp: &Temp, name: &str) {
    write_png(
        &temp.path(&format!("demo/assets/sprites/{name}.png")),
        8,
        16,
        |x, _| match x {
            0 => [0xA0, 0xA0, 0xA0, 0xFF],
            1 => [0x60, 0x60, 0x60, 0xFF],
            _ => CLEAR,
        },
    );
}

/// An 8x8 background in the same two grays.
fn background(temp: &Temp, name: &str) {
    write_png(
        &temp.path(&format!("demo/assets/bg/{name}.png")),
        8,
        8,
        |x, _| {
            if x < 4 {
                [0xA0, 0xA0, 0xA0, 0xFF]
            } else {
                [0x60, 0x60, 0x60, 0xFF]
            }
        },
    );
}

/// An 8x16 color sheet in white, black and red: unrelated color art.
fn coin(temp: &Temp) {
    write_png(
        &temp.path("demo/assets/sprites/coin.png"),
        8,
        16,
        |x, _| match x {
            0 => WHITE,
            1 => BLACK,
            2 => [0xFF, 0x00, 0x00, 0xFF],
            _ => CLEAR,
        },
    );
}

fn palettes(temp: &Temp, text: &str) {
    fs::write(temp.path("demo/assets/palettes.txt"), text).unwrap();
}

fn convert(temp: &Temp, target: BuildTarget) -> ImagesMade {
    convert_into(temp, target, None)
}

/// `convert`, keeping the data in `keep` (under the temp folder) if given.
fn convert_into(temp: &Temp, target: BuildTarget, keep: Option<&str>) -> ImagesMade {
    let options = AssetsOptions {
        target,
        keep: keep.map(|k| temp.path(k)),
        ..AssetsOptions::new(AssetsInput::Project(temp.path("demo")), temp.0.clone())
    };
    let made = assets(rgbds().images(), &options).unwrap().images;
    assert!(made.first_error().is_none(), "{:#?}", made.problems);
    made
}

fn unpinned(made: &ImagesMade) -> Vec<&ImageProblem> {
    made.problems
        .iter()
        .filter(|p| p.limit == ImageLimit::ShadesUnpinned)
        .collect()
}

/// The defect the warning names (the review probe of 2efed71): an
/// unrelated color sheet moves the indexed sheet's OBJ palette.
#[test]
fn color_art_recolors_an_indexed_sheet_and_the_project_warns_once() {
    let (temp, _) = starter("unpinned-sheet");
    sheet(&temp, "hero");
    sheet(&temp, "bat");
    palettes(
        &temp,
        &format!("{EMBER}sprites/hero.png = ember\nsprites/bat.png = ember\n"),
    );
    let slot =
        |made: &ImagesMade| -> GbcPalette { made.gbc.as_ref().expect("gbc").obj_palettes[0] };
    let alone = convert(&temp, BuildTarget::Gbc);
    coin(&temp);
    let with_coin = convert(&temp, BuildTarget::Gbc);
    assert_ne!(
        slot(&alone),
        slot(&with_coin),
        "the premise: the slot moves"
    );
    for made in [&alone, &with_coin] {
        let [w] = &unpinned(made)[..] else {
            panic!("one warning for two indexed sheets: {:#?}", made.problems)
        };
        assert_eq!(w.image, None);
        assert_eq!(w.severity, Severity::Warning);
        assert!(w.message.contains("sprites/hero.png"), "{}", w.message);
        assert!(w.message.contains("sprites/bat.png"), "{}", w.message);
        assert!(w.message.contains("OBP"), "{}", w.message);
        assert!(
            !w.message.contains("tile data"),
            "no background: {}",
            w.message
        );
        assert!(w.remedy.contains("dmg line"), "{}", w.remedy);
        assert!(w.remedy.contains("gbcli help images"), "{}", w.remedy);
    }
}

/// An indexed background's index is its shade, so a shade the cascade
/// moves moves its tile data, and the entry of its palette it draws.
#[test]
fn an_indexed_background_alone_warns_too() {
    let (temp, _) = starter("unpinned-bg");
    background(&temp, "sky");
    palettes(&temp, &format!("{EMBER}bg/sky.png = ember\n"));
    let made = convert_into(&temp, BuildTarget::Gbc, Some("alone"));
    coin(&temp);
    convert_into(&temp, BuildTarget::Gbc, Some("with-coin"));
    let tiles = |keep: &str| fs::read(temp.path(&format!("{keep}/bg/sky.2bpp"))).unwrap();
    assert_ne!(
        tiles("alone"),
        tiles("with-coin"),
        "the premise: the tiles move"
    );
    let [w] = &unpinned(&made)[..] else {
        panic!("one warning: {:#?}", made.problems)
    };
    assert!(w.message.contains("bg/sky.png"), "{}", w.message);
    assert!(w.message.contains("tile data"), "{}", w.message);
    assert!(!w.message.contains("OBP"), "no sheet: {}", w.message);
}

#[test]
fn a_dmg_line_pins_the_shades_and_silences_it() {
    let (temp, _) = starter("unpinned-dmg");
    sheet(&temp, "hero");
    background(&temp, "sky");
    coin(&temp);
    palettes(
        &temp,
        &format!("{DMG}{EMBER}sprites/hero.png = ember\nbg/sky.png = ember\n"),
    );
    let made = convert(&temp, BuildTarget::Gbc);
    assert!(unpinned(&made).is_empty(), "{:#?}", made.problems);
}

#[test]
fn color_art_alone_is_not_warned() {
    let (temp, _) = starter("unpinned-color");
    coin(&temp);
    sheet(&temp, "hero");
    palettes(&temp, EMBER);
    let made = convert(&temp, BuildTarget::Gbc);
    assert!(unpinned(&made).is_empty(), "{:#?}", made.problems);
}

/// Without --gbc indexed art builds as DMG art (plan question 10): there
/// is no GBC palette for other art to recolor.
#[test]
fn a_dmg_build_is_not_warned() {
    let (temp, _) = starter("unpinned-dmg-build");
    sheet(&temp, "hero");
    coin(&temp);
    palettes(&temp, &format!("{EMBER}sprites/hero.png = ember\n"));
    let made = convert(&temp, BuildTarget::Dmg);
    assert!(unpinned(&made).is_empty(), "{:#?}", made.problems);
}

/// Art in the DMG grays alone takes the known rule, exact today; a new
/// color would make it stretched, so it warns all the same.
#[test]
fn the_known_rule_still_warns() {
    let (temp, _) = starter("unpinned-known");
    write_png(
        &temp.path("demo/assets/sprites/hero.png"),
        8,
        16,
        |x, _| match x {
            0 => LIGHT,
            1 => DARK,
            _ => CLEAR,
        },
    );
    palettes(&temp, &format!("{EMBER}sprites/hero.png = ember\n"));
    let made = convert(&temp, BuildTarget::Gbc);
    let shades = made.shades.expect("shades");
    assert_eq!(shades.rule, gbcli_protocol::ShadeRule::Known);
    let [w] = &unpinned(&made)[..] else {
        panic!("one warning: {:#?}", made.problems)
    };
    assert!(w.message.contains("known"), "{}", w.message);
}
