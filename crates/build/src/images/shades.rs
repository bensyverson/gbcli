//! Which DMG shade each of an image's colors becomes. The DMG shows four
//! shades, 0 (lightest) to 3; an image may draw them in any four colors
//! (grays, or the classic greens), so each color takes the shade nearest
//! its lightness, and colors that would share a shade are spread apart in
//! order of lightness instead.

use super::pixels::Rgba;

/// Lightness as rgbgfx weighs it: 2126 R + 7152 G + 722 B.
pub(crate) fn luminance(c: Rgba) -> u32 {
    2126 * u32::from(c[0]) + 7152 * u32::from(c[1]) + 722 * u32::from(c[2])
}

const WHITE_LUMINANCE: u32 = 255 * 10_000;

/// The shade of each of `colors` (distinct, at most four), in their order.
pub(crate) fn shades(colors: &[Rgba]) -> Vec<u8> {
    assert!(colors.len() <= 4, "at most four colors take shades");
    let mut order: Vec<usize> = (0..colors.len()).collect();
    // Lightest first; ties by color, so the result never depends on order.
    order.sort_by_key(|&i| (std::cmp::Reverse(luminance(colors[i])), colors[i]));
    let nearest: Vec<u8> = order
        .iter()
        .map(|&i| {
            let dark = WHITE_LUMINANCE - luminance(colors[i]);
            ((dark * 3 + WHITE_LUMINANCE / 2) / WHITE_LUMINANCE) as u8
        })
        .collect();
    let distinct = nearest.windows(2).all(|w| w[0] < w[1]);
    let by_rank: Vec<u8> = if distinct {
        nearest
    } else {
        match colors.len() {
            2 => vec![0, 3],
            3 => {
                let middle = luminance(colors[order[1]]);
                vec![0, if middle * 2 >= WHITE_LUMINANCE { 1 } else { 2 }, 3]
            }
            _ => vec![0, 1, 2, 3],
        }
    };
    let mut out = vec![0; colors.len()];
    for (rank, &i) in order.iter().enumerate() {
        out[i] = by_rank[rank];
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    const W: Rgba = [255, 255, 255, 255];
    const L: Rgba = [170, 170, 170, 255];
    const D: Rgba = [85, 85, 85, 255];
    const B: Rgba = [0, 0, 0, 255];

    #[test]
    fn the_four_dmg_grays_are_their_own_shades_in_any_order() {
        assert_eq!(shades(&[B, W, D, L]), [3, 0, 2, 1]);
    }

    #[test]
    fn fewer_colors_keep_the_shade_nearest_their_lightness() {
        assert_eq!(shades(&[W, B]), [0, 3]);
        assert_eq!(shades(&[D, B]), [2, 3]);
        assert_eq!(shades(&[L]), [1]);
    }

    #[test]
    fn colors_that_would_share_a_shade_are_spread_by_lightness() {
        let greens = [
            [0x9B, 0xBC, 0x0F, 0xFF],
            [0x8B, 0xAC, 0x0F, 0xFF],
            [0x30, 0x62, 0x30, 0xFF],
            [0x0F, 0x38, 0x0F, 0xFF],
        ];
        assert_eq!(shades(&greens), [0, 1, 2, 3]);
        let near_white = [255, 255, 250, 255];
        assert_eq!(shades(&[near_white, W]), [3, 0]);
        assert_eq!(shades(&[W, near_white, B]), [0, 1, 3]);
    }
}
