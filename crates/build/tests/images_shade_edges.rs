//! The edges of the project-wide DMG shades (Ben's rulings, 2026-11-07):
//! shared shades across images, the merged copy rgbgfx reads, a colorless
//! project, and `assets/palette.txt`'s encodings and comments. From the
//! review of the shade choice (leaf GTXXyr).

mod common;
mod pngs;

use std::fs;
use std::path::Path;

use gbcli_build::{BuildError, build};
use gbcli_protocol::{BuildMade, ImageLimit, KnownPalette, Severity, ShadeRule};

use common::{rgbds, starter};
use pngs::*;

const NEAR_BLACK: Rgba = [0x10, 0x10, 0x10, 0xFF];

fn merged_files(dir: &Path) -> Vec<String> {
    let mut found = Vec::new();
    for kind in ["bg", "sprites"] {
        let Ok(entries) = fs::read_dir(dir.join(kind)) else {
            continue;
        };
        for e in entries.flatten() {
            let name = e.file_name().to_string_lossy().to_string();
            if name.contains("merged") {
                found.push(format!("{kind}/{name}"));
            }
        }
    }
    found
}

/// Fake rgbasm/rgblink/rgbfix running the real ones, and an rgbgfx running `body`.
fn tools_with(temp: &common::Temp, rgbgfx: &str) -> gbcli_build::Toolchain {
    use gbcli_protocol::Tool;
    let scripts = [
        ("rgbasm", common::real(Tool::Rgbasm, "")),
        ("rgblink", common::real(Tool::Rgblink, "")),
        ("rgbfix", common::real(Tool::Rgbfix, "")),
        ("rgbgfx", rgbgfx.to_string()),
    ];
    let scripts: Vec<(&str, &str)> = scripts.iter().map(|(n, b)| (*n, b.as_str())).collect();
    let path = common::fake_tools(temp, &scripts);
    gbcli_build::Toolchain::discover(Some(&path), &temp.0).unwrap()
}

fn shared_bg(temp: &common::Temp) {
    // White, black and near-black: near-black shares black's shade.
    write_png(&temp.path("demo/assets/bg/cave.png"), 24, 8, |x, _| {
        [WHITE, BLACK, NEAR_BLACK][(x / 8) as usize]
    });
}

#[test]
fn the_merged_copy_is_removed_after_a_build() {
    let (temp, options) = starter("edge-merged-ok");
    shared_bg(&temp);
    let made = build(&rgbds(), &options).unwrap();
    assert!(
        !problems(&made, ImageLimit::SharedShade).is_empty(),
        "the image must share a shade for this test"
    );
    assert_eq!(
        merged_files(&temp.path("demo/build/images")),
        Vec::<String>::new()
    );
}

#[test]
fn the_merged_copy_is_removed_when_rgbgfx_fails() {
    let (temp, options) = starter("edge-merged-fail");
    shared_bg(&temp);
    let tools = tools_with(&temp, "echo 'FATAL: out of cheese' >&2; exit 1");
    match build(&tools, &options) {
        Err(BuildError::Failed { .. }) => {}
        other => panic!("expected failure, got {other:?}"),
    }
    assert_eq!(
        merged_files(&temp.path("demo/build/images")),
        Vec::<String>::new()
    );
}

/// rgbgfx reads the merged copy, but the user drew the source: a
/// diagnostic naming its input names the source.
#[test]
fn an_rgbgfx_diagnostic_names_the_source_not_the_merged_copy() {
    let (temp, options) = starter("edge-merged-diag");
    shared_bg(&temp);
    let tools = tools_with(
        &temp,
        "for a; do last=$a; done; echo \"error: Failed to open input image (\\\"$last\\\")\" >&2; exit 1",
    );
    let made = match build(&tools, &options) {
        Err(BuildError::Failed { made, .. }) => *made,
        other => panic!("expected failure, got {other:?}"),
    };
    let d = &made.diagnostics[0];
    assert_eq!(d.file.as_deref(), Some("demo/assets/bg/cave.png"));
    assert!(
        !d.message.contains("merged"),
        "the transient file reaches the user: {}",
        d.message
    );
    assert!(
        d.message.contains("demo/assets/bg/cave.png"),
        "the source is named instead: {}",
        d.message
    );
}

