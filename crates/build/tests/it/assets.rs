//! `assets` (leaf 7gGMeZ): images converted as a build converts them,
//! without rgbasm or rgblink, from a project or from a manifest, writing
//! nothing unless asked.

use crate::common;
use crate::pngs;

use std::fs;
use std::path::Path;

use gbcli_build::{AssetsInput, AssetsOptions, BuildOptions, assets, build};
use gbcli_protocol::{
    AssetManifest, BuildTarget, DmgPalette, ErrorCode, ImageArt, ImageKind, ImageLimit, ImagesMade,
    ManifestImage, NamedPalette, NamedPalettes, Rgb555,
};

use common::{Temp, rgbds, starter};
use pngs::*;

const RED: Rgba = [0xFF, 0x00, 0x00, 0xFF];
const BLUE: Rgba = [0x00, 0x00, 0xFF, 0xFF];
const GREEN: Rgba = [0x00, 0xFF, 0x00, 0xFF];

/// A background in color, a color sheet with a mirrored frame, a tileset,
/// and an assigned background, so every kind and both arts are compared.
fn art(temp: &Temp) {
    write_png(&temp.path("demo/assets/bg/town.png"), 16, 8, |x, _| {
        [RED, BLUE][(x / 8) as usize]
    });
    write_png(&temp.path("demo/assets/bg/sky.png"), 8, 8, |x, _| {
        if x < 4 { WHITE } else { BLACK }
    });
    write_png(
        &temp.path("demo/assets/sprites/bat.8x16.png"),
        16,
        16,
        |x, _| match x {
            0..=2 | 13..=15 => GREEN,
            _ => CLEAR,
        },
    );
    write_png(&temp.path("demo/assets/tiles/font.png"), 24, 8, |x, y| {
        if x % 8 == y && x < 16 { BLACK } else { WHITE }
    });
    fs::write(
        temp.path("demo/assets/palettes.txt"),
        "forest: #fff #8c4 #363 #021\nbg/sky.png = forest\n",
    )
    .unwrap();
}

fn project_options(temp: &Temp, target: BuildTarget) -> AssetsOptions {
    AssetsOptions {
        target,
        ..AssetsOptions::new(AssetsInput::Project(temp.path("demo")), temp.0.clone())
    }
}

/// Every file under `dir`, relative to it.
fn files(dir: &Path) -> Vec<String> {
    let mut out = Vec::new();
    let mut stack = vec![dir.to_path_buf()];
    while let Some(d) = stack.pop() {
        for e in fs::read_dir(&d).into_iter().flatten().flatten() {
            let p = e.path();
            if p.is_dir() {
                stack.push(p);
            } else {
                out.push(p.strip_prefix(dir).unwrap().display().to_string());
            }
        }
    }
    out.sort();
    out
}

/// The criterion: for the same project, the images `assets` reports are
/// the ones `build` records, on both targets. Kept in the build's own
/// `build/images/`, they are equal but for `include`, which only a build
/// writes (`images.inc`).
#[test]
fn the_ops_results_equal_the_ones_build_records_for_the_same_project() {
    for target in [BuildTarget::Dmg, BuildTarget::Gbc] {
        let (temp, options) = starter(&format!("assets-equal-{}", target.name()));
        art(&temp);
        let options = BuildOptions { target, ..options };
        let built = build(&rgbds(), &options).unwrap().images.expect("images");
        assert!(built.first_error().is_none(), "{:#?}", built.problems);
        let kinds: Vec<ImageKind> = built.images.iter().map(|i| i.kind).collect();
        assert!(kinds.contains(&ImageKind::Tiles), "{kinds:?}");
        let mut op = project_options(&temp, target);
        op.keep = Some(temp.path("demo/build/images"));
        let made = assets(&op).unwrap();
        assert_eq!(made.target, target);
        assert_eq!(made.images.include, None);
        let expected = ImagesMade {
            include: None,
            ..built.clone()
        };
        assert_eq!(made.images, expected, "{}", target.name());

        // Without `keep`, the same results but the previews, and nothing
        // written to the project.
        fs::remove_dir_all(temp.path("demo/build")).unwrap();
        let before = files(&temp.path("demo"));
        let made = assets(&project_options(&temp, target)).unwrap();
        assert_eq!(files(&temp.path("demo")), before, "nothing written");
        let mut unkept = expected.clone();
        for i in &mut unkept.images {
            i.preview.clear();
            if let Some(g) = &mut i.gbc {
                g.preview.clear();
            }
        }
        assert_eq!(made.images, unkept, "{}", target.name());
    }
}

