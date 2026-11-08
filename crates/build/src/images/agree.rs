//! The check that rgbgfx drew what gbcli planned. gbcli decides each
//! pixel's color index (its shade) before rgbgfx runs, and the problems,
//! previews and frame lists all describe that plan; rgbgfx decodes the PNG
//! on its own. So after it runs, its tile data is compared with the plan
//! pixel by pixel, and any difference stops the build (fail closed) rather
//! than ship shades nothing checked.
//!
//! A GBC version is compared by color, never by palette number: rgbgfx may
//! draw a tile from another of the given palettes that holds its colors
//! (research §3), which is as good, so what must agree is the color each
//! pixel draws.

use gbcli_protocol::{ImageLimit, ImageProblem, Rgb555, TileAt};

use super::census::error;

/// One pixel where rgbgfx's data and gbcli's plan differ.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct Difference {
    /// The pixel within the compared block (8 wide).
    pub x: u32,
    pub y: u32,
    pub planned: u8,
    pub wrote: u8,
}

/// One tile that disagrees: where in the source image, and how.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct Disagreement {
    pub tile: TileAt,
    /// The first pixel that differs (within the tile); none when rgbgfx's
    /// map names a tile it didn't write.
    pub pixel: Option<Difference>,
}

/// 2bpp tile data as color indices, 8 a row.
pub(crate) fn indices(data: &[u8]) -> Vec<u8> {
    data.chunks(2)
        .flat_map(|row| {
            let (low, high) = (row[0], row[1]);
            (0..8).map(move |x| ((low >> (7 - x)) & 1) | (((high >> (7 - x)) & 1) << 1))
        })
        .collect()
}

/// Color indices, 8 a row, as 2bpp tile data: `indices`' inverse.
pub(crate) fn encode(indices: &[u8]) -> Vec<u8> {
    indices
        .chunks(8)
        .flat_map(|row| {
            let plane = |bit: u8| {
                row.iter()
                    .enumerate()
                    .fold(0u8, |b, (x, &i)| b | (((i >> bit) & 1) << (7 - x)))
            };
            [plane(0), plane(1)]
        })
        .collect()
}

/// One pixel of a GBC version where rgbgfx's data draws another color
/// than the image holds; `None` is transparent.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct ColorDisagreement {
    pub tile: TileAt,
    /// The pixel within the tile.
    pub x: u32,
    pub y: u32,
    pub planned: Option<Rgb555>,
    pub wrote: Option<Rgb555>,
}

/// The first pixel, row by row, where `data` (2bpp, as rgbgfx wrote it)
/// doesn't draw `planned` (color indices, 8 a row, as many rows).
pub(crate) fn first_difference(planned: &[u8], data: &[u8]) -> Option<Difference> {
    let wrote = indices(data);
    let i = (0..planned.len().max(wrote.len())).find(|&i| planned.get(i) != wrote.get(i))?;
    Some(Difference {
        x: (i % 8) as u32,
        y: (i / 8) as u32,
        planned: planned.get(i).copied().unwrap_or(0),
        wrote: wrote.get(i).copied().unwrap_or(0),
    })
}

/// The problem for an image whose converted tiles disagree with its plan.
pub(crate) fn problem(image: &str, disagreements: &[Disagreement]) -> ImageProblem {
    let first = disagreements.first().map_or(String::new(), |d| {
        let TileAt { x, y } = d.tile;
        match d.pixel {
            Some(p) => format!(
                "; at tile ({x}, {y}), pixel ({}, {}), gbcli planned color index {} and \
                     rgbgfx wrote index {}",
                p.x, p.y, p.planned, p.wrote
            ),
            None => format!("; at tile ({x}, {y}), rgbgfx's map names a tile it didn't write"),
        }
    });
    let tiles = disagreements.iter().map(|d| d.tile).collect();
    stopped(image, tiles, &first)
}

/// The problem for a GBC version whose converted tiles draw other colors
/// than the image: one disagreement per tile, its first pixel.
pub(crate) fn color_problem(image: &str, disagreements: &[ColorDisagreement]) -> ImageProblem {
    let color = |c: Option<Rgb555>| match c {
        Some(c) => format!("RGB555 ${:04x}", c.word()),
        None => "transparent".into(),
    };
    let first = disagreements.first().map_or(String::new(), |d| {
        format!(
            "; at tile ({}, {}), pixel ({}, {}), the image holds {} and rgbgfx's GBC data draws {}",
            d.tile.x,
            d.tile.y,
            d.x,
            d.y,
            color(d.planned),
            color(d.wrote)
        )
    });
    let tiles = disagreements.iter().map(|d| d.tile).collect();
    stopped(image, tiles, &first)
}

