//! What `pixels` writes when it can't write everything, and a PNG too big
//! for pixel text that asks for no text (review last15 of c38cd29).

use std::fs;

use gbcli_build::pixels;
use gbcli_pixels::{MAX_SIDE, Raster, encode_png};
use gbcli_protocol::PixelsArgs;

use crate::common::Temp;

fn png(width: u32, height: u32) -> Vec<u8> {
    encode_png(&Raster {
        width,
        height,
        rgba: vec![[0xFF, 0xFF, 0xFF, 0xFF]; (width * height) as usize],
    })
}

/// A PNG past pixel text's largest side is still a PNG: asking only for a
/// preview (or a 1:1 PNG, or just the counts) writes no text, so nothing
/// the parser would refuse is made.
#[test]
fn a_big_png_still_previews_when_no_text_is_asked_for() {
    let temp = Temp::new("pixels-out-big");
    fs::write(temp.path("level.png"), png(MAX_SIDE + 8, 8)).unwrap();
    let args = PixelsArgs {
        file: "level.png".into(),
        preview: Some("level.preview.png".into()),
        ..PixelsArgs::default()
    };
    let made = pixels(&temp.0, &args);
    assert!(made.is_ok(), "{made:?}");
    assert!(temp.path("level.preview.png").is_file());
}

/// An output whose path is a directory is refused before anything is
/// renamed into place, so no earlier output is left written.
#[test]
fn an_output_that_cant_be_renamed_leaves_no_other_output_written() {
    let temp = Temp::new("pixels-out-rename");
    fs::write(temp.path("tiny.png"), png(8, 8)).unwrap();
    fs::create_dir_all(temp.path("out.png/inside")).unwrap();
    let args = PixelsArgs {
        file: "tiny.png".into(),
        text: Some("tiny.pixels".into()),
        png: Some("out.png".into()),
        ..PixelsArgs::default()
    };
    let made = pixels(&temp.0, &args);
    assert!(made.is_err(), "{made:?}");
    let left: Vec<String> = fs::read_dir(&temp.0)
        .unwrap()
        .map(|e| e.unwrap().file_name().to_string_lossy().into_owned())
        .collect();
    assert!(
        !temp.path("tiny.pixels").exists(),
        "a failed run wrote tiny.pixels: {left:?}"
    );
    assert!(
        left.iter().all(|n| !n.ends_with(".gbcli-tmp")),
        "temp files left: {left:?}"
    );
}

/// Two outputs naming one path: the last one is written, as before
/// c38cd29, never a failure after writing the file.
#[test]
fn two_outputs_of_one_path_do_not_fail_after_writing_it() {
    let temp = Temp::new("pixels-out-same");
    fs::write(temp.path("tiny.png"), png(8, 8)).unwrap();
    let args = PixelsArgs {
        file: "tiny.png".into(),
        png: Some("same.png".into()),
        preview: Some("same.png".into()),
        ..PixelsArgs::default()
    };
    let made = pixels(&temp.0, &args).unwrap_or_else(|e| panic!("{}", e.message));
    assert_eq!(made.wrote.len(), 1, "{:?}", made.wrote);
    assert_eq!(
        made.wrote[0].kind,
        gbcli_protocol::PixelsOutputKind::Preview
    );
    let left: Vec<String> = fs::read_dir(&temp.0)
        .unwrap()
        .map(|e| e.unwrap().file_name().to_string_lossy().into_owned())
        .collect();
    assert!(left.iter().all(|n| !n.ends_with(".gbcli-tmp")), "{left:?}");
}
