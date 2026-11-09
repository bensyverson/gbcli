//! What image remedies name as the place to pin the shades (leaf zqarpL,
//! from the review of 4381e6a): a project's `dmg` setting is the dmg line
//! of `assets/palettes.txt`; a manifest's is its `palettes.dmg` field, and
//! it has no line. Each case converts the same art twice, once as a
//! project and once as a manifest, through `assets`.

use crate::common;
use crate::pngs;

use std::fs;

use gbcli_build::{AssetsError, AssetsInput, AssetsOptions, BuildError, assets};
use gbcli_protocol::{
    AssetManifest, BuildTarget, GbcPalette, ImageLimit, ImageProblem, ImagesMade, ManifestImage,
    NamedPalette, NamedPalettes, Rgb555,
};

use common::{Temp, rgbds};
use pngs::*;

const NEAR_BLACK: Rgba = [0x10, 0x10, 0x10, 0xFF];
const NEAR_WHITE: Rgba = [0xF0, 0xF0, 0xF0, 0xFF];

/// One image of a case: its path under `assets/`, and its pixels as a row
/// of solid 8x8 tiles.
struct Art {
    image: &'static str,
    tiles: Vec<Rgba>,
}

/// A case: its images, and the palette `ember` assigned to some of them.
struct Case {
    name: &'static str,
    art: Vec<Art>,
    assigned: Vec<&'static str>,
    dmg_version: Option<&'static str>,
    target: BuildTarget,
}

const EMBER: [u32; 4] = [0xFFFFEE, 0xFF8800, 0xAA2200, 0x220000];

fn ember() -> NamedPalette {
    NamedPalette {
        name: "ember".into(),
        colors: GbcPalette(
            EMBER.map(|c| Rgb555::from_rgb888((c >> 16) as u8, (c >> 8) as u8, c as u8)),
        ),
    }
}

fn draw(path: &std::path::Path, tiles: &[Rgba]) {
    let n = tiles.len() as u32;
    write_png(path, 8 * n, 8, |x, _| tiles[(x / 8) as usize]);
}

impl Case {
    /// The case as a project under `demo/`.
    fn project(&self, temp: &Temp) -> AssetsOptions {
        for a in &self.art {
            draw(&temp.path(&format!("demo/assets/{}", a.image)), &a.tiles);
        }
        if let Some(image) = self.dmg_version {
            let dmg = image.replace(".png", ".dmg.png");
            draw(&temp.path(&format!("demo/assets/{dmg}")), &[WHITE]);
        }
        let mut text = String::from("ember: #ffe #f80 #a20 #200\n");
        for a in &self.assigned {
            text += &format!("{a} = ember\n");
        }
        fs::write(temp.path("demo/assets/palettes.txt"), text).unwrap();
        AssetsOptions {
            target: self.target,
            ..AssetsOptions::new(AssetsInput::Project(temp.path("demo")), temp.0.clone())
        }
    }

    /// The case as a manifest, its files under `art/`.
    fn manifest(&self, temp: &Temp) -> AssetsOptions {
        let mut images = Vec::new();
        for a in &self.art {
            let file = format!("art/{}", a.image.replace('/', "-"));
            draw(&temp.path(&file), &a.tiles);
            let dmg = (self.dmg_version == Some(a.image)).then(|| {
                let dmg = file.replace(".png", ".dmg.png");
                draw(&temp.path(&dmg), &[WHITE]);
                dmg
            });
            images.push(ManifestImage {
                image: a.image.into(),
                file,
                dmg,
            });
        }
        let palettes = NamedPalettes {
            dmg: None,
            palettes: vec![ember()],
            assignments: self
                .assigned
                .iter()
                .map(|a| (a.to_string(), "ember".to_string()))
                .collect(),
        };
        let manifest = AssetManifest {
            images,
            palettes,
            objects: None,
            fonts: Vec::new(),
        };
        AssetsOptions {
            target: self.target,
            ..AssetsOptions::new(AssetsInput::Manifest(manifest), temp.0.clone())
        }
    }