/// The merged copy rgbgfx reads keeps transparency, and merges only the
/// shared color.
#[test]
fn the_merged_sheet_copy_keeps_alpha_and_merges_only_the_shared_color() {
    let (temp, options) = starter("edge-merged-alpha");
    // Background spans white..black so the stretch is the identity.
    write_png(&temp.path("demo/assets/bg/sky.png"), 16, 8, |x, _| {
        if x < 8 { WHITE } else { BLACK }
    });
    // Sheet: col 0 black (most), col 1 near-black, col 2 white, rest clear
    // with a junk RGB.
    write_png(
        &temp.path("demo/assets/sprites/bug.png"),
        8,
        16,
        |x, y| match x {
            0 => BLACK,
            1 if y < 8 => BLACK,
            1 => NEAR_BLACK,
            2 => WHITE,
            _ => [0x12, 0x34, 0x56, 0x00],
        },
    );
    let seen = temp.path("seen");
    fs::create_dir_all(&seen).unwrap();
    let real = rgbds().rgbgfx().unwrap().display().to_string();
    let body = format!(
        "for a; do last=$a; done; cp \"$last\" '{}/'\"$(basename \"$last\")\"; exec '{real}' \"$@\"",
        seen.display()
    );
    let tools = tools_with(&temp, &body);
    let made = build(&tools, &options).unwrap();
    assert!(!problems(&made, ImageLimit::SharedShade).is_empty());
    let copy = seen.join("bug.merged.png");
    assert!(copy.exists(), "rgbgfx read a merged copy");
    // Read it as RGBA.
    let mut decoder = png::Decoder::new(fs::File::open(&copy).unwrap());
    decoder.set_transformations(png::Transformations::EXPAND);
    let mut reader = decoder.read_info().unwrap();
    let mut buf = vec![0; reader.output_buffer_size()];
    let info = reader.next_frame(&mut buf).unwrap();
    assert_eq!(info.color_type, png::ColorType::Rgba);
    let px = |x: usize, y: usize| -> [u8; 4] {
        let i = (y * 8 + x) * 4;
        [buf[i], buf[i + 1], buf[i + 2], buf[i + 3]]
    };
    assert_eq!(px(1, 12), BLACK, "near-black merged into black");
    assert_eq!(px(2, 3), WHITE);
    assert_eq!(px(5, 5), [0x12, 0x34, 0x56, 0x00], "transparent kept as is");
    // And the ROM data: near-black's pixel is black's index.
    let data = fs::read(temp.path("demo/build/images/sprites/bug.2bpp")).unwrap();
    let bottom = &data[16..32];
    assert_eq!(index_at(bottom, 4, 0), index_at(bottom, 4, 1));
    assert_eq!(index_at(bottom, 4, 3), 0);
}

/// A sheet whose colors take shades 1 and 3 only: indices 1, 2 lightest
/// first, OBP mapping 1 -> 1 and 2 -> 3.
#[test]
fn a_sheet_using_shades_1_and_3_only_maps_two_indices_through_obp() {
    let (temp, options) = starter("edge-sheet-13");
    write_png(
        &temp.path("demo/assets/sprites/bug.png"),
        8,
        16,
        |x, _| match x {
            0 => LIGHT,
            1 => BLACK,
            _ => CLEAR,
        },
    );
    let made = build(&rgbds(), &options).unwrap();
    let s = made.images.as_ref().unwrap().shades.unwrap();
    assert_eq!(s.palette, Some(KnownPalette::DmgGrays));
    let data = fs::read(temp.path("demo/build/images/sprites/bug.2bpp")).unwrap();
    let inc = fs::read_to_string(temp.path("demo/build/images/images.inc")).unwrap();
    let obp = binary_constant(&inc, "BUG_OBP");
    assert_eq!((index_at(&data, 0, 0), index_at(&data, 0, 1)), (1, 2));
    let shade = |x| (obp >> (2 * index_at(&data, 0, x))) & 3;
    assert_eq!((shade(0), shade(1)), (1, 3), "{obp:08b}");
}

