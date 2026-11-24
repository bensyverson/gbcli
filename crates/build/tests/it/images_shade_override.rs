//! A `dmg` override replaces its color sheet in a plain build, so the
//! listing's `art` flag marks only what the target draws (a plain build:
//! the override; --gbc: both), while the shade rule still chooses over
//! every color, the discarded sheet's included (leaf bUYajW).

use crate::common;
use crate::pngs;

use std::fs;

use gbcli_build::{AssetsInput, AssetsOptions, assets};
use gbcli_protocol::{
    AssetManifest, BuildTarget, ColorShade, ImagesMade, ManifestImage, NamedPalettes, Rgb555,
};

use common::Temp;
use pngs::*;

const SHEET_LIGHT: Rgba = [0xC0, 0xC0, 0xC0, 0xFF];
const SHEET_MID: Rgba = [0x80, 0x80, 0x80, 0xFF];
const OVERRIDE: Rgba = [0x20, 0x20, 0x20, 0xFF];

fn c(p: Rgba) -> Rgb555 {
    Rgb555::from_rgb888(p[0], p[1], p[2])
}

/// A color sheet in two grays beside a `dmg` override in one, built for
/// `target`; the listing and the kept tile data.
fn built(name: &str, target: BuildTarget, with_override: bool) -> (ImagesMade, Vec<u8>) {
    let temp = Temp::new(name);
    write_png(&temp.path("hero.png"), 8, 8, |x, _| {
        if x < 4 { SHEET_LIGHT } else { SHEET_MID }
    });
    write_png(&temp.path("hero.dmg.png"), 8, 8, |_, _| OVERRIDE);
    let manifest = AssetManifest {
        images: vec![ManifestImage {
            image: "tiles/hero.png".into(),
            file: "hero.png".into(),
            dmg: with_override.then(|| "hero.dmg.png".into()),
            resident: true,
        }],
        palettes: NamedPalettes::default(),
        objects: None,
        fonts: Vec::new(),
    };
    let mut options = AssetsOptions::new(AssetsInput::Manifest(manifest), temp.0.clone());
    options.target = target;
    options.keep = Some(temp.path("kept"));
    let made = assets(&options).unwrap();
    assert!(
        made.images.first_error().is_none(),
        "{:#?}",
        made.images.problems
    );
    let tiles = fs::read(temp.path("kept/tiles/hero.2bpp")).unwrap();
    (made.images, tiles)
}

fn art(listed: &[ColorShade]) -> Vec<(Rgb555, bool)> {
    listed.iter().map(|l| (l.color, l.art)).collect()
}

#[test]
fn a_plain_build_marks_only_the_overrides_colors_as_art() {
    let (made, _) = built("shade-override-dmg", BuildTarget::Dmg, true);
    // Every color is still listed, with the shade the rule gave it.
    assert_eq!(
        art(&made.color_shades),
        [
            (c(OVERRIDE), true),
            (c(SHEET_MID), false),
            (c(SHEET_LIGHT), false),
        ]
    );
}

#[test]
fn a_gbc_build_marks_both_the_sheets_and_the_overrides_colors_as_art() {
    let (made, _) = built("shade-override-gbc", BuildTarget::Gbc, true);
    assert_eq!(
        art(&made.color_shades),
        [
            (c(OVERRIDE), true),
            (c(SHEET_MID), true),
            (c(SHEET_LIGHT), true),
        ]
    );
}

#[test]
fn an_image_with_no_override_keeps_its_colors_as_art() {
    let (made, _) = built("shade-override-none", BuildTarget::Dmg, false);
    assert_eq!(
        art(&made.color_shades),
        [(c(SHEET_MID), true), (c(SHEET_LIGHT), true)]
    );
}

/// The rule chose over the discarded sheet's colors too: the shades and
/// the tile data are what they were before `art` narrowed.
#[test]
fn narrowing_art_moves_no_shade_and_no_byte() {
    let (made, tiles) = built("shade-override-bytes", BuildTarget::Dmg, true);
    let shades: Vec<u8> = made.color_shades.iter().map(|l| l.shade).collect();
    assert_eq!(shades, SHADES, "{:#?}", made.color_shades);
    assert_eq!(tiles, TILES, "{tiles:02x?}");
}

const SHADES: [u8; 3] = [3, 1, 0];
const TILES: [u8; 16] = [0xFF; 16];
