//! The project-level `shared_shade` warning with many named palettes (the
//! examples' DX review, item 6; leaf GQFHVi). On the DMG every palette's
//! lightest color is white, so colors of different named palettes sharing
//! a shade is how a DMG version of many palettes works, not a mistake: no
//! warning. Within one image, or across images of one palette, it still
//! warns, one row a shade. Run through `assets`, which shares the build's
//! conversion and needs no rgbasm.

use crate::common;
use crate::pngs;

use std::fs;

use gbcli_build::{AssetsInput, AssetsOptions, assets};
use gbcli_protocol::{BuildTarget, ImageLimit, ImageProblem, ImagesMade};

use common::{Temp, rgbds};
use pngs::*;

const BOTH: [BuildTarget; 2] = [BuildTarget::Dmg, BuildTarget::Gbc];

// All shade 0 beside white and black, and distinct in RGB555.
const NEAR_WHITE: Rgba = [0xF0, 0xF0, 0xF0, 0xFF];
const PALE: Rgba = [0xE0, 0xE0, 0xE0, 0xFF];
const PALER: Rgba = [0xD8, 0xD8, 0xD8, 0xFF];

const PALETTES: &str = "sky: #fff #aaa #555 #000\nsea: #eef #99a #446 #001\n";

/// A tileset under `demo/assets/tiles/`, one solid tile a color.
fn tiles(temp: &Temp, name: &str, colors: &[Rgba]) {
    let n = colors.len() as u32;
    write_png(
        &temp.path(&format!("demo/assets/tiles/{name}.png")),
        8 * n,
        8,
        |x, _| colors[(x / 8) as usize],
    );
}

fn convert(temp: &Temp, target: BuildTarget, assignments: &str) -> ImagesMade {
    fs::write(
        temp.path("demo/assets/palettes.txt"),
        format!("{PALETTES}{assignments}"),
    )
    .unwrap();
    let options = AssetsOptions {
        target,
        ..AssetsOptions::new(AssetsInput::Project(temp.path("demo")), temp.0.clone())
    };
    let made = assets(rgbds().images(), &options).unwrap().images;
    assert!(made.first_error().is_none(), "{:#?}", made.problems);
    made
}

/// The `shared_shade` warnings: the project's (no image), then the images'.
fn shared(made: &ImagesMade) -> (Vec<&ImageProblem>, Vec<&ImageProblem>) {
    made.problems
        .iter()
        .filter(|p| p.limit == ImageLimit::SharedShade)
        .partition(|p| p.image.is_none())
}

/// The shooter's case: each image its own palette, white in one and
/// near-white in another, both shade 0.
#[test]
fn colors_of_different_named_palettes_sharing_a_shade_dont_warn() {
    for target in BOTH {
        let temp = Temp::new(&format!("shared-palettes-{}", target.name()));
        tiles(&temp, "sky", &[WHITE, BLACK]);
        tiles(&temp, "sea", &[NEAR_WHITE, BLACK]);
        let made = convert(&temp, target, "tiles/sky.png = sky\ntiles/sea.png = sea\n");
        let (project, images) = shared(&made);
        assert!(project.is_empty(), "{target:?}: {project:#?}");
        assert!(images.is_empty(), "{target:?}: {images:#?}");
    }
}

/// Two images of one palette: their colors share the shade only across
/// the images, which the project's warning still names.
#[test]
fn colors_of_one_named_palette_across_images_still_warn() {
    for target in BOTH {
        let temp = Temp::new(&format!("shared-one-palette-{}", target.name()));
        tiles(&temp, "sky", &[WHITE, BLACK]);
        tiles(&temp, "sea", &[NEAR_WHITE, BLACK]);
        let made = convert(&temp, target, "tiles/sky.png = sky\ntiles/sea.png = sky\n");
        let (project, _) = shared(&made);
        let [p] = &project[..] else {
            panic!("{target:?}: one warning: {project:#?}")
        };
        assert_eq!(p.count, 2, "{target:?}");
    }
}

/// One image holding white and near-white still warns, whatever the other
/// images' palettes.
#[test]
fn colors_sharing_a_shade_within_one_image_still_warn() {
    for target in BOTH {
        let temp = Temp::new(&format!("shared-within-{}", target.name()));
        tiles(&temp, "sky", &[WHITE, NEAR_WHITE, BLACK]);
        tiles(&temp, "sea", &[PALE, BLACK]);
        let made = convert(&temp, target, "tiles/sky.png = sky\ntiles/sea.png = sea\n");
        let (project, images) = shared(&made);
        assert!(project.is_empty(), "{target:?}: {project:#?}");
        let [p] = &images[..] else {
            panic!("{target:?}: one warning: {images:#?}")
        };
        assert_eq!(
            p.image.as_deref(),
            Some("demo/assets/tiles/sky.png"),
            "{target:?}"
        );
    }
}

/// Two palettes whose images each share shade 0 across images: one row
/// for the shade, listing every group, not one row a palette.
#[test]
fn the_projects_warning_is_one_row_a_shade_over_every_palette() {
    let temp = Temp::new("shared-one-row");
    tiles(&temp, "a", &[WHITE]);
    tiles(&temp, "b", &[NEAR_WHITE, BLACK]);
    tiles(&temp, "c", &[PALE]);
    tiles(&temp, "d", &[PALER, BLACK]);
    let made = convert(
        &temp,
        BuildTarget::Dmg,
        "tiles/a.png = sky\ntiles/b.png = sky\ntiles/c.png = sea\ntiles/d.png = sea\n",
    );
    let (project, _) = shared(&made);
    let [p] = &project[..] else {
        panic!("one warning: {project:#?}")
    };
    assert_eq!(p.count, 4);
    for image in ["a.png", "b.png", "c.png", "d.png"] {
        assert!(p.message.contains(image), "{image} in {}", p.message);
    }
}