/// Transparent pixels with junk RGB don't stop a known palette.
#[test]
fn transparent_pixels_with_junk_rgb_are_not_a_project_color() {
    let (temp, options) = starter("edge-clear-junk");
    write_png(
        &temp.path("demo/assets/sprites/bug.png"),
        8,
        16,
        |x, _| match x {
            0 => LIGHT,
            1 => BLACK,
            _ => [0x12, 0x34, 0x56, 0x00],
        },
    );
    let made = build(&rgbds(), &options).unwrap();
    let s = made.images.as_ref().unwrap().shades.unwrap();
    assert_eq!(s.rule, ShadeRule::Known);
}

/// A project whose only image is a fully transparent sheet has no colors:
/// no palette can be said to hold them, so the rule is the plain nearest.
#[test]
fn a_project_without_colors_claims_no_known_palette() {
    let (temp, options) = starter("edge-no-colors");
    write_png(
        &temp.path("demo/assets/sprites/ghost.png"),
        8,
        16,
        |_, _| CLEAR,
    );
    let made = build(&rgbds(), &options).unwrap();
    let s = made.images.as_ref().unwrap().shades.unwrap();
    assert_eq!((s.rule, s.palette), (ShadeRule::Nearest, None), "{s:?}");
}

/// Two colors in different images that the project gives one shade draw
/// alike on screen (BGP and OBP both identity here), though no image holds
/// both: one project-level shared_shade, without an image, names them and
/// the images each is in.
#[test]
fn colors_sharing_a_shade_only_across_images_warn_for_the_project() {
    let (temp, options) = starter("edge-cross-image");
    write_png(&temp.path("demo/assets/bg/sky.png"), 16, 8, |x, _| {
        if x < 8 { WHITE } else { BLACK }
    });
    write_png(&temp.path("demo/assets/sprites/bug.png"), 8, 16, |x, _| {
        if x < 4 { NEAR_BLACK } else { CLEAR }
    });
    let made = build(&rgbds(), &options).unwrap();
    let inc = fs::read_to_string(temp.path("demo/build/images/images.inc")).unwrap();
    let obp = binary_constant(&inc, "BUG_OBP");
    let data = fs::read(temp.path("demo/build/images/sprites/bug.2bpp")).unwrap();
    assert_eq!(
        (obp >> (2 * index_at(&data, 0, 0))) & 3,
        3,
        "near-black drawn black"
    );
    let shared = problems(&made, ImageLimit::SharedShade);
    let [w] = &shared[..] else {
        panic!("one project-level shared_shade: {shared:#?}");
    };
    assert_eq!(w.image, None, "{w:?}");
    assert_eq!(w.severity, Severity::Warning);
    assert_eq!((w.count, w.max), (2, 1));
    assert!(w.tiles.is_empty(), "{w:?}");
    for said in [
        "#000000 (demo/assets/bg/sky.png)",
        "#101010 (demo/assets/sprites/bug.png)",
        "shade 3 (black)",
    ] {
        assert!(w.message.contains(said), "{said:?} in {}", w.message);
    }
}

