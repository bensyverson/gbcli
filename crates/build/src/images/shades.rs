//! Which DMG shade each color becomes. The DMG shows four shades, 0
//! (lightest) to 3, and images may be drawn in any colors. Ben's rulings
//! (2026-11-07): the shade is decided once for the whole project, so one
//! color is one shade in every image, by the first rule that applies:
//! 1. declared: the `dmg` line of `assets/palettes.txt` lists shades 0-3,
//!    and each color takes its nearest entry's shade, in RGB555;
//! 2. known: every color belongs to one known palette, which maps exactly;
//! 3. otherwise lightness is stretched over the project's colors (lightest
//!    shade 0, darkest shade 3), then each takes the nearest shade; with one
//!    lightness (or no colors at all), nothing to stretch, the plain
//!    nearest shade.
//!
//! Two colors may take one shade: BGP and OBP map indices to shades, so the
//! hardware shows them alike, and the build warns (`shared`).

use std::collections::HashMap;

use gbcli_protocol::{GbcPalette, KnownPalette, ShadeChoice, ShadeRule};

use super::census::{Key, key};
use super::pixels::Rgba;

/// Lightness as rgbgfx weighs it: 2126 R + 7152 G + 722 B.
pub(crate) fn luminance(c: Rgba) -> u32 {
    2126 * u32::from(c[0]) + 7152 * u32::from(c[1]) + 722 * u32::from(c[2])
}

const WHITE_LUMINANCE: u32 = 255 * 10_000;

/// A known palette's colors, shade 0 first.
fn known_colors(p: KnownPalette) -> [u32; 4] {
    match p {
        KnownPalette::DmgGrays => [0xFFFFFF, 0xAAAAAA, 0x555555, 0x000000],
        KnownPalette::PeaGreens => [0x9BBC0F, 0x8BAC0F, 0x306230, 0x0F380F],
        KnownPalette::GbStudioGreens => [0xE0F8CF, 0x86C06C, 0x306850, 0x071821],
        // SameBoy, Core/display.c, GB_PALETTE_MGB (darkest first there;
        // its fifth entry is the screen off).
        KnownPalette::PocketGrays => [0xC2CE93, 0x818D66, 0x3A4C3A, 0x07100E],
    }
}

fn rgba(hex: u32) -> Rgba {
    [(hex >> 16) as u8, (hex >> 8) as u8, hex as u8, 0xFF]
}

/// The project's shades: the rule that chose them, and each color's.
#[derive(Debug)]
pub(crate) struct Shading {
    pub choice: ShadeChoice,
    by_key: HashMap<Key, u8>,
}

/// A color the stretch put two or more shades from its plain nearest one.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct Moved {
    pub nearest: u8,
    pub given: u8,
}

/// How far from its nearest shade the stretch may put a color unremarked.
const MOVE_UNREMARKED: u8 = 1;

impl Shading {
    /// The shade of each of `colors`, which `choose` was given (by RGB555).
    pub fn shades(&self, colors: &[Rgba]) -> Vec<u8> {
        colors.iter().map(|&c| self.shade(c)).collect()
    }

    /// The shade of `c`, a color `choose` was given (by RGB555).
    pub fn shade(&self, c: Rgba) -> u8 {
        self.by_key[&key(c)]
    }

    /// Whether the stretch moved `c` far from its nearest shade (Ben's
    /// ruling, 2026-11-07: the build warns, `shade_stretched`). Only the
    /// stretched rule moves colors.
    pub fn moved(&self, c: Rgba) -> Option<Moved> {
        if self.choice.rule != ShadeRule::Stretched {
            return None;
        }
        let (nearest, given) = (nearest(c), self.by_key[&key(c)]);
        (nearest.abs_diff(given) > MOVE_UNREMARKED).then_some(Moved { nearest, given })
    }
}

/// The plain nearest shade of `c`'s lightness.
fn nearest(c: Rgba) -> u8 {
    round_shade(WHITE_LUMINANCE - luminance(c), WHITE_LUMINANCE)
}

