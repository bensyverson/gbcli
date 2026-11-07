//! PNG encoding: palette-indexed at the smallest bit depth that holds the
//! image's colors, else 8-bit RGB. Every setting is pinned, so an image gives
//! the same bytes on every run (design §10).

use std::collections::{HashMap, HashSet};

use crate::ViewError;
use crate::image::Image;

/// Colors an indexed PNG can hold.
const MAX_PALETTE: usize = 256;

pub(crate) fn png(image: &Image) -> Result<Vec<u8>, ViewError> {
    let mut out = Vec::new();
    let mut encoder = png::Encoder::new(&mut out, image.width, image.height);
    encoder.set_compression(png::Compression::Best);
    encoder.set_adaptive_filter(png::AdaptiveFilterType::NonAdaptive);
    let data = match palette(&image.pixels) {
        Some(colors) => {
            let depth = depth_for(colors.len());
            encoder.set_color(png::ColorType::Indexed);
            encoder.set_depth(depth.0);
            encoder.set_filter(png::FilterType::NoFilter);
            encoder.set_palette(
                colors
                    .iter()
                    .flat_map(|&c| [(c >> 16) as u8, (c >> 8) as u8, c as u8])
                    .collect::<Vec<u8>>(),
            );
            pack_indices(image, &colors, depth.1)
        }
        None => {
            encoder.set_color(png::ColorType::Rgb);
            encoder.set_depth(png::BitDepth::Eight);
            encoder.set_filter(png::FilterType::Paeth);
            image
                .pixels
                .iter()
                .flat_map(|&c| [(c >> 16) as u8, (c >> 8) as u8, c as u8])
                .collect()
        }
    };
    for (key, text) in &image.labels {
        encoder
            .add_text_chunk(key.keyword().to_string(), text.clone())
            .map_err(png_err)?;
    }
    let mut writer = encoder.write_header().map_err(png_err)?;
    writer.write_image_data(&data).map_err(png_err)?;
    writer.finish().map_err(png_err)?;
    Ok(out)
}

fn png_err(err: png::EncodingError) -> ViewError {
    ViewError::Png(err.to_string())
}

/// The image's colors in order of first appearance, if they fit a palette.
fn palette(pixels: &[u32]) -> Option<Vec<u32>> {
    let mut colors: Vec<u32> = Vec::new();
    let mut seen = HashSet::new();
    for &p in pixels {
        if seen.insert(p) {
            if colors.len() == MAX_PALETTE {
                return None;
            }
            colors.push(p);
        }
    }
    Some(colors)
}

/// The smallest PNG bit depth for `n` palette entries, and its bits.
fn depth_for(n: usize) -> (png::BitDepth, u32) {
    match n {
        0..=2 => (png::BitDepth::One, 1),
        3..=4 => (png::BitDepth::Two, 2),
        5..=16 => (png::BitDepth::Four, 4),
        _ => (png::BitDepth::Eight, 8),
    }
}

/// Palette indices packed `bits` per pixel, most significant first, each
/// row padded to a whole byte.
fn pack_indices(image: &Image, colors: &[u32], bits: u32) -> Vec<u8> {
    let per_byte = 8 / bits;
    let row_bytes = image.width.div_ceil(per_byte) as usize;
    let index_of: HashMap<u32, u8> = colors
        .iter()
        .enumerate()
        .map(|(i, &c)| (c, i as u8))
        .collect();
    let mut data = vec![0u8; row_bytes * image.height as usize];
    for (y, row) in image.pixels.chunks(image.width.max(1) as usize).enumerate() {
        for (x, p) in row.iter().enumerate() {
            let index = index_of[p];
            let x = x as u32;
            let byte = y * row_bytes + (x / per_byte) as usize;
            let shift = 8 - bits * (x % per_byte + 1);
            data[byte] |= index << shift;
        }
    }
    data
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::image::LabelKey;

    fn image(width: u32, height: u32, pixels: Vec<u32>) -> Image {
        Image {
            width,
            height,
            pixels,
            labels: Vec::new(),
        }
    }

    /// Decodes to (color type, bit depth, RGB pixels, text chunks).
    fn decode(
        bytes: &[u8],
    ) -> (
        png::ColorType,
        png::BitDepth,
        Vec<u32>,
        Vec<(String, String)>,
    ) {
        let mut decoder = png::Decoder::new(bytes);
        decoder.set_transformations(png::Transformations::EXPAND);
        let mut reader = decoder.read_info().unwrap();
        let (color, depth) = (reader.info().color_type, reader.info().bit_depth);
        let mut buf = vec![0; reader.output_buffer_size()];
        let frame = reader.next_frame(&mut buf).unwrap();
        let pixels = buf[..frame.buffer_size()]
            .chunks(3)
            .map(|c| (u32::from(c[0]) << 16) | (u32::from(c[1]) << 8) | u32::from(c[2]))
            .collect();
        let text = reader
            .info()
            .uncompressed_latin1_text
            .iter()
            .map(|t| (t.keyword.clone(), t.text.clone()))
            .collect();
        (color, depth, pixels, text)
    }

    #[test]
    fn few_colors_encode_indexed_at_the_smallest_depth() {
        for (colors, depth) in [
            (2, png::BitDepth::One),
            (4, png::BitDepth::Two),
            (5, png::BitDepth::Four),
            (16, png::BitDepth::Four),
            (17, png::BitDepth::Eight),
            (256, png::BitDepth::Eight),
        ] {
            // 13 wide, so rows end mid-byte at every depth below 8.
            let pixels: Vec<u32> = (0..13 * 20).map(|i| (i % colors) * 0x01_01_01).collect();
            let bytes = image(13, 20, pixels.clone()).png().unwrap();
            let (color, got, decoded, _) = decode(&bytes);
            assert_eq!((color, got), (png::ColorType::Indexed, depth), "{colors}");
            assert_eq!(decoded, pixels, "{colors} colors round-trip");
        }
    }

    #[test]
    fn many_colors_encode_as_rgb() {
        let pixels: Vec<u32> = (0..300).map(|i| i * 0x00_0101).collect();
        let (color, depth, decoded, _) = decode(&image(30, 10, pixels.clone()).png().unwrap());
        assert_eq!((color, depth), (png::ColorType::Rgb, png::BitDepth::Eight));
        assert_eq!(decoded, pixels);
    }

    #[test]
    fn labels_become_text_chunks_and_bytes_repeat() {
        let mut img = image(2, 1, vec![0xFFFFFF, 0]);
        img.labels
            .push((LabelKey::Frame, "complete frame 3 (vblank normal)".into()));
        let bytes = img.png().unwrap();
        let (_, _, _, text) = decode(&bytes);
        assert_eq!(
            text,
            [(
                "gbcli:frame".to_string(),
                "complete frame 3 (vblank normal)".to_string()
            )]
        );
        assert_eq!(img.png().unwrap(), bytes);
    }
}
