//! Test images: PNGs drawn from a function of the pixel, written into a
//! starter project's `assets/`, and read back.

use std::fs;
use std::path::Path;

use gbcli_build::{BuildError, BuildOptions, build};
use gbcli_protocol::{BuildMade, ImageLimit, ImageProblem, Severity};

use crate::common::rgbds;

pub type Rgba = [u8; 4];

pub const WHITE: Rgba = [0xFF, 0xFF, 0xFF, 0xFF];
pub const LIGHT: Rgba = [0xAA, 0xAA, 0xAA, 0xFF];
pub const DARK: Rgba = [0x55, 0x55, 0x55, 0xFF];
pub const BLACK: Rgba = [0x00, 0x00, 0x00, 0xFF];
pub const CLEAR: Rgba = [0, 0, 0, 0];

/// Writes a `width`x`height` RGBA PNG at `path`, each pixel from `pixel`.
pub fn write_png(path: &Path, width: u32, height: u32, pixel: impl Fn(u32, u32) -> Rgba) {
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    let mut data = Vec::new();
    for y in 0..height {
        for x in 0..width {
            data.extend_from_slice(&pixel(x, y));
        }
    }
    let file = fs::File::create(path).unwrap();
    let mut encoder = png::Encoder::new(file, width, height);
    encoder.set_color(png::ColorType::Rgba);
    encoder.set_depth(png::BitDepth::Eight);
    let mut writer = encoder.write_header().unwrap();
    writer.write_image_data(&data).unwrap();
}

/// A PNG's size and pixels as 0xRRGGBB.
pub fn read_png(path: &Path) -> (u32, u32, Vec<u32>) {
    let mut decoder = png::Decoder::new(fs::File::open(path).unwrap());
    decoder.set_transformations(png::Transformations::EXPAND);
    let mut reader = decoder.read_info().unwrap();
    let mut buf = vec![0; reader.output_buffer_size()];
    let info = reader.next_frame(&mut buf).unwrap();
    let channels = info.color_type.samples();
    let pixels = buf[..info.buffer_size()]
        .chunks(channels)
        .map(|p| {
            if channels < 3 {
                u32::from(p[0]) * 0x01_01_01
            } else {
                (u32::from(p[0]) << 16) | (u32::from(p[1]) << 8) | u32::from(p[2])
            }
        })
        .collect();
    (info.width, info.height, pixels)
}

/// The 2bpp color index of pixel `x` in row `row` of the tile data at `data`.
pub fn index_at(data: &[u8], row: usize, x: u32) -> u8 {
    let (lo, hi) = (data[row * 2], data[row * 2 + 1]);
    ((lo >> (7 - x)) & 1) | (((hi >> (7 - x)) & 1) << 1)
}

/// The value of `DEF <constant> EQU %..` in images.inc, read as binary.
pub fn binary_constant(inc: &str, constant: &str) -> u8 {
    let prefix = format!("DEF {constant} EQU %");
    let bits = inc
        .lines()
        .find_map(|l| l.strip_prefix(prefix.as_str()))
        .unwrap_or_else(|| panic!("{constant} in\n{inc}"));
    let bits: String = bits.split(' ').next().unwrap().replace('_', "");
    u8::from_str_radix(&bits, 2).unwrap()
}

/// The problems under `limit`, any number.
pub fn problems(made: &BuildMade, limit: ImageLimit) -> Vec<ImageProblem> {
    let images = made.images.as_ref().expect("the build reports its images");
    images
        .problems
        .iter()
        .filter(|p| p.limit == limit)
        .cloned()
        .collect()
}

/// Tile `n` as a pattern no other `n` below 2^16 shares: its bits as black
/// pixels along the first two rows, on white.
pub fn numbered_tile(n: u32, x: u32, y: u32) -> Rgba {
    let bit = y * 8 + x;
    if bit < 16 && (n >> bit) & 1 == 1 {
        BLACK
    } else {
        WHITE
    }
}

/// Builds the project and expects the images to stop it.
pub fn refused(options: &BuildOptions) -> BuildMade {
    match build(&rgbds(), options) {
        Err(BuildError::Failed { made, .. }) => *made,
        other => panic!("expected the images to stop the build, got {other:?}"),
    }
}

/// The one problem under `limit`.
pub fn problem(made: &BuildMade, limit: ImageLimit) -> ImageProblem {
    let images = made.images.as_ref().expect("the build reports its images");
    let found: Vec<&ImageProblem> = images
        .problems
        .iter()
        .filter(|p| p.limit == limit)
        .collect();
    assert_eq!(
        found.len(),
        1,
        "one {limit:?} problem in {:#?}",
        images.problems
    );
    let p = found[0].clone();
    assert!(!p.message.is_empty() && !p.remedy.is_empty(), "{p:?}");
    p
}

/// `made`'s errors are exactly `limits`, in order.
pub fn errors(made: &BuildMade) -> Vec<ImageLimit> {
    made.images
        .as_ref()
        .map(|i| {
            i.problems
                .iter()
                .filter(|p| p.severity == Severity::Error)
                .map(|p| p.limit)
                .collect()
        })
        .unwrap_or_default()
}
