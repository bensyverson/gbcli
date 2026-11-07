//! PNG encodings the agree check must let through when gbcli and rgbgfx
//! read them alike, and one the build must refuse, against the real rgbgfx
//! 1.0.4: 16-bit samples on every rounding boundary, 16-bit RGBA, RGB with
//! a tRNS key, 2-bit paletted and gray, flipped and repeated frames (the
//! second review's probes, leaf FPQsnp's review; kept as guards in iH2KOO).

mod common;
mod pngs;

use std::fs;
use std::path::Path;

use gbcli_build::{BuildError, build};

use common::{rgbds, starter};

fn describe(r: &Result<gbcli_protocol::BuildMade, BuildError>) -> String {
    match r {
        Ok(_) => "ok".into(),
        Err(BuildError::Failed { made, .. }) => format!(
            "failed: diagnostics {:#?}, images {:#?}",
            made.diagnostics,
            made.images.as_ref().map(|i| &i.problems)
        ),
        Err(e) => format!("{:?}", e.body()),
    }
}

/// A raw PNG: `data` already packed for `color` at `depth`.
struct Raw<'a> {
    width: u32,
    height: u32,
    color: png::ColorType,
    depth: png::BitDepth,
    data: &'a [u8],
    palette: Option<&'a [u8]>,
    trns: Option<&'a [u8]>,
}

fn write_raw(path: &Path, raw: Raw<'_>) {
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    let file = fs::File::create(path).unwrap();
    let mut encoder = png::Encoder::new(file, raw.width, raw.height);
    encoder.set_color(raw.color);
    encoder.set_depth(raw.depth);
    if let Some(p) = raw.palette {
        encoder.set_palette(p.to_vec());
    }
    if let Some(t) = raw.trns {
        encoder.set_trns(t.to_vec());
    }
    let mut writer = encoder.write_header().unwrap();
    writer.write_image_data(raw.data).unwrap();
}

/// 16-bit gray backgrounds whose samples sit on every 8-bit boundary where
/// rounding (libpng's scale_16) and truncation pick different RGB555 colors.
/// The agree check referees each: the build must succeed.
#[test]
fn sixteen_bit_gray_at_every_rgb555_boundary_builds() {
    let (temp, options) = starter("png-16-boundaries");
    for k in 1u16..32 {
        let below = (8 * k - 1) << 8;
        let samples = [
            below,
            below | 0x7E,
            below | 0x7F,
            below | 0x80,
            below | 0x81,
            below | 0xFF,
        ];
        let mut data = Vec::new();
        for y in 0..8u32 {
            for x in 0..8u32 {
                let v = if y == 0 && k < 31 {
                    0xFFFF
                } else {
                    samples[(x as usize + y as usize) % 6]
                };
                data.extend_from_slice(&v.to_be_bytes());
            }
        }
        write_raw(
            &temp.path(&format!("demo/assets/bg/edge{k}.png")),
            Raw {
                width: 8,
                height: 8,
                color: png::ColorType::Grayscale,
                depth: png::BitDepth::Sixteen,
                data: &data,
                palette: None,
                trns: None,
            },
        );
    }
    let r = build(&rgbds(), &options);
    assert!(r.is_ok(), "{}", describe(&r));
    let made = r.unwrap();
    assert_eq!(
        made.images.unwrap().images.len(),
        31,
        "every image converted"
    );
}

/// A 16-bit RGBA sprite sheet: transparent pixels, and colors on the
/// rounding boundary.
#[test]
fn sixteen_bit_rgba_sheet_builds() {
    let (temp, options) = starter("png-16-rgba");
    let mut data = Vec::new();
    for y in 0..16u32 {
        for x in 0..8u32 {
            let px: [u16; 4] = match (x + y) % 4 {
                0 => [0, 0, 0, 0],
                1 => [0x077F, 0x0780, 0x07FF, 0xFFFF],
                2 => [0x8F80, 0x8F7F, 0x8F80, 0xFFFF],
                _ => [0x0000, 0x0000, 0x0000, 0xFFFF],
            };
            for s in px {
                data.extend_from_slice(&s.to_be_bytes());
            }
        }
    }
    write_raw(
        &temp.path("demo/assets/sprites/deep.png"),
        Raw {
            width: 8,
            height: 16,
            color: png::ColorType::Rgba,
            depth: png::BitDepth::Sixteen,
            data: &data,
            palette: None,
            trns: None,
        },
    );
    let r = build(&rgbds(), &options);
    assert!(r.is_ok(), "{}", describe(&r));
}