/// Colors that share a shade within one image keep that image's warning
/// and add no project-level one, even when another image holds one of them.
#[test]
fn colors_sharing_a_shade_within_an_image_warn_only_for_that_image() {
    let (temp, options) = starter("edge-within-image");
    shared_bg(&temp);
    write_png(&temp.path("demo/assets/sprites/bug.png"), 8, 16, |x, _| {
        if x < 4 { NEAR_BLACK } else { CLEAR }
    });
    let made = build(&rgbds(), &options).unwrap();
    let shared = problems(&made, ImageLimit::SharedShade);
    let images: Vec<_> = shared.iter().map(|p| p.image.clone()).collect();
    assert_eq!(images, [Some("demo/assets/bg/cave.png".to_string())]);
}

fn palette_build(name: &str, text: &[u8]) -> (common::Temp, Result<BuildMade, BuildError>) {
    let (temp, options) = starter(name);
    fs::write(temp.path("demo/assets/palette.txt"), text).unwrap();
    write_png(&temp.path("demo/assets/bg/sky.png"), 8, 8, |x, _| {
        [WHITE, LIGHT, DARK, BLACK][(x / 2) as usize]
    });
    let r = build(&rgbds(), &options);
    (temp, r)
}

#[test]
fn a_palette_file_with_crlf_is_read() {
    let (_t, r) = palette_build(
        "edge-pal-crlf",
        b"# lightest\r\n#ffffff\r\n#aaaaaa # c\r\n#555555\r\n#000000\r\n",
    );
    let made = r.expect("a CRLF palette builds");
    assert_eq!(
        made.images.unwrap().shades.unwrap().rule,
        ShadeRule::Declared
    );
}

/// Windows editors (Notepad before 2019) write a UTF-8 BOM.
#[test]
fn a_palette_file_with_a_bom_is_read() {
    let (_t, r) = palette_build(
        "edge-pal-bom",
        b"\xEF\xBB\xBF#ffffff\n#aaaaaa\n#555555\n#000000\n",
    );
    match r {
        Ok(made) => assert_eq!(
            made.images.unwrap().shades.unwrap().rule,
            ShadeRule::Declared
        ),
        Err(BuildError::Failed { made, .. }) => {
            let p = problem(&made, ImageLimit::PaletteFile);
            panic!(
                "a BOM'd palette is refused: line {:?}: {}",
                p.line, p.message
            );
        }
        Err(e) => panic!("{e:?}"),
    }
}

#[test]
fn a_palette_file_not_in_utf8_is_refused_as_a_whole() {
    let (_t, r) = palette_build(
        "edge-pal-latin1",
        b"#ffffff\n#aaaaaa\xA0\n#555555\n#000000\n",
    );
    let made = match r {
        Err(BuildError::Failed { made, .. }) => *made,
        other => panic!("expected refusal, got {other:?}"),
    };
    let p = problem(&made, ImageLimit::PaletteFile);
    assert_eq!(p.line, None);
    assert!(p.message.contains("UTF-8"), "{}", p.message);
    assert!(made.images.unwrap().shades.is_none());
}

/// `#` then a tab is a comment, as `#` then a space is.
#[test]
fn a_hash_then_a_tab_starts_a_comment() {
    let (_t, r) = palette_build(
        "edge-pal-tab",
        b"#\tlightest first\n#ffffff\n#aaaaaa\n#555555\n#000000\n",
    );
    match r {
        Ok(_) => {}
        Err(BuildError::Failed { made, .. }) => {
            let p = problem(&made, ImageLimit::PaletteFile);
            panic!(
                "'#<tab>comment' is refused: line {:?}: {}",
                p.line, p.message
            );
        }
        Err(e) => panic!("{e:?}"),
    }
}

/// A palette.txt that is a directory: refused, without a panic, and said
/// to be a directory rather than text in the wrong encoding.
#[test]
fn a_palette_path_that_is_a_directory_is_refused_as_one() {
    let (temp, options) = starter("edge-pal-dir");
    fs::create_dir_all(temp.path("demo/assets/palette.txt")).unwrap();
    write_png(&temp.path("demo/assets/bg/sky.png"), 8, 8, |_, _| WHITE);
    let made = match build(&rgbds(), &options) {
        Err(BuildError::Failed { made, .. }) => *made,
        other => panic!("expected refusal, got {other:?}"),
    };
    let p = problem(&made, ImageLimit::PaletteFile);
    assert!(p.message.contains("directory"), "{}", p.message);
    assert!(!p.message.contains("UTF-8"), "{}", p.message);
}

