//! A sheet's shade order in its result (`ImageAsset::obp`), and a manifest's
//! stored sheets (`ManifestImage::resident: false`): converted and checked
//! as sheets, but left out of the tiles objects address at once and of
//! VRAM's, since a client composes them into ROM and loads only what it
//! shows (LoadHigh's sprite parts, its questions 782 and 784).

use crate::common;
use crate::pngs;

use gbcli_build::{AssetsInput, AssetsOptions, assets};
use gbcli_protocol::{AssetManifest, ImageLimit, ManifestImage, NamedPalettes};

use common::Temp;
use pngs::*;

fn image(image: &str, file: &str, resident: bool) -> ManifestImage {
    ManifestImage {
        image: image.into(),
        file: file.into(),
        dmg: None,
        resident,
    }
}

fn manifest(images: Vec<ManifestImage>) -> AssetManifest {
    AssetManifest {
        images,
        palettes: NamedPalettes::default(),
        objects: None,
        fonts: Vec::new(),
    }
}

/// A sheet `units` 8x16 objects wide, each unique even flipped
/// (`numbered_tile` in its top two rows, from `first`, and a mark at the
/// left of its third row), on a clear background.
fn sheet_of(temp: &Temp, file: &str, units: u32, first: u32) {
    write_png(&temp.path(file), 8 * units, 16, |x, y| {
        let mark = (x % 8, y) == (0, 2);
        if mark || (y < 8 && numbered_tile(first + x / 8 + 1, x % 8, y) == BLACK) {
            BLACK
        } else {
            CLEAR
        }
    });
}

/// Each sheet answers its OBP, the shade of each of its indices: one a
/// shade it uses, lightest first; a background answers none.
#[test]
fn each_sheet_answers_its_shade_order_and_a_background_none() {
    let temp = Temp::new("assets-obp");
    // Dark gray and black only: indices 1 and 2.
    write_png(&temp.path("hat.png"), 8, 16, |x, _| match x {
        0..=1 => CLEAR,
        2..=4 => DARK,
        _ => BLACK,
    });
    // Every opaque shade: OBP $E4.
    write_png(&temp.path("body.png"), 8, 16, |x, _| match x {
        0..=1 => CLEAR,
        2..=3 => LIGHT,
        4..=5 => DARK,
        _ => BLACK,
    });
    write_png(&temp.path("town.png"), 8, 8, |_, _| WHITE);
    let made = assets(&AssetsOptions::new(
        AssetsInput::Manifest(manifest(vec![
            image("sprites/hat.8x16.png", "hat.png", true),
            image("sprites/body.8x16.png", "body.png", true),
            image("bg/town.png", "town.png", true),
        ])),
        temp.0.clone(),
    ))
    .unwrap();
    let obp = |label: &str| {
        made.images
            .images
            .iter()
            .find(|i| i.label == label)
            .unwrap_or_else(|| panic!("no {label} in {:#?}", made.images.images))
            .obp
    };
    assert_eq!(
        obp("Hat"),
        Some(0b00_11_10_00),
        "dark gray at 1, black at 2"
    );
    assert_eq!(obp("Body"), Some(0xE4));
    assert_eq!(obp("Town"), None);
}

/// Two sheets past the 256 tiles objects address fail together; stored,
/// either one leaves the count, and the other converts as before.
#[test]
fn a_stored_sheet_leaves_the_object_tile_count() {
    let temp = Temp::new("assets-stored");
    sheet_of(&temp, "a.png", 100, 0);
    sheet_of(&temp, "b.png", 100, 100);
    let both = |resident_b: bool| {
        manifest(vec![
            image("sprites/a.8x16.png", "a.png", true),
            image("sprites/b.8x16.png", "b.png", resident_b),
        ])
    };
    let body = assets(&AssetsOptions::new(
        AssetsInput::Manifest(both(true)),
        temp.0.clone(),
    ))
    .unwrap_err()
    .body();
    let made = body.result.and_then(|r| r.assets).expect("a report");
    assert!(
        made.images
            .problems
            .iter()
            .any(|p| p.limit == ImageLimit::ObjTiles),
        "{:#?}",
        made.images.problems
    );
    let made = assets(&AssetsOptions::new(
        AssetsInput::Manifest(both(false)),
        temp.0.clone(),
    ))
    .unwrap();
    assert!(
        made.images.first_error().is_none(),
        "{:#?}",
        made.images.problems
    );
    let tiles: Vec<u32> = made.images.images.iter().map(|i| i.tiles).collect();
    assert_eq!(tiles, [200, 200], "both converted whole");
}