/// An RGB PNG (no alpha channel) whose tRNS chunk makes black transparent:
/// a sprite sheet's usual export from some editors.
#[test]
fn rgb_with_trns_sheet_builds() {
    let (temp, options) = starter("png-rgb-trns");
    let mut data = Vec::new();
    for y in 0..16u32 {
        for x in 0..8u32 {
            let c: [u8; 3] = if x == y % 8 {
                [0xFF, 0xFF, 0xFF]
            } else if x == 0 {
                [0x55, 0x55, 0x55]
            } else {
                [0, 0, 0]
            };
            data.extend_from_slice(&c);
        }
    }
    write_raw(
        &temp.path("demo/assets/sprites/keyed.png"),
        Raw {
            width: 8,
            height: 16,
            color: png::ColorType::Rgb,
            depth: png::BitDepth::Eight,
            data: &data,
            palette: None,
            trns: Some(&[0, 0, 0, 0, 0, 0]),
        },
    );
    let r = build(&rgbds(), &options);
    assert!(r.is_ok(), "{}", describe(&r));
}

/// A 2-bit paletted sheet with tRNS, and a 2-bit gray background.
#[test]
fn low_depth_pngs_build() {
    let (temp, options) = starter("png-low-depth");
    // 8 pixels a row at 2 bits: 2 bytes a row.
    let rows = |f: &dyn Fn(u32, u32) -> u8, h: u32| -> Vec<u8> {
        let mut out = Vec::new();
        for y in 0..h {
            let mut byte = 0u8;
            for x in 0..8u32 {
                byte = (byte << 2) | f(x, y);
                if x % 4 == 3 {
                    out.push(byte);
                    byte = 0;
                }
            }
        }
        out
    };
    let sheet = rows(&|x, y| ((x + y) % 4) as u8, 16);
    write_raw(
        &temp.path("demo/assets/sprites/pal.png"),
        Raw {
            width: 8,
            height: 16,
            color: png::ColorType::Indexed,
            depth: png::BitDepth::Two,
            data: &sheet,
            palette: Some(&[
                0x12, 0x34, 0x56, 0xFF, 0xFF, 0xFF, 0x80, 0x80, 0x80, 0, 0, 0,
            ]),
            trns: Some(&[0]),
        },
    );
    let bg = rows(&|x, y| ((x * y) % 4) as u8, 8);
    write_raw(
        &temp.path("demo/assets/bg/gray2.png"),
        Raw {
            width: 8,
            height: 8,
            color: png::ColorType::Grayscale,
            depth: png::BitDepth::Two,
            data: &bg,
            palette: None,
            trns: None,
        },
    );
    let r = build(&rgbds(), &options);
    assert!(r.is_ok(), "{}", describe(&r));
}

/// A sheet of four 16x16 frames: a figure, its mirror (flip-merged units),
/// an empty frame, and the figure again (deduped). Only the first frame's
/// units are kept; the agree check must let it through.
#[test]
fn flipped_empty_and_repeated_frames_build() {
    use pngs::{BLACK, CLEAR, DARK, LIGHT, write_png};
    let (temp, options) = starter("png-flips");
    let figure = |x: u32, y: u32| match (x * 3 + y * 5) % 7 {
        0 => BLACK,
        1 => DARK,
        2 => LIGHT,
        _ => CLEAR,
    };
    write_png(
        &temp.path("demo/assets/sprites/hero.16x16.png"),
        64,
        16,
        |x, y| {
            let (frame, fx) = (x / 16, x % 16);
            match frame {
                0 | 3 => figure(fx, y),
                1 => figure(15 - fx, y),
                _ => CLEAR,
            }
        },
    );
    let r = build(&rgbds(), &options);
    assert!(r.is_ok(), "{}", describe(&r));
    let made = r.unwrap();
    let images = made.images.unwrap();
    assert_eq!(images.images.len(), 1);
    assert_eq!(
        images.images[0].tiles, 4,
        "two kept units: {:?}",
        images.images[0]
    );
}

/// A 16-bit gray background with a tRNS key: backgrounds have no
/// transparency, so gbcli must refuse it as rgbgfx would (not build it).
#[test]
fn sixteen_bit_gray_with_trns_background_is_refused_as_alpha() {
    let (temp, options) = starter("png-16-gray-trns");
    let mut data = Vec::new();
    for y in 0..8u32 {
        for x in 0..8u32 {
            let v: u16 = if x == y { 0x1234 } else { 0xFFFF };
            data.extend_from_slice(&v.to_be_bytes());
        }
    }
    write_raw(
        &temp.path("demo/assets/bg/keyed.png"),
        Raw {
            width: 8,
            height: 8,
            color: png::ColorType::Grayscale,
            depth: png::BitDepth::Sixteen,
            data: &data,
            palette: None,
            trns: Some(&[0x12, 0x34]),
        },
    );
    let made = match build(&rgbds(), &options) {
        Err(BuildError::Failed { made, .. }) => *made,
        other => panic!(
            "expected the alpha check to stop it, got {}",
            describe(&other)
        ),
    };
    let limits: Vec<_> = made
        .images
        .unwrap()
        .problems
        .iter()
        .map(|p| p.limit)
        .collect();
    assert!(
        limits.contains(&gbcli_protocol::ImageLimit::Alpha),
        "{limits:?}"
    );
}
