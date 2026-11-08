//! `gbcli assets` and the `assets` op (leaf 7gGMeZ): images converted as
//! `build` converts them, with rgbgfx alone, from a project or a manifest,
//! by a `serve` that has no bundle as by the CLI.

use std::fs;

use crate::common::{Project, golden};
use crate::images::write_png;
use crate::rgbds::rgbds_dir;

const WHITE: [u8; 3] = [0xFF; 3];
const BLACK: [u8; 3] = [0; 3];

/// Four glyph tiles, 0 and 2 alike: three unique tiles.
fn glyphs(p: &Project, rel: &str) {
    write_png(&p.path(rel), 32, 8, |x, y| {
        let lit = match x / 8 {
            0 | 2 => x % 8 == y,
            1 => x % 8 < 2,
            _ => x % 8 >= 6,
        };
        if lit { BLACK } else { WHITE }
    });
}

/// A `PATH` holding rgbgfx and nothing else, so a conversion that ran
/// rgbasm or rgblink would fail.
fn only_rgbgfx(p: &Project) -> String {
    let dir = p.path("only-rgbgfx");
    fs::create_dir_all(&dir).unwrap();
    let rgbgfx = std::path::Path::new(&rgbds_dir()).join("rgbgfx");
    std::os::unix::fs::symlink(rgbgfx, dir.join("rgbgfx")).unwrap();
    dir.display().to_string()
}

const MANIFEST: &str = r#"{"images":[{"image":"tiles/font.png","file":"art/glyphs.png"}],"palettes":{"palettes":[{"name":"forest","colors":[32767,9009,6534,2176]}],"assignments":{"tiles/font.png":"forest"}}}"#;