/// Chooses the shades of `colors`, every color in the project (fully
/// transparent pixels aside), under `declared` when `assets/palettes.txt`
/// has a `dmg` line.
pub(crate) fn choose(declared: Option<&GbcPalette>, colors: &[Rgba]) -> Shading {
    let shading = |rule, palette, shade: &dyn Fn(Rgba) -> u8| Shading {
        choice: ShadeChoice { rule, palette },
        by_key: colors.iter().map(|&c| (key(c), shade(c))).collect(),
    };
    if let Some(entries) = declared {
        return shading(ShadeRule::Declared, None, &|c| nearest_entry(entries, c));
    }
    // Without colors no palette holds them: the plain nearest rule, rather
    // than a known palette by vacuous truth.
    if colors.is_empty() {
        return shading(ShadeRule::Nearest, None, &nearest);
    }
    for p in KnownPalette::ALL {
        let keys = known_colors(p).map(|hex| key(rgba(hex)));
        if colors.iter().all(|&c| keys.contains(&key(c))) {
            let shade = |c| keys.iter().position(|&k| k == key(c)).unwrap() as u8;
            return shading(ShadeRule::Known, Some(p), &shade);
        }
    }
    let lightest = colors.iter().map(|&c| luminance(c)).max().unwrap_or(0);
    let darkest = colors.iter().map(|&c| luminance(c)).min().unwrap_or(0);
    if lightest == darkest {
        return shading(ShadeRule::Nearest, None, &nearest);
    }
    shading(ShadeRule::Stretched, None, &|c| {
        round_shade(lightest - luminance(c), lightest - darkest)
    })
}

/// `dark` of `span` as the nearest of shades 0-3.
fn round_shade(dark: u32, span: u32) -> u8 {
    ((u64::from(dark) * 6 + u64::from(span)) / (u64::from(span) * 2)) as u8
}

/// The shade of the declared entry nearest `c`, both in RGB555 as rgbgfx
/// reads them, so a color one with an entry to rgbgfx takes its shade
/// (ties go lighter).
fn nearest_entry(entries: &GbcPalette, c: Rgba) -> u8 {
    let c = key(c);
    let distance = |s: u8| -> u32 {
        let e = entries.0[usize::from(s)].channels();
        (0..3)
            .map(|i| (i32::from(e[i]) - i32::from(c[i])).pow(2) as u32)
            .sum()
    };
    (0..4u8).min_by_key(|&s| distance(s)).unwrap()
}

/// The DMG's name for `shade`.
pub(crate) fn name(shade: u8) -> &'static str {
    ["white", "light gray", "dark gray", "black"][usize::from(shade)]
}

#[cfg(test)]
mod tests {
    use super::*;
    use gbcli_protocol::Rgb555;

    const W: Rgba = [255, 255, 255, 255];
    const L: Rgba = [170, 170, 170, 255];
    const D: Rgba = [85, 85, 85, 255];
    const B: Rgba = [0, 0, 0, 255];

    const NEAR_B: Rgba = [0x10, 0x10, 0x10, 0xFF];
    const PEA: [Rgba; 4] = [
        [0x9B, 0xBC, 0x0F, 0xFF],
        [0x8B, 0xAC, 0x0F, 0xFF],
        [0x30, 0x62, 0x30, 0xFF],
        [0x0F, 0x38, 0x0F, 0xFF],
    ];

    fn rgb(hex: u32) -> Rgba {
        [(hex >> 16) as u8, (hex >> 8) as u8, hex as u8, 0xFF]
    }

    /// The rule `choose` picked for `colors`, and their shades.
    fn chosen(declared: Option<[Rgba; 4]>, colors: &[Rgba]) -> (ShadeChoice, Vec<u8>) {
        let declared =
            declared.map(|d| GbcPalette(d.map(|c| Rgb555::from_rgb888(c[0], c[1], c[2]))));
        let s = choose(declared.as_ref(), colors);
        (s.choice, s.shades(colors))
    }

    fn known(p: KnownPalette) -> ShadeChoice {
        ShadeChoice {
            rule: ShadeRule::Known,
            palette: Some(p),
        }
    }

    fn rule(rule: ShadeRule) -> ShadeChoice {
        ShadeChoice {
            rule,
            palette: None,
        }
    }

    #[test]
    fn the_four_dmg_grays_are_known_in_any_order_and_in_rgb555() {
        let grays = known(KnownPalette::DmgGrays);
        assert_eq!(chosen(None, &[B, W, D, L]), (grays, vec![3, 0, 2, 1]));
        assert_eq!(chosen(None, &[D, B]), (grays, vec![2, 3]));
        // The same colors to rgbgfx (5 bits a channel): #a8a8a8, #545454.
        let variants = [rgb(0xA8A8A8), rgb(0x545454), rgb(0x040404)];
        assert_eq!(chosen(None, &variants), (grays, vec![1, 2, 3]));
    }