/// The second criterion: no project, only PNGs, a manifest that names them
/// as a project would, and a `NamedPalettes` struct.
#[test]
fn a_manifest_with_named_palettes_and_a_tileset_converts_without_a_project() {
    let temp = Temp::new("assets-manifest");
    // Four tiles: 0 and 2 alike, 3 is 1 mirrored, which a tileset keeps.
    write_png(&temp.path("art/glyphs.png"), 32, 8, |x, y| {
        let (t, x) = (x / 8, x % 8);
        let lit = match t {
            0 | 2 => x == y,
            1 => x < 2,
            _ => x >= 6,
        };
        if lit { BLACK } else { WHITE }
    });
    let forest = NamedPalette {
        name: "forest".into(),
        colors: gbcli_protocol::GbcPalette(
            [0xFFFFFF, 0x88CC44, 0x336633, 0x002211]
                .map(|c: u32| Rgb555::from_rgb888((c >> 16) as u8, (c >> 8) as u8, c as u8)),
        ),
    };
    let mut palettes = NamedPalettes {
        palettes: vec![forest],
        ..NamedPalettes::default()
    };
    palettes
        .assign("tiles/font.png".into(), "forest".into())
        .unwrap();
    let manifest = AssetManifest {
        images: vec![ManifestImage {
            image: "tiles/font.png".into(),
            file: "art/glyphs.png".into(),
            dmg: None,
        }],
        palettes,
        objects: None,
        fonts: Vec::new(),
    };
    manifest.check().unwrap();
    let mut options = AssetsOptions::new(AssetsInput::Manifest(manifest), temp.0.clone());
    options.target = BuildTarget::Gbc;
    let made = assets(&options).unwrap();
    assert!(
        made.images.first_error().is_none(),
        "{:#?}",
        made.images.problems
    );
    let [font] = &made.images.images[..] else {
        panic!("{:#?}", made.images.images)
    };
    assert_eq!(
        (font.kind, font.source.as_str(), font.label.as_str()),
        (ImageKind::Tiles, "art/glyphs.png", "Font")
    );
    assert_eq!(font.tiles, 3);
    let map: Vec<(u32, u8)> = font
        .tile_map
        .iter()
        .map(|e| {
            let e = e.expect("a tileset has no transparent unit");
            assert!(e.flip.is_none());
            (e.index, e.palette.expect("a GBC slot").get())
        })
        .collect();
    assert_eq!(map, [(0, 0), (1, 0), (0, 0), (2, 0)]);
    let gbc = font.gbc.as_ref().expect("a GBC version");
    assert_eq!(
        gbc.art,
        ImageArt::Indexed {
            palette: "forest".into()
        }
    );
    assert_eq!(gbc.palettes.len(), 1);
    assert_eq!(font.preview, "", "no previews kept");
    assert_eq!(files(&temp.0), ["art/glyphs.png"], "nothing written");

    // With `keep`, the data and previews are kept there, and named.
    options.keep = Some(temp.path("kept"));
    let made = assets(&options).unwrap();
    let font = &made.images.images[0];
    assert_eq!(font.preview, "kept/tiles/font.preview.png");
    assert_eq!(
        gbc_preview(&made.images),
        Some("kept/tiles/font.gbc.preview.png")
    );
    assert_eq!(
        files(&temp.path("kept")),
        [
            "tiles/font.2bpp",
            "tiles/font.gbc.preview.png",
            "tiles/font.preview.png"
        ]
    );
}