    fn converted(&self, options: &AssetsOptions) -> ImagesMade {
        let made = assets(rgbds().images(), options).unwrap().images;
        assert!(made.first_error().is_none(), "{:#?}", made.problems);
        made
    }

    /// The problem `limit` in both conversions: the project's, then the
    /// manifest's.
    fn both(&self, limit: ImageLimit) -> (ImageProblem, ImageProblem) {
        let temp = Temp::new(&format!("remedy-{}", self.name));
        let one = |options: AssetsOptions| {
            let made = self.converted(&options);
            made.problems
                .iter()
                .find(|p| p.limit == limit)
                .cloned()
                .unwrap_or_else(|| panic!("no {limit:?} in {:#?}", made.problems))
        };
        (one(self.project(&temp)), one(self.manifest(&temp)))
    }
}

/// The project's problem names the dmg line of its palettes.txt; the
/// manifest's names `palettes.dmg` and no line.
fn names_each_source(project: &ImageProblem, manifest: &ImageProblem) {
    let text = |p: &ImageProblem| format!("{} {}", p.message, p.remedy);
    assert!(
        project.remedy.contains("dmg line") && project.remedy.contains("assets/palettes.txt"),
        "project: {}",
        project.remedy
    );
    assert!(
        manifest.remedy.contains("palettes.dmg"),
        "manifest: {}",
        manifest.remedy
    );
    for wrong in ["assets/palettes.txt", "dmg line"] {
        assert!(
            !text(manifest).contains(wrong),
            "manifest names {wrong:?}: {}",
            text(manifest)
        );
    }
}

#[test]
fn shades_unpinned_names_each_sources_dmg_setting() {
    let case = Case {
        name: "unpinned",
        art: vec![Art {
            image: "tiles/font.png",
            tiles: vec![WHITE, BLACK],
        }],
        assigned: vec!["tiles/font.png"],
        dmg_version: None,
        target: BuildTarget::Gbc,
    };
    let (project, manifest) = case.both(ImageLimit::ShadesUnpinned);
    names_each_source(&project, &manifest);
}

/// Near-black beside black: the stretch draws it white, two or more shades
/// from its nearest.
#[test]
fn shade_stretched_names_each_sources_dmg_setting() {
    let case = Case {
        name: "stretched",
        art: vec![Art {
            image: "tiles/font.png",
            tiles: vec![NEAR_BLACK, BLACK],
        }],
        assigned: vec![],
        dmg_version: None,
        target: BuildTarget::Dmg,
    };
    let (project, manifest) = case.both(ImageLimit::ShadeStretched);
    names_each_source(&project, &manifest);
}

/// Indexed art holding white and near-white: both shade 0, in one image.
#[test]
fn an_images_shared_shade_names_each_sources_dmg_setting() {
    let case = Case {
        name: "shared",
        art: vec![Art {
            image: "tiles/font.png",
            tiles: vec![WHITE, NEAR_WHITE, BLACK],
        }],
        assigned: vec!["tiles/font.png"],
        dmg_version: None,
        target: BuildTarget::Dmg,
    };
    let (project, manifest) = case.both(ImageLimit::SharedShade);
    assert!(project.image.is_some() && manifest.image.is_some());
    names_each_source(&project, &manifest);
}

/// Two images of one palette, white in one and near-white in the other:
/// they share shade 0 only across the images.
#[test]
fn the_projects_shared_shade_names_each_sources_dmg_setting() {
    let case = Case {
        name: "across",
        art: vec![
            Art {
                image: "tiles/a.png",
                tiles: vec![WHITE],
            },
            Art {
                image: "tiles/b.png",
                tiles: vec![NEAR_WHITE, BLACK],
            },
        ],
        assigned: vec!["tiles/a.png", "tiles/b.png"],
        dmg_version: None,
        target: BuildTarget::Dmg,
    };
    let (project, manifest) = case.both(ImageLimit::SharedShade);
    assert!(project.image.is_none() && manifest.image.is_none());
    names_each_source(&project, &manifest);
}