    #[test]
    fn every_known_palette_maps_exactly_even_in_part() {
        assert_eq!(
            chosen(None, &PEA),
            (known(KnownPalette::PeaGreens), vec![0, 1, 2, 3])
        );
        assert_eq!(
            chosen(None, &[PEA[3], PEA[2]]),
            (known(KnownPalette::PeaGreens), vec![3, 2])
        );
        let studio = [0xE0F8CF, 0x86C06C, 0x306850, 0x071821].map(rgb);
        assert_eq!(
            chosen(None, &studio),
            (known(KnownPalette::GbStudioGreens), vec![0, 1, 2, 3])
        );
        let pocket = [0xC2CE93, 0x818D66, 0x3A4C3A, 0x07100E].map(rgb);
        assert_eq!(
            chosen(None, &pocket),
            (known(KnownPalette::PocketGrays), vec![0, 1, 2, 3])
        );
    }

    #[test]
    fn a_declared_palette_wins_and_each_color_takes_its_nearest_entry() {
        let backward = [B, D, L, W];
        assert_eq!(
            chosen(Some(backward), &[W, L, D, B]),
            (rule(ShadeRule::Declared), vec![3, 2, 1, 0])
        );
        // Off the declared colors: the nearest one's shade.
        let declared = [0xE0F0E0, 0xA0B0A0, 0x506050, 0x102010].map(rgb);
        assert_eq!(
            chosen(Some(declared), &[W, L, D, B, NEAR_B]).1,
            [0, 1, 2, 3, 3]
        );
    }

    /// #070707 is #000000 to rgbgfx (5 bits a channel), so it takes that
    /// entry's shade, though in 8-bit RGB it is nearer #080808.
    #[test]
    fn a_color_one_in_rgb555_with_an_entry_takes_that_entrys_shade() {
        let declared = [0xFFFFFF, 0xAAAAAA, 0x080808, 0x000000].map(rgb);
        assert_eq!(chosen(Some(declared), &[rgb(0x070707)]).1, [3]);
    }

    #[test]
    fn colors_from_two_palettes_are_stretched_over_the_project() {
        // A gray and a pea green: no one known palette holds both.
        let (choice, _) = chosen(None, &[W, PEA[3]]);
        assert_eq!(choice, rule(ShadeRule::Stretched));
        // A dim palette stretches to all four shades.
        let dim = [0x808080, 0x666666, 0x4D4D4D, 0x333333].map(rgb);
        assert_eq!(
            chosen(None, &dim),
            (rule(ShadeRule::Stretched), vec![0, 1, 2, 3])
        );
        // Beside white, near-black is nearest black, as black is.
        assert_eq!(chosen(None, &[W, B, NEAR_B]).1, [0, 3, 3]);
    }

    /// The stretch's consequence for a project of two near colors: the
    /// lighter becomes white and the darker black, however near they are,
    /// so the color moved from its nearest shade by two or more is named
    /// (Ben's ruling: the build warns, `shade_stretched`).
    #[test]
    fn two_near_colors_alone_are_stretched_apart_and_the_far_move_is_named() {
        let s = choose(None, &[B, NEAR_B]);
        assert_eq!(s.shades(&[B, NEAR_B]), [3, 0]);
        assert_eq!(
            s.moved(NEAR_B),
            Some(Moved {
                nearest: 3,
                given: 0
            })
        );
        assert_eq!(s.moved(B), None);
    }

    /// A move of one shade is the stretch doing its job: not named.
    #[test]
    fn a_dim_palette_stretched_by_one_shade_names_no_move() {
        let dim = [0x808080, 0x666666, 0x4D4D4D, 0x333333].map(rgb);
        let s = choose(None, &dim);
        assert!(dim.iter().all(|&c| s.moved(c).is_none()), "{s:?}");
    }

    /// Only the stretch moves colors; the other rules never name one.
    #[test]
    fn only_the_stretched_rule_names_moves() {
        let declared = GbcPalette([B, D, L, W].map(|c| Rgb555::from_rgb888(c[0], c[1], c[2])));
        let s = choose(Some(&declared), &[W, B]);
        assert_eq!((s.moved(W), s.moved(B)), (None, None));
    }

    #[test]
    fn one_lightness_keeps_the_plain_nearest_shade() {
        assert_eq!(
            chosen(None, &[rgb(0x606060)]),
            (rule(ShadeRule::Nearest), vec![2])
        );
    }

    /// No colors (every pixel transparent): no palette holds them, so no
    /// known palette is claimed.
    #[test]
    fn no_colors_keep_the_plain_nearest_rule() {
        assert_eq!(chosen(None, &[]), (rule(ShadeRule::Nearest), vec![]));
    }
}