/// A malformed palette.txt in a project with no images still stops the build.
#[test]
fn a_malformed_palette_is_refused_without_images() {
    let (temp, options) = starter("edge-pal-noimg");
    fs::write(temp.path("demo/assets/palette.txt"), "garbage\n").unwrap();
    let made = match build(&rgbds(), &options) {
        Err(BuildError::Failed { made, .. }) => *made,
        other => panic!("a malformed assets/palette.txt passes without images: {other:?}"),
    };
    let p = problem(&made, ImageLimit::PaletteFile);
    assert_eq!(p.line, Some(1));
}

/// A well-formed palette.txt in a project with no images changes nothing.
#[test]
fn a_good_palette_without_images_builds_as_before() {
    let (temp, options) = starter("edge-pal-noimg-ok");
    fs::write(
        temp.path("demo/assets/palette.txt"),
        "#ffffff\n#aaaaaa\n#555555\n#000000\n",
    )
    .unwrap();
    let made = build(&rgbds(), &options).unwrap();
    assert!(made.images.is_none(), "{:?}", made.images);
}

/// The stretch over a project: near-black beside white only in another
/// image is black; one image's shades depend on the other's colors.
#[test]
fn the_stretch_warning_names_each_image_once() {
    let (temp, options) = starter("edge-stretch-two");
    // Project: black and #101010 only, in two images.
    write_png(&temp.path("demo/assets/bg/a.png"), 16, 8, |x, _| {
        if x < 8 { BLACK } else { NEAR_BLACK }
    });
    write_png(&temp.path("demo/assets/bg/b.png"), 8, 8, |_, _| NEAR_BLACK);
    let made = build(&rgbds(), &options).unwrap();
    let w = problems(&made, ImageLimit::ShadeStretched);
    let images: Vec<_> = w.iter().map(|p| p.image.clone().unwrap()).collect();
    assert_eq!(images, ["demo/assets/bg/a.png", "demo/assets/bg/b.png"]);
}

/// Black and near-black together in cave, a third near-black alone in bug,
/// all one shade (review of d0e891a): cave's own warning names the pair, so
/// the project-level one lists cave's colors as one group and bug's as
/// another, counts the groups, and says no image holds two groups together.
#[test]
fn the_project_warning_groups_colors_an_image_already_names_together() {
    let (temp, options) = starter("edge-across-overlap");
    let nb2: Rgba = [0x20, 0x20, 0x20, 0xFF];
    write_png(&temp.path("demo/assets/bg/cave.png"), 24, 8, |x, _| {
        [WHITE, BLACK, NEAR_BLACK][(x / 8) as usize]
    });
    write_png(&temp.path("demo/assets/sprites/bug.png"), 8, 16, |x, _| {
        if x < 4 { nb2 } else { CLEAR }
    });
    let made = build(&rgbds(), &options).unwrap();
    let shared = problems(&made, ImageLimit::SharedShade);
    let images: Vec<_> = shared.iter().map(|p| p.image.clone()).collect();
    assert_eq!(
        images,
        [Some("demo/assets/bg/cave.png".to_string()), None],
        "{shared:#?}"
    );
    let w = &shared[1];
    assert_eq!((w.count, w.max), (2, 1), "{w:?}");
    assert!(
        w.message.contains(
            "#000000/#101010 (demo/assets/bg/cave.png) and #202020 (demo/assets/sprites/bug.png)"
        ),
        "{}",
        w.message
    );
    assert!(
        w.message
            .contains("no image holds colors from two of these groups"),
        "{}",
        w.message
    );
}