/// A `serve` with no `.gbcli/` answers `assets` (the bare responder), and
/// with only rgbgfx on `PATH`.
#[test]
fn a_serve_with_no_bundle_converts_a_manifest_with_rgbgfx_alone() {
    let p = Project::new("assets-serve");
    glyphs(&p, "art/glyphs.png");
    let path = only_rgbgfx(&p);
    let line =
        format!(r#"{{"id":"a","op":"assets","args":{{"manifest":{MANIFEST},"target":"gbc"}}}}"#);
    let run = p.gbcli_with(&["serve"], &[("PATH", &path)], Some(&format!("{line}\n")));
    assert_eq!(run.code, 0, "{}", run.stderr);
    let answer: serde_json::Value = serde_json::from_str(run.stdout.trim()).unwrap();
    assert_eq!(answer["ok"], true, "{answer}");
    let font = &answer["result"]["assets"]["images"]["images"][0];
    assert_eq!(font["kind"], "tiles");
    assert_eq!(font["tiles"], 3);
    assert_eq!(font["gbc"]["art"], "indexed");
    assert_eq!(font["tile_map"][2]["index"], 0, "{font}");
    assert!(font.get("preview").is_none(), "no previews kept: {font}");
    assert!(!p.path(".gbcli").exists(), "assets made a bundle");
}

/// The verb is the op: `--json` is the line `serve` answers, from a
/// manifest file, with the data and previews kept under `--keep`.
#[test]
fn the_clis_assets_is_the_serve_op_and_prints_its_rows() {
    let p = Project::new("assets-cli");
    glyphs(&p, "art/glyphs.png");
    fs::write(p.path("font.json"), MANIFEST).unwrap();
    let path = only_rgbgfx(&p);
    let env = [("PATH", path.as_str())];
    let args = [
        "assets",
        "--manifest",
        "font.json",
        "--gbc",
        "--keep",
        "kept",
    ];
    let rows = p.gbcli_with(&args, &env, None);
    assert_eq!(rows.code, 0, "{}", rows.stderr);
    assert!(p.path("kept/tiles/font.preview.png").is_file());
    let json_args = [&args[..], &["--json"]].concat();
    let cli = p.gbcli_with(&json_args, &env, None);
    assert_eq!(cli.code, 0, "{}", cli.stderr);
    let line = format!(
        r#"{{"id":"cli","op":"assets","args":{{"manifest":{MANIFEST},"target":"gbc","keep":"kept"}}}}"#
    );
    let served = p.gbcli_with(&["serve"], &env, Some(&format!("{line}\n")));
    assert_eq!(cli.stdout, served.stdout);
    let mut out = rows.transcript(&args, &p.dir);
    // A project: its own assets/, nothing written to it.
    glyphs(&p, "game/assets/tiles/font.png");
    let project = ["assets", "game"];
    let run = p.gbcli_with(&project, &env, None);
    out += &run.transcript(&project, &p.dir);
    assert_eq!(run.code, 0, "{}", run.stderr);
    assert!(!p.path("game/build").exists(), "nothing written");
    golden("assets.txt", &out);
}

/// A manifest that doesn't read, or beside a project, is the caller's
/// mistake (exit 2), named with what to do.
#[test]
fn a_bad_manifest_and_a_manifest_with_a_project_are_refused() {
    let p = Project::new("assets-refused");
    fs::write(
        p.path("bad.json"),
        r#"{"images":[{"image":"town.png","file":"t.png"}]}"#,
    )
    .unwrap();
    let run = p.gbcli(&["assets", "--manifest", "bad.json"]);
    assert_eq!(run.code, 2, "{}", run.stderr);
    assert!(run.stderr.contains("bad.json"), "{}", run.stderr);
    assert!(
        run.stderr.contains("isn't a path under assets/"),
        "{}",
        run.stderr
    );
    fs::write(p.path("good.json"), MANIFEST).unwrap();
    let both = p.gbcli(&["assets", "game", "--manifest", "good.json"]);
    both.fails(2, "bad_request", "--manifest");
}

fn manifest_naming(file: &str) -> String {
    format!(
        r#"{{"images":[{{"image":"tiles/font.png","file":"{}"}}]}}"#,
        file.replace('\\', "\\\\")
    )
}

/// A manifest's relative `file` resolves against the manifest's own
/// folder, as a manifest file's paths usually do; an absolute one stays.
#[test]
fn a_manifests_relative_paths_resolve_against_its_folder() {
    let p = Project::new("assets-manifest-dir");
    glyphs(&p, "sets/glyphs.png");
    fs::create_dir_all(p.path("sets")).unwrap();
    fs::write(p.path("sets/m.json"), manifest_naming("glyphs.png")).unwrap();
    let path = only_rgbgfx(&p);
    let env = [("PATH", path.as_str())];
    let run = p.gbcli_with(&["assets", "--manifest", "sets/m.json"], &env, None);
    assert_eq!(run.code, 0, "{}", run.stderr);

    let absolute = p.path("sets/glyphs.png").display().to_string();
    fs::create_dir_all(p.path("elsewhere")).unwrap();
    fs::write(p.path("elsewhere/m.json"), manifest_naming(&absolute)).unwrap();
    let run = p.gbcli_with(&["assets", "--manifest", "elsewhere/m.json"], &env, None);
    assert_eq!(run.code, 0, "{}", run.stderr);
}

/// A manifest's missing image names the path as resolved, and the remedy
/// says how a manifest's paths resolve.
#[test]
fn a_manifests_missing_image_names_the_resolved_path() {
    let p = Project::new("assets-manifest-missing");
    fs::create_dir_all(p.path("sets")).unwrap();
    fs::write(p.path("sets/m.json"), manifest_naming("gone.png")).unwrap();
    let path = only_rgbgfx(&p);
    let env = [("PATH", path.as_str())];
    let run = p.gbcli_with(&["assets", "--manifest", "sets/m.json"], &env, None);
    assert_ne!(run.code, 0);
    assert!(run.stderr.contains("sets/gone.png"), "{}", run.stderr);
    assert!(!run.stderr.contains("save it as a PNG"), "{}", run.stderr);
    assert!(run.stderr.contains("manifest's folder"), "{}", run.stderr);
}