/// `rgbgfx_agrees` for `tiles` of `image`; `first` describes the first.
fn stopped(image: &str, tiles: Vec<TileAt>, first: &str) -> ImageProblem {
    let n = tiles.len();
    ImageProblem {
        tiles,
        ..error(
            image,
            ImageLimit::RgbgfxAgrees,
            n,
            0,
            format!(
                "gbcli and rgbgfx disagree on {n} tile{} of {image}{first}. gbcli checked its \
                 own reading of the image, so the build stops rather than ship shades nothing \
                 checked",
                if n == 1 { "" } else { "s" }
            ),
            "this is a bug in gbcli, not in the image: please report it with the PNG. Until \
             it's fixed, re-save the image as an 8-bit PNG (gray, RGB or RGBA), which both \
             read alike",
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// One 8x8 tile whose rows are `rows` (one index for the whole row).
    fn tile(rows: [u8; 8]) -> (Vec<u8>, Vec<u8>) {
        let planned = rows.iter().flat_map(|&i| [i; 8]).collect();
        let data = rows
            .iter()
            .flat_map(|&i| {
                let plane = |bit: u8| if i & bit != 0 { 0xFF } else { 0x00 };
                [plane(1), plane(2)]
            })
            .collect();
        (planned, data)
    }

    #[test]
    fn two_bpp_rows_read_as_indices_left_pixel_first() {
        // Low plane $80, high plane $01: pixel 0 is index 1, pixel 7 index 2.
        let row = indices(&[0x80, 0x01]);
        assert_eq!(row, [1, 0, 0, 0, 0, 0, 0, 2]);
        let (planned, data) = tile([0, 1, 2, 3, 3, 2, 1, 0]);
        assert_eq!(indices(&data), planned);
    }

    #[test]
    fn indices_encode_back_to_the_2bpp_they_read_from() {
        let (planned, data) = tile([0, 1, 2, 3, 3, 2, 1, 0]);
        assert_eq!(encode(&planned), data);
        let mixed: Vec<u8> = (0..64).map(|i| (i * 7 % 4) as u8).collect();
        assert_eq!(indices(&encode(&mixed)), mixed);
    }

    #[test]
    fn a_color_disagreement_names_the_tile_pixel_and_both_colors() {
        let p = color_problem(
            "assets/bg/town.png",
            &[ColorDisagreement {
                tile: TileAt { x: 2, y: 1 },
                x: 3,
                y: 4,
                planned: Some(Rgb555::WHITE),
                wrote: None,
            }],
        );
        assert_eq!(p.limit, ImageLimit::RgbgfxAgrees);
        assert_eq!(p.tiles, [TileAt { x: 2, y: 1 }]);
        for part in ["tile (2, 1)", "pixel (3, 4)", "$7fff", "transparent"] {
            assert!(p.message.contains(part), "{part:?} in {}", p.message);
        }
    }

    #[test]
    fn the_first_differing_pixel_is_found_with_both_indices() {
        let (planned, mut data) = tile([3, 3, 3, 3, 3, 3, 3, 3]);
        assert_eq!(first_difference(&planned, &data), None);
        // Row 5, pixel 6: clear its high-plane bit, so 3 becomes 1.
        data[5 * 2 + 1] &= !0x02;
        assert_eq!(
            first_difference(&planned, &data),
            Some(Difference {
                x: 6,
                y: 5,
                planned: 3,
                wrote: 1
            })
        );
    }

    #[test]
    fn an_8x16_unit_compares_both_tiles() {
        let (top, top_data) = tile([1; 8]);
        let (bottom, mut bottom_data) = tile([2; 8]);
        bottom_data[0] = 0x01;
        let planned: Vec<u8> = top.into_iter().chain(bottom).collect();
        let data: Vec<u8> = top_data.into_iter().chain(bottom_data).collect();
        let d = first_difference(&planned, &data).expect("the bottom tile differs");
        assert_eq!((d.x, d.y, d.planned, d.wrote), (7, 8, 2, 3));
    }

    #[test]
    fn the_problem_names_the_image_the_tiles_and_the_first_pixel_with_a_remedy() {
        let at = |x, y| TileAt { x, y };
        let p = problem(
            "assets/bg/town.png",
            &[
                Disagreement {
                    tile: at(3, 1),
                    pixel: Some(Difference {
                        x: 2,
                        y: 7,
                        planned: 3,
                        wrote: 2,
                    }),
                },
                Disagreement {
                    tile: at(4, 1),
                    pixel: None,
                },
            ],
        );
        assert_eq!(p.image.as_deref(), Some("assets/bg/town.png"));
        assert_eq!(p.limit, ImageLimit::RgbgfxAgrees);
        assert_eq!(p.severity, gbcli_protocol::Severity::Error);
        assert_eq!((p.count, p.max), (2, 0));
        assert_eq!(p.tiles, [at(3, 1), at(4, 1)]);
        for part in [
            "gbcli and rgbgfx disagree",
            "assets/bg/town.png",
            "tile (3, 1)",
            "pixel (2, 7)",
            "index 3",
            "index 2",
        ] {
            assert!(p.message.contains(part), "{part:?} in {}", p.message);
        }
        assert!(p.remedy.contains("8-bit"), "{}", p.remedy);
        assert!(p.remedy.contains("report"), "{}", p.remedy);
    }

    #[test]
    fn a_map_naming_a_missing_tile_is_said_so() {
        let p = problem(
            "assets/bg/town.png",
            &[Disagreement {
                tile: TileAt { x: 0, y: 2 },
                pixel: None,
            }],
        );
        assert!(p.message.contains("tile (0, 2)"), "{}", p.message);
        assert!(p.message.contains("didn't write"), "{}", p.message);
    }
}
