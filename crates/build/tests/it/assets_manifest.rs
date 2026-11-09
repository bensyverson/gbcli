//! What a manifest's problems name and say (leaf 3MeEHM, from the review of
//! `assets`): the `dmg` file given, the path a missing image resolved to
//! with a remedy that fits a manifest, and palettes a Rust-built manifest
//! never sent over the wire's check.

use crate::common;
use crate::pngs;

use std::fs;

use gbcli_build::{AssetsInput, AssetsOptions, assets};
use gbcli_protocol::{AssetManifest, BuildTarget, ManifestImage, NamedPalettes};

use common::Temp;
use pngs::*;

const RED: Rgba = [0xFF, 0x00, 0x00, 0xFF];

fn manifest_of(file: &str, image: &str, dmg: Option<&str>) -> AssetManifest {
    AssetManifest {
        images: vec![ManifestImage {
            image: image.into(),
            file: file.into(),
            dmg: dmg.map(str::to_string),
        }],
        palettes: NamedPalettes::default(),
        objects: None,
        fonts: Vec::new(),
    }
}

/// A manifest's `dmg` is any file; a problem with it should name that
/// file, not the `<name>.dmg.png` a project would hold beside `file`.
#[test]
fn a_manifest_dmg_versions_problem_names_the_file_given() {
    let temp = Temp::new("manifest-dmgname");
    write_png(&temp.path("art/glyphs.png"), 16, 8, |_, _| RED);
    write_png(&temp.path("dmg/mono.png"), 8, 8, |_, _| WHITE);
    let tool = common::rgbds().images().clone();
    let mut options = AssetsOptions::new(
        AssetsInput::Manifest(manifest_of(
            "art/glyphs.png",
            "tiles/font.png",
            Some("dmg/mono.png"),
        )),
        temp.0.clone(),
    );
    options.target = BuildTarget::Gbc;
    let body = assets(&tool, &options).unwrap_err().body();
    let made = body.result.and_then(|r| r.assets).expect("a report");
    let p = &made.images.problems[0];
    assert_eq!(
        p.image.as_deref(),
        Some("dmg/mono.png"),
        "the problem names {:?}: {}",
        p.image,
        p.message
    );
}

/// A manifest made in Rust skips the wire's `NamedPalettes` check; an
/// assignment to a palette that doesn't exist, which palettes.txt and the
/// wire both refuse, is refused here too.
#[test]
fn a_rust_manifest_assigning_an_unknown_palette_is_refused() {
    let temp = Temp::new("manifest-unknown-palette");
    write_png(&temp.path("a.png"), 8, 8, |_, _| WHITE);
    let mut m = manifest_of("a.png", "bg/a.png", None);
    m.palettes
        .assignments
        .insert("bg/a.png".into(), "nope".into());
    assert!(m.check().is_ok(), "AssetManifest::check passes it");
    assert!(
        m.palettes.check().is_err(),
        "NamedPalettes::check refuses it"
    );
    let mut options = AssetsOptions::new(AssetsInput::Manifest(m), temp.0.clone());
    options.target = BuildTarget::Gbc;
    let tool = common::rgbds().images().clone();
    let body = assets(&tool, &options).unwrap_err().body();
    assert!(body.message.contains("nope"), "{}", body.message);
}

/// A manifest's missing file is named as it resolved, and the remedy is
/// about the manifest's paths, not "save it as a PNG".
#[test]
fn a_manifests_missing_file_is_named_with_a_remedy_about_the_manifest() {
    let temp = Temp::new("manifest-missing");
    let tool = common::rgbds().images().clone();
    let options = AssetsOptions::new(
        AssetsInput::Manifest(manifest_of("art/gone.png", "bg/a.png", None)),
        temp.0.clone(),
    );
    let body = assets(&tool, &options).unwrap_err().body();
    assert!(body.message.contains("art/gone.png"), "{}", body.message);
    assert!(
        !body.remedy.contains("save it as a PNG"),
        "build's remedy: {}",
        body.remedy
    );
    assert!(body.remedy.contains("manifest"), "{}", body.remedy);
}

/// A project's unreadable image keeps build's remedy.
#[test]
fn a_projects_unreadable_image_keeps_the_save_as_png_remedy() {
    let temp = Temp::new("project-unreadable");
    fs::create_dir_all(temp.path("demo/assets/bg")).unwrap();
    fs::write(temp.path("demo/assets/bg/a.png"), b"not a png").unwrap();
    let tool = common::rgbds().images().clone();
    let options = AssetsOptions::new(AssetsInput::Project(temp.path("demo")), temp.0.clone());
    let body = assets(&tool, &options).unwrap_err().body();
    assert!(body.remedy.contains("save it as a PNG"), "{}", body.remedy);
}
