//! Every PNG the example games draw goes PNG to text to PNG pixel for
//! pixel, in rows and in tiles.

use std::path::{Path, PathBuf};

use gbcli_pixels::{
    Layout, Palettes, decode_png, encode_png, frame_size, from_raster, parse_text, to_text,
};

fn pngs(dir: &Path, out: &mut Vec<PathBuf>) {
    let mut entries: Vec<PathBuf> = std::fs::read_dir(dir)
        .unwrap()
        .map(|e| e.unwrap().path())
        .collect();
    entries.sort();
    for path in entries {
        if path.is_dir() {
            pngs(&path, out);
        } else if path.extension().is_some_and(|e| e == "png") {
            out.push(path);
        }
    }
}

#[test]
fn every_example_png_round_trips_through_text_in_both_layouts() {
    let examples = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../examples");
    let mut found = Vec::new();
    for game in std::fs::read_dir(&examples).unwrap() {
        let assets = game.unwrap().path().join("assets");
        if assets.is_dir() {
            pngs(&assets, &mut found);
        }
    }
    assert!(found.len() >= 16, "{found:?}");
    for path in &found {
        let shown = path.strip_prefix(&examples).unwrap().display();
        let original = decode_png(&std::fs::read(path).unwrap())
            .unwrap()
            .normalized();
        let picture = from_raster(&original).unwrap_or_else(|e| panic!("{shown}: {e}"));
        let name = path.file_name().unwrap().to_string_lossy();
        for layout in [
            Layout::Rows {
                frame: frame_size(&name),
            },
            Layout::Tiles,
        ] {
            let text = to_text(&picture, layout).unwrap_or_else(|e| panic!("{shown}: {e}"));
            let back = parse_text(&text)
                .unwrap_or_else(|e| panic!("{shown} {layout:?}: {e}"))
                .raster(&Palettes::default())
                .unwrap();
            assert!(back == original, "{shown} {layout:?} changed");
            let png = decode_png(&encode_png(&back)).unwrap();
            assert!(png == original, "{shown} {layout:?} changed as a PNG");
        }
    }
}