/// One stored sheet past 256 tiles converts, where a resident one fails.
#[test]
fn a_stored_sheet_past_the_objects_tiles_converts() {
    let temp = Temp::new("assets-stored-big");
    sheet_of(&temp, "big.png", 140, 0);
    let one = |resident: bool| manifest(vec![image("sprites/big.8x16.png", "big.png", resident)]);
    let body = assets(&AssetsOptions::new(
        AssetsInput::Manifest(one(true)),
        temp.0.clone(),
    ))
    .unwrap_err()
    .body();
    let made = body.result.and_then(|r| r.assets).expect("a report");
    assert_eq!(made.images.problems[0].limit, ImageLimit::ObjTiles);
    let made = assets(&AssetsOptions::new(
        AssetsInput::Manifest(one(false)),
        temp.0.clone(),
    ))
    .unwrap();
    assert!(
        made.images.first_error().is_none(),
        "{:#?}",
        made.images.problems
    );
    assert_eq!(made.images.images[0].tiles, 280);
}

/// A stored sheet leaves VRAM's count too: a background beside it is
/// checked against VRAM alone.
#[test]
fn a_stored_sheet_leaves_the_vram_count() {
    let temp = Temp::new("assets-stored-vram");
    sheet_of(&temp, "s.png", 120, 0);
    // 200 unique background tiles: with the sheet's 240, past 384.
    write_png(&temp.path("bg.png"), 8 * 20, 8 * 10, |x, y| {
        numbered_tile((y / 8) * 20 + x / 8 + 1, x % 8, y % 8)
    });
    let with = |resident: bool| {
        manifest(vec![
            image("bg/bg.png", "bg.png", true),
            image("sprites/s.8x16.png", "s.png", resident),
        ])
    };
    let body = assets(&AssetsOptions::new(
        AssetsInput::Manifest(with(true)),
        temp.0.clone(),
    ))
    .unwrap_err()
    .body();
    let made = body.result.and_then(|r| r.assets).expect("a report");
    assert!(
        made.images
            .problems
            .iter()
            .any(|p| p.limit == ImageLimit::VramTiles),
        "{:#?}",
        made.images.problems
    );
    let made = assets(&AssetsOptions::new(
        AssetsInput::Manifest(with(false)),
        temp.0.clone(),
    ))
    .unwrap();
    assert!(
        made.images.first_error().is_none(),
        "{:#?}",
        made.images.problems
    );
}

/// A stored sheet is still checked as a sheet: a frame past OAM's 40
/// objects is refused whether or not it is resident.
#[test]
fn a_stored_sheet_is_still_checked_as_a_sheet() {
    let temp = Temp::new("assets-stored-checked");
    // One 48x112 frame: 42 objects, each drawn.
    write_png(&temp.path("wide.png"), 48, 112, |x, y| {
        if (x, y) == (0, 0) { CLEAR } else { BLACK }
    });
    let made = assets(&AssetsOptions::new(
        AssetsInput::Manifest(manifest(vec![image("sprites/wide.png", "wide.png", false)])),
        temp.0.clone(),
    ))
    .unwrap_err()
    .body()
    .result
    .and_then(|r| r.assets)
    .expect("a report");
    assert!(
        made.images
            .problems
            .iter()
            .any(|p| p.limit == ImageLimit::ObjTotal),
        "{:#?}",
        made.images.problems
    );
}

/// Only a sheet can be stored: a background or tileset marked so is
/// refused by the manifest's check.
#[test]
fn only_a_sheet_can_be_stored() {
    for path in ["bg/a.png", "tiles/a.png"] {
        let m = manifest(vec![image(path, "a.png", false)]);
        let why = m.check().expect_err(path);
        assert!(why.contains("resident"), "{why}");
    }
    assert!(
        manifest(vec![image("sprites/a.png", "a.png", false)])
            .check()
            .is_ok()
    );
}
