//! PNG in and out, and the legend a PNG's colors are given.

use gbcli_pixels::{
    Form, Layout, MADE_SYMBOLS, Raster, Value, decode_png, encode_png, from_raster, is_png, read,
    to_text,
};

fn raster(width: u32, rgba: Vec<[u8; 4]>) -> Raster {
    Raster {
        width,
        height: rgba.len() as u32 / width,
        rgba,
    }
}

/// A PNG of `color` type and 8-bit `data`, by the png crate.
fn png_of(
    width: u32,
    height: u32,
    color: png::ColorType,
    depth: png::BitDepth,
    data: &[u8],
) -> Vec<u8> {
    let mut out = Vec::new();
    let mut e = png::Encoder::new(&mut out, width, height);
    e.set_color(color);
    e.set_depth(depth);
    if color == png::ColorType::Indexed {
        e.set_palette(vec![0x10, 0x20, 0x30, 0xF0, 0xE0, 0xD0]);
    }
    e.write_header().unwrap().write_image_data(data).unwrap();
    out
}

#[test]
fn the_made_legend_is_clear_then_lightest_first_ties_by_hex() {
    // #001700 and #0300db are equally light to rgbgfx (2126 R + 7152 G + 722 B).
    let r = raster(
        5,
        vec![
            [0, 0, 0, 255],
            [0x03, 0x00, 0xdb, 255],
            [0, 0, 0, 0],
            [0x00, 0x17, 0x00, 255],
            [255, 255, 255, 255],
        ],
    );
    let p = from_raster(&r).unwrap();
    let legend: Vec<(char, Value)> = p
        .legend
        .iter()
        .map(|s| (s.symbol, s.value.clone()))
        .collect();
    assert_eq!(
        legend,
        [
            ('.', Value::Clear),
            ('0', Value::Color([255, 255, 255])),
            ('1', Value::Color([0x00, 0x17, 0x00])),
            ('2', Value::Color([0x03, 0x00, 0xdb])),
            ('3', Value::Color([0, 0, 0])),
        ]
    );
    let text = to_text(&p, Layout::Rows { frame: None }).unwrap();
    assert!(text.ends_with("rows 0 0\n32.10\n"), "{text}");
}

#[test]
fn four_shades_read_as_0_to_3_with_no_clear() {
    let gray = |c: u8| [c, c, c, 255];
    let r = raster(4, vec![gray(0x55), gray(0xFF), gray(0x00), gray(0xAA)]);
    let p = from_raster(&r).unwrap();
    assert!(!p.has_clear());
    let text = to_text(&p, Layout::Rows { frame: None }).unwrap();
    assert_eq!(
        text,
        "format gbcli-pixels 1\nsize 4 1\ncolor 0 #ffffff\ncolor 1 #aaaaaa\ncolor 2 #555555\n\
         color 3 #000000\n\nrows 0 0\n2031\n"
    );
}

#[test]
fn half_transparent_pixels_have_no_spelling() {
    let r = raster(3, vec![[0, 0, 0, 255], [0, 0, 0, 0], [9, 9, 9, 128]]);
    let e = from_raster(&r).expect_err("alpha 128");
    assert_eq!(e.pixel, Some((2, 0)));
    assert_eq!(
        e.message,
        "this pixel is partly transparent (alpha 128), and pixel text has no spelling for that"
    );
    assert_eq!(
        e.remedy,
        "make every pixel opaque (alpha 255) or clear (alpha 0); the build refuses partial alpha too"
    );
}

#[test]
fn more_colors_than_legend_characters_names_the_first_too_many() {
    let n = MADE_SYMBOLS.chars().count();
    assert_eq!(n, 92, "93 legend characters, `.` kept for clear");
    let rgba: Vec<[u8; 4]> = (0..=n as u8).map(|i| [i, 0, 0, 255]).collect();
    let e = from_raster(&raster(n as u32 + 1, rgba)).expect_err("93 colors");
    assert_eq!(e.pixel, Some((92, 0)));
    assert_eq!(
        e.message,
        "this pixel is the image's 93rd color, and pixel text has 92 characters for colors"
    );
    assert_eq!(
        e.remedy,
        "use fewer colors: no Game Boy image needs this many (a --gbc background holds 32)"
    );
}

#[test]
fn a_png_encodes_and_decodes_to_the_same_pixels() {
    let opaque = raster(2, vec![[1, 2, 3, 255], [4, 5, 6, 255]]);
    let bytes = encode_png(&opaque);
    assert!(is_png(&bytes));
    assert_eq!(decode_png(&bytes).unwrap(), opaque);
    let clear = raster(2, vec![[1, 2, 3, 255], [0, 0, 0, 0]]);
    assert_eq!(decode_png(&encode_png(&clear)).unwrap(), clear);
    assert_eq!(encode_png(&clear), encode_png(&clear));
    // Opaque art is written as RGB, so it's smaller than RGBA.
    let decoder = png::Decoder::new(std::io::Cursor::new(bytes));
    assert_eq!(
        decoder.read_info().unwrap().info().color_type,
        png::ColorType::Rgb
    );
}

#[test]
fn every_color_type_decodes_to_rgba_and_16_bits_scale_as_rgbgfx_scales_them() {
    use png::{BitDepth::*, ColorType::*};
    let gray = png_of(2, 1, Grayscale, Eight, &[0x00, 0xFF]);
    assert_eq!(decode_png(&gray).unwrap().rgba, [[0, 0, 0, 255], [255; 4]]);
    let indexed = png_of(2, 1, Indexed, Eight, &[1, 0]);
    assert_eq!(
        decode_png(&indexed).unwrap().rgba,
        [[0xF0, 0xE0, 0xD0, 255], [0x10, 0x20, 0x30, 255]]
    );
    let ga = png_of(1, 1, GrayscaleAlpha, Eight, &[0x80, 0x00]);
    assert_eq!(decode_png(&ga).unwrap().rgba, [[0x80, 0x80, 0x80, 0]]);
    // The high byte alone would give $07; libpng's scaling gives $08.
    let deep = png_of(1, 1, Rgb, Sixteen, &[0x07, 0xFF, 0x08, 0x00, 0x80, 0x7F]);
    assert_eq!(decode_png(&deep).unwrap().rgba, [[0x08, 0x08, 0x80, 255]]);
}

#[test]
fn a_file_is_read_as_png_or_text_by_its_bytes() {
    let png = encode_png(&raster(1, vec![[0, 0, 0, 0]]));
    let loaded = read(&png).unwrap();
    assert_eq!(loaded.form, Form::Png);
    assert!(loaded.picture.has_clear());
    let e = read(&png[..20]).expect_err("cut short");
    let gbcli_pixels::PixelsError::Png(e) = e else {
        panic!("{e:?}")
    };
    assert_eq!(e.pixel, None);
    assert!(
        e.message.starts_with("the PNG doesn't decode: "),
        "{}",
        e.message
    );
    assert_eq!(e.remedy, "save it again as a PNG, or check the path");
}
