//! Shade pins (LoadHigh's question 993): a color pinned to a DMG shade, in
//! `assets/palettes.txt` (`#aaaaaa = 3`) or a client's `NamedPalettes`,
//! takes that shade in every image and palette, while the automatic rule
//! still decides every other color; `ImagesMade.color_shades` answers each
//! color's shade and what decided it.

use crate::common;
use crate::pngs;

use std::fs;

use gbcli_build::{AssetsInput, AssetsOptions, assets};
use gbcli_protocol::{
    AssetManifest, ColorShade, DmgPalette, GbcPalette, ManifestImage, NamedPalette, NamedPalettes,
    Rgb555, ShadePin, ShadeRule, ShadeSource,
};

use common::Temp;
use pngs::*;

fn c(p: Rgba) -> Rgb555 {
    Rgb555::from_rgb888(p[0], p[1], p[2])
}

/// The DMG grays, white to black: `$E4` unpinned.
fn grays() -> NamedPalette {
    NamedPalette {
        name: "grays".into(),
        colors: GbcPalette([WHITE, LIGHT, DARK, BLACK].map(c)),
    }
}

/// White, then light gray: the known DMG grays' rule, light gray shade 1
/// unpinned.
fn tiles(temp: &Temp, path: &str) {
    write_png(
        &temp.path(path),
        16,
        8,
        |x, _| {
            if x < 8 { WHITE } else { LIGHT }
        },
    );
}

/// What a light gray pinned to 3 makes of the art, the palette and the
/// list: the pinned color's tile is drawn in index 3, the palette's byte
/// carries the pin, the rule is still known, and the colors nothing
/// pinned are listed under it.
fn assert_pinned_light_gray(made: &gbcli_protocol::ImagesMade, kept: &std::path::Path) {
    assert!(made.first_error().is_none(), "{:#?}", made.problems);
    assert_eq!(made.shades.map(|s| s.rule), Some(ShadeRule::Known));
    assert_eq!(
        made.dmg_palettes,
        [DmgPalette {
            name: "grays".into(),
            byte: 0b11_10_11_00,
        }]
    );
    let shade = |color, shade, rule| ColorShade { color, shade, rule };
    assert_eq!(
        made.color_shades,
        [
            shade(c(BLACK), 3, ShadeSource::Known),
            shade(c(DARK), 2, ShadeSource::Known),
            shade(c(LIGHT), 3, ShadeSource::Pinned),
            shade(c(WHITE), 0, ShadeSource::Known),
        ]
    );
    let data = fs::read(kept.join("tiles/art.2bpp")).unwrap();
    assert_eq!(&data[16..32], &[0xFF; 16], "{data:02x?}");
    assert_eq!(&data[..16], &[0; 16], "{data:02x?}");
}

#[test]
fn a_pin_in_the_palette_file_shades_its_color_everywhere() {
    let temp = Temp::new("shade-pins-file");
    tiles(&temp, "demo/assets/tiles/art.png");
    fs::write(
        temp.path("demo/assets/palettes.txt"),
        "grays: #fff #aaa #555 #000\n#aaaaaa = 3   # pinned dark\n",
    )
    .unwrap();
    let mut options = AssetsOptions::new(AssetsInput::Project(temp.path("demo")), temp.0.clone());
    options.keep = Some(temp.path("kept"));
    let made = assets(&options).unwrap();
    assert_pinned_light_gray(&made.images, &temp.path("kept"));
}

#[test]
fn a_pin_sent_in_named_palettes_shades_its_color_everywhere() {
    let temp = Temp::new("shade-pins-wire");
    tiles(&temp, "art.png");
    let palettes = NamedPalettes {
        palettes: vec![grays()],
        pins: vec![ShadePin {
            color: c(LIGHT),
            shade: 3,
        }],
        ..NamedPalettes::default()
    };
    let manifest = AssetManifest {
        images: vec![ManifestImage {
            image: "tiles/art.png".into(),
            file: "art.png".into(),
            dmg: None,
            resident: true,
        }],
        palettes,
        objects: None,
        fonts: Vec::new(),
    };
    let mut options = AssetsOptions::new(AssetsInput::Manifest(manifest), temp.0.clone());
    options.keep = Some(temp.path("kept"));
    let made = assets(&options).unwrap();
    assert_pinned_light_gray(&made.images, &temp.path("kept"));
}

/// With no images, the palettes' colors are still listed, a pinned one
/// with its pin, as their bytes are answered.
#[test]
fn with_no_images_the_palettes_colors_are_listed_with_their_pins() {
    let temp = Temp::new("shade-pins-no-images");
    let manifest = AssetManifest {
        images: Vec::new(),
        palettes: NamedPalettes {
            palettes: vec![grays()],
            pins: vec![ShadePin {
                color: c(WHITE),
                shade: 1,
            }],
            ..NamedPalettes::default()
        },
        objects: None,
        fonts: Vec::new(),
    };
    let made = assets(&AssetsOptions::new(
        AssetsInput::Manifest(manifest),
        temp.0.clone(),
    ))
    .unwrap();
    assert_eq!(made.images.dmg_palettes[0].byte, 0b11_10_01_01);
    let rules: Vec<(u8, ShadeSource)> = made
        .images
        .color_shades
        .iter()
        .map(|s| (s.shade, s.rule))
        .collect();
    assert_eq!(
        rules,
        [
            (3, ShadeSource::Nearest),
            (2, ShadeSource::Nearest),
            (1, ShadeSource::Nearest),
            (1, ShadeSource::Pinned),
        ]
    );
}