/// A DMG version beside indexed art: the error says where the image is
/// assigned its palette.
#[test]
fn the_override_beside_indexed_art_error_names_each_sources_assignment() {
    let case = Case {
        name: "override",
        art: vec![Art {
            image: "bg/town.png",
            tiles: vec![WHITE, BLACK],
        }],
        assigned: vec!["bg/town.png"],
        dmg_version: Some("bg/town.png"),
        target: BuildTarget::Gbc,
    };
    let temp = Temp::new("remedy-override");
    let layout = |options: AssetsOptions| match assets(rgbds().images(), &options) {
        Err(AssetsError::Build(BuildError::Layout { message, remedy })) => {
            format!("{message} {remedy}")
        }
        Err(other) => panic!("expected a layout error, got {:?}", other.body()),
        Ok(made) => panic!("expected a layout error, got {:#?}", made.images),
    };
    let project = layout(case.project(&temp));
    let manifest = layout(case.manifest(&temp));
    assert!(
        project.contains("assets/palettes.txt"),
        "project: {project}"
    );
    assert!(
        manifest.contains("palettes.assignments"),
        "manifest: {manifest}"
    );
    assert!(
        !manifest.contains("assets/palettes.txt"),
        "manifest: {manifest}"
    );
}

/// A manifest reads a DMG version only from an image's `dmg` field, never
/// from a file beside its `file` (review-img14 of 372d772): a derived
/// version's remedies name that field, not `art/hero.dmg.png`. Both
/// remedies that offer a hand-drawn DMG version: a sheet's four shades,
/// and (under --gbc) colors sharing a shade in one object.
#[test]
fn a_manifests_derived_remedy_names_its_dmg_field_not_a_file_beside_it() {
    let temp = Temp::new("remedy-manifest-dmg-field");
    write_png(&temp.path("art/hero.png"), 8, 16, |x, _| {
        [WHITE, LIGHT, DARK, BLACK, CLEAR, CLEAR, CLEAR, CLEAR][x as usize]
    });
    // One object, near-black beside black.
    write_png(&temp.path("art/bat.png"), 8, 16, |x, _| {
        [WHITE, BLACK, NEAR_BLACK, CLEAR, CLEAR, CLEAR, CLEAR, CLEAR][x as usize]
    });
    for (image, file, limit, target) in [
        (
            "sprites/hero.png",
            "art/hero.png",
            ImageLimit::DmgPalette,
            BuildTarget::Dmg,
        ),
        (
            "sprites/bat.png",
            "art/bat.png",
            ImageLimit::SharedShade,
            BuildTarget::Gbc,
        ),
    ] {
        let manifest = AssetManifest {
            images: vec![ManifestImage {
                image: image.into(),
                file: file.into(),
                dmg: None,
            }],
            palettes: NamedPalettes::default(),
            objects: None,
            fonts: Vec::new(),
        };
        let options = AssetsOptions {
            target,
            ..AssetsOptions::new(AssetsInput::Manifest(manifest), temp.0.clone())
        };
        let made = match assets(rgbds().images(), &options) {
            Ok(m) => m.images,
            Err(AssetsError::Failed(m)) => m.images,
            Err(e) => panic!("{:?}", e.body()),
        };
        let p = made
            .problems
            .iter()
            .find(|p| p.limit == limit)
            .unwrap_or_else(|| panic!("{image}: {:#?}", made.problems));
        assert!(
            !p.remedy.contains(".dmg.png"),
            "{image}: names a file a manifest never reads: {}",
            p.remedy
        );
        assert!(
            p.remedy.contains(&format!("the `dmg` field of {image}")),
            "{image}: {}",
            p.remedy
        );
    }
}
