use super::*;
use crate::{Op, parse_request};

fn manifest(json: &str) -> Result<AssetManifest, String> {
    serde_json::from_str(json).map_err(|e| e.to_string())
}

/// `assets` needs no bundle and changes no session, as a build without
/// `restore`: a `serve` with no `.gbcli/` answers it.
#[test]
fn the_assets_op_needs_no_bundle_and_changes_nothing() {
    let request = parse_request(
        r#"{"id":"1","op":"assets","args":{"manifest":{"images":[{"image":"tiles/font.png","file":"art/font.png"}]},"target":"gbc","keep":"converted"}}"#,
    )
    .unwrap();
    let Op::Assets(args) = &request.op else {
        panic!("{:?}", request.op)
    };
    assert_eq!(args.target, BuildTarget::Gbc);
    assert_eq!(args.keep.as_deref(), Some("converted"));
    let image = &args.manifest.as_ref().unwrap().images[0];
    assert_eq!(
        (image.image.as_str(), image.dmg.as_deref()),
        ("tiles/font.png", None)
    );
    assert_eq!(request.op.name(), "assets");
    assert!(!request.op.needs_bundle());
    assert!(!request.op.changes_session() && !request.op.mutates_head());
    // Absent args are a DMG conversion of the working directory's project.
    let bare = parse_request(r#"{"id":"2","op":"assets"}"#).unwrap();
    assert_eq!(bare.op, Op::Assets(AssetsArgs::default()));
    assert_eq!(serde_json::to_string(&AssetsArgs::default()).unwrap(), "{}");
}

#[test]
fn a_manifest_names_its_images_as_a_project_would() {
    let ok = manifest(
        r#"{"images":[{"image":"bg/town.png","file":"a.png","dmg":"b.png"},{"image":"sprites/hero.16x16.png","file":"c.png"}],
            "palettes":{"palettes":[{"name":"forest","colors":[32767,9009,6534,2176]}],"assignments":{"bg/town.png":"forest"}}}"#,
    )
    .unwrap();
    assert_eq!(ok.images.len(), 2);
    assert_eq!(ok.palettes.assignments["bg/town.png"], "forest");
    for (json, says) in [
        (
            r#"{"images":[{"image":"town.png","file":"a.png"}]}"#,
            "isn't a path under assets/",
        ),
        (
            r#"{"images":[{"image":"music/town.png","file":"a.png"}]}"#,
            "isn't a path under assets/",
        ),
        (
            r#"{"images":[{"image":"bg/../town.png","file":"a.png"}]}"#,
            "isn't a path under assets/",
        ),
        (
            r#"{"images":[{"image":"bg/town.dmg.png","file":"a.png"}]}"#,
            "DMG version",
        ),
        (
            r#"{"images":[{"image":"bg/town.png","file":"a.png"},{"image":"bg/town.png","file":"b.png"}]}"#,
            "listed twice",
        ),
        (
            r#"{"images":[{"image":"bg/town.png","file":"a.png"}],"palettes":{"palettes":[{"name":"forest","colors":[0,0,0,0]}],"assignments":{"bg/sky.png":"forest"}}}"#,
            "doesn't list",
        ),
        (
            r#"{"images":[{"image":"bg/town.png","file":"a.png","size":3}]}"#,
            "unknown field",
        ),
    ] {
        let err = manifest(json).unwrap_err();
        assert!(err.contains(says), "{json}: {err}");
    }
}

/// The result is the images a build records, beside the target and
/// rgbgfx's diagnostics; an image whose previews weren't kept names none.
#[test]
fn the_result_reads_back_and_an_image_may_name_no_preview() {
    let json = r#"{"target":"dmg","images":{"images":[{"kind":"tiles","source":"art/font.png","label":"Font","width":2,"height":1,"tiles":1,"tile_map":[{"index":0},{"index":0}]}]},"diagnostics":[]}"#;
    let made: AssetsMade = serde_json::from_str(json).unwrap();
    assert_eq!(made.images.images[0].preview, "");
    assert_eq!(serde_json::to_string(&made).unwrap(), json);
}