/// LoadHigh's ask: each named palette's DMG byte, used by an image or not,
/// and with no images at all.
#[test]
fn every_named_palette_comes_back_as_its_dmg_byte() {
    let color = |c: u32| Rgb555::from_rgb888((c >> 16) as u8, (c >> 8) as u8, c as u8);
    let grays = gbcli_protocol::GbcPalette([0xFFFFFF, 0xAAAAAA, 0x555555, 0].map(color));
    let inverted = gbcli_protocol::GbcPalette([0, 0x555555, 0xAAAAAA, 0xFFFFFF].map(color));
    let palettes = NamedPalettes {
        dmg: Some(grays),
        palettes: vec![
            NamedPalette {
                name: "inverted".into(),
                colors: inverted,
            },
            NamedPalette {
                name: "plain".into(),
                colors: grays,
            },
        ],
        ..NamedPalettes::default()
    };
    let bytes = vec![
        DmgPalette {
            name: "inverted".into(),
            byte: 0x1B,
        },
        DmgPalette {
            name: "plain".into(),
            byte: 0xE4,
        },
    ];

    let temp = Temp::new("assets-dmg-bytes");
    write_png(&temp.path("art/a.png"), 8, 8, |x, _| {
        if x < 4 { BLACK } else { WHITE }
    });
    let with_image = AssetManifest {
        images: vec![ManifestImage {
            image: "tiles/a.png".into(),
            file: "art/a.png".into(),
            dmg: None,
        }],
        palettes: palettes.clone(),
        objects: None,
        fonts: Vec::new(),
    };
    let made = assets(&AssetsOptions::new(
        AssetsInput::Manifest(with_image),
        temp.0.clone(),
    ))
    .unwrap();
    assert_eq!(made.images.dmg_palettes, bytes);

    let no_images = AssetManifest {
        images: Vec::new(),
        palettes,
        objects: None,
        fonts: Vec::new(),
    };
    let made = assets(&AssetsOptions::new(
        AssetsInput::Manifest(no_images),
        temp.0.clone(),
    ))
    .unwrap();
    assert_eq!(made.images.dmg_palettes, bytes);
}

fn gbc_preview(images: &ImagesMade) -> Option<&str> {
    images.images[0].gbc.as_ref().map(|g| g.preview.as_str())
}

/// A manifest's image is checked as a project's is: a problem comes back
/// with the result, as a build's does, and stops the conversion.
#[test]
fn a_manifest_image_that_breaks_a_limit_fails_with_its_problem() {
    let temp = Temp::new("assets-limit");
    write_png(&temp.path("odd.png"), 12, 8, |_, _| WHITE);
    let manifest = AssetManifest {
        images: vec![ManifestImage {
            image: "bg/odd.png".into(),
            file: "odd.png".into(),
            dmg: None,
        }],
        palettes: NamedPalettes::default(),
        objects: None,
        fonts: Vec::new(),
    };
    let options = AssetsOptions::new(AssetsInput::Manifest(manifest), temp.0.clone());
    let body = assets(&options).unwrap_err().body();
    assert_eq!(body.code, ErrorCode::BuildFailed);
    assert!(body.message.contains("odd.png"), "{}", body.message);
    assert!(!body.remedy.is_empty());
    let made = body
        .result
        .and_then(|r| r.assets)
        .expect("the result rides along");
    assert_eq!(made.images.problems[0].limit, ImageLimit::SizeMultiple8);
}

/// A manifest's file that isn't there is the caller's mistake, named.
#[test]
fn a_manifest_file_that_isnt_there_is_a_bad_request_naming_it() {
    let temp = Temp::new("assets-missing");
    let manifest = AssetManifest {
        images: vec![ManifestImage {
            image: "bg/town.png".into(),
            file: "nowhere/town.png".into(),
            dmg: None,
        }],
        palettes: NamedPalettes::default(),
        objects: None,
        fonts: Vec::new(),
    };
    let options = AssetsOptions::new(AssetsInput::Manifest(manifest), temp.0.clone());
    let body = assets(&options).unwrap_err().body();
    assert_eq!(body.code, ErrorCode::BadRequest);
    assert!(
        body.message.contains("nowhere/town.png"),
        "{}",
        body.message
    );
}
