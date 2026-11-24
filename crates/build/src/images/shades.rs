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
//!
//! A pin (`#rrggbb = N`, `NamedPalettes::pins`; LoadHigh's question 993)
//! then overrides its own color's shade, art color or palette color. The
//! rule is chosen and run over every color, pinned ones included, before
//! the pins apply, so a pin never moves another color's shade.

use std::collections::{BTreeMap, HashMap, HashSet};

use gbcli_protocol::{
    ColorShade, DmgPalette, GbcPalette, KnownPalette, NamedPalette, Rgb555, ShadeChoice, ShadePin,
    ShadeRule, ShadeSource,
};

use super::census::{Key, key};
use super::input::PaletteSource;
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
    /// Where the palettes came from, whose `dmg` setting a remedy names.
    pub source: PaletteSource,
    by_key: HashMap<Key, u8>,
    /// The pinned colors' shades, which `by_key` already holds for the art.
    pins: HashMap<Key, u8>,
    /// The colors the target's art draws: `by_key` less the colors only a
    /// discarded color sheet carries (`drawing`).
    drawn: HashSet<Key>,
    mapping: Mapping,
}

/// What a rule makes of a color, including one no image has.
#[derive(Clone, Copy, Debug)]
enum Mapping {
    /// The nearest of four colors, shade 0 first (declared and known).
    Entries(GbcPalette),
    /// Lightness over the art's range; a color past either end takes that
    /// end's shade.
    Stretched { lightest: u32, darkest: u32 },
    /// The plain nearest shade of its lightness.
    Lightness,
}

impl Mapping {
    fn shade(self, c: Rgba) -> u8 {
        match self {
            Mapping::Entries(entries) => nearest_entry(&entries, c),
            Mapping::Stretched { lightest, darkest } => {
                round_shade(lightest.saturating_sub(luminance(c)), lightest - darkest).min(3)
            }
            Mapping::Lightness => nearest(c),
        }
    }
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

    /// `palette`'s entries as one BGP byte: entry `i`'s shade in bits `2i`
    /// and `2i + 1`.
    pub fn palette_byte(&self, palette: &GbcPalette) -> u8 {
        (0..4).fold(0, |byte, i| byte | self.any(palette.0[i]) << (2 * i))
    }

    /// The named palettes as DMG bytes, in their order.
    pub fn named(&self, palettes: &[NamedPalette]) -> Vec<DmgPalette> {
        palettes
            .iter()
            .map(|p| DmgPalette {
                name: p.name.clone(),
                byte: self.palette_byte(&p.colors),
            })
            .collect()
    }

    /// The shade of any color: its pin's, else an art color's own, so a
    /// palette and the art can't disagree, else what the project's rule
    /// makes of it.
    fn any(&self, c: Rgb555) -> u8 {
        let [r, g, b] = c.to_rgb888();
        let c = [r, g, b, 0xFF];
        let k = key(c);
        self.pins
            .get(&k)
            .or_else(|| self.by_key.get(&k))
            .copied()
            .unwrap_or_else(|| self.mapping.shade(c))
    }

    /// This shading with `pins` applied: each pinned color takes its pin's
    /// shade, in the art and in any palette, and every other color keeps
    /// the shade the rule gave it. The pins are checked
    /// (`NamedPalettes::check`): shades 0-3, each color once.
    pub fn pinned(mut self, pins: &[ShadePin]) -> Shading {
        for pin in pins {
            let k = pin.color.channels();
            self.pins.insert(k, pin.shade);
            if let Some(shade) = self.by_key.get_mut(&k) {
                *shade = pin.shade;
            }
        }
        self
    }

    /// This shading with `art` use narrowed to `colors`, the colors the
    /// target's art draws: the rule still chose over every color, so no
    /// shade moves, but a color only a discarded color sheet carries (a
    /// `dmg` override replaces it in a plain build) is no longer art.
    pub fn drawing(mut self, colors: &[Rgba]) -> Shading {
        self.drawn = colors.iter().map(|&c| key(c)).collect();
        self
    }

    /// Every art color's shade and every color of `palettes`', each color
    /// once, sorted by its RGB555 word, with whether art uses it and what
    /// decided it.
    pub fn color_shades(&self, palettes: &[NamedPalette]) -> Vec<ColorShade> {
        let art = self
            .by_key
            .keys()
            .map(|&[r, g, b]| Rgb555::new(r, g, b).expect("a key is five-bit channels"));
        let named = palettes.iter().flat_map(|p| p.colors.0);
        let colors: BTreeMap<u16, Rgb555> = art.chain(named).map(|c| (c.word(), c)).collect();
        colors
            .into_values()
            .map(|color| ColorShade {
                color,
                shade: self.any(color),
                art: self.drawn.contains(&color.channels()),
                rule: match self.pins.contains_key(&color.channels()) {
                    true => ShadeSource::Pinned,
                    false => self.choice.rule.into(),
                },
            })
            .collect()
    }

    /// Whether the stretch moved `c` far from its nearest shade (Ben's
    /// ruling, 2026-11-07: the build warns, `shade_stretched`). Only the
    /// stretched rule moves colors, and never a pinned one, which was
    /// pinned on purpose.
    pub fn moved(&self, c: Rgba) -> Option<Moved> {
        if self.choice.rule != ShadeRule::Stretched || self.pins.contains_key(&key(c)) {
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
/// transparent pixels aside), under `declared` when the palettes (from
/// `source`) have a `dmg` line.
pub(crate) fn choose(
    source: &PaletteSource,
    declared: Option<&GbcPalette>,
    colors: &[Rgba],
) -> Shading {
    let shading = |rule, palette, mapping: Mapping| Shading {
        choice: ShadeChoice { rule, palette },
        source: source.clone(),
        by_key: colors.iter().map(|&c| (key(c), mapping.shade(c))).collect(),
        pins: HashMap::new(),
        drawn: colors.iter().map(|&c| key(c)).collect(),
        mapping,
    };
    if let Some(entries) = declared {
        return shading(ShadeRule::Declared, None, Mapping::Entries(*entries));
    }
    // Without colors no palette holds them: the plain nearest rule, rather
    // than a known palette by vacuous truth.
    if colors.is_empty() {
        return shading(ShadeRule::Nearest, None, Mapping::Lightness);
    }
    for p in KnownPalette::ALL {
        let entries = GbcPalette(known_colors(p).map(|hex| {
            let [r, g, b, _] = rgba(hex);
            Rgb555::from_rgb888(r, g, b)
        }));
        let keys = entries.0.map(Rgb555::channels);
        if colors.iter().all(|&c| keys.contains(&key(c))) {
            return shading(ShadeRule::Known, Some(p), Mapping::Entries(entries));
        }
    }
    let lightest = colors.iter().map(|&c| luminance(c)).max().unwrap_or(0);
    let darkest = colors.iter().map(|&c| luminance(c)).min().unwrap_or(0);
    if lightest == darkest {
        return shading(ShadeRule::Nearest, None, Mapping::Lightness);
    }
    shading(
        ShadeRule::Stretched,
        None,
        Mapping::Stretched { lightest, darkest },
    )
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

    fn file() -> PaletteSource {
        PaletteSource::File("assets/palettes.txt".into())
    }

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
        let s = choose(&file(), declared.as_ref(), colors);
        (s.choice, s.shades(colors))
    }

    fn known(p: KnownPalette) -> ShadeChoice {
        ShadeChoice {
            rule: ShadeRule::Known,
            palette: Some(p),
        }
    }

    fn palette(colors: [Rgba; 4]) -> GbcPalette {
        GbcPalette(colors.map(|c| Rgb555::from_rgb888(c[0], c[1], c[2])))
    }

    /// LoadHigh's example: `palette inverted (black, darkgray, lightgray,
    /// white)` is $1B.
    #[test]
    fn a_palette_byte_holds_entry_is_shade_in_bits_2i_and_2i_plus_1() {
        let s = choose(&file(), Some(&palette([W, L, D, B])), &[]);
        assert_eq!(s.palette_byte(&palette([B, D, L, W])), 0x1B);
        assert_eq!(s.palette_byte(&palette([W, L, D, B])), 0xE4);
    }

    /// Under a `dmg` line a color no image has takes its nearest entry.
    #[test]
    fn a_declared_rule_gives_a_palette_color_no_image_uses_its_nearest_entry() {
        let s = choose(&file(), Some(&palette([W, L, D, B])), &[W]);
        assert_eq!(s.palette_byte(&palette([NEAR_B, W, W, W])), 0x03);
    }

    /// Two entries of one shade are the byte's business, not a refusal.
    #[test]
    fn two_entries_of_one_shade_both_write_that_shade() {
        let s = choose(&file(), Some(&palette([W, L, D, B])), &[]);
        assert_eq!(s.palette_byte(&palette([B, B, W, W])), 0x0F);
    }

    /// Art in a known palette: its own colors map exactly, and another
    /// color takes the nearest of that palette.
    #[test]
    fn a_known_rule_maps_its_colors_and_gives_others_the_nearest() {
        let s = choose(&file(), None, &PEA);
        assert_eq!(s.choice.rule, ShadeRule::Known);
        assert_eq!(s.palette_byte(&palette(PEA)), 0xE4);
        assert_eq!(s.palette_byte(&palette([B, PEA[1], PEA[1], PEA[1]])), 0x57);
    }

    /// The stretch's own colors take what the art took, and one past either
    /// end of the art's range stays on the scale.
    #[test]
    fn a_stretched_rule_clamps_a_palette_color_past_the_arts_range() {
        let (light, dark) = (rgb(0xC0C0C0), rgb(0x404040));
        let s = choose(&file(), None, &[light, dark]);
        assert_eq!(s.choice.rule, ShadeRule::Stretched);
        assert_eq!(s.palette_byte(&palette([light, dark, W, B])), 0b11_00_11_00);
    }

    /// With no art there is no range: the plain nearest shade.
    #[test]
    fn with_no_art_and_no_dmg_line_a_palette_takes_the_nearest_shades() {
        let s = choose(&file(), None, &[]);
        assert_eq!(s.choice.rule, ShadeRule::Nearest);
        assert_eq!(s.palette_byte(&palette([B, D, L, W])), 0x1B);
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
        let s = choose(&file(), None, &[B, NEAR_B]);
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
        let s = choose(&file(), None, &dim);
        assert!(dim.iter().all(|&c| s.moved(c).is_none()), "{s:?}");
    }

    /// Only the stretch moves colors; the other rules never name one.
    #[test]
    fn only_the_stretched_rule_names_moves() {
        let declared = GbcPalette([B, D, L, W].map(|c| Rgb555::from_rgb888(c[0], c[1], c[2])));
        let s = choose(&file(), Some(&declared), &[W, B]);
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

    fn pin(c: Rgba, shade: u8) -> ShadePin {
        ShadePin {
            color: Rgb555::from_rgb888(c[0], c[1], c[2]),
            shade,
        }
    }

    const LIGHT: Rgba = [0xC0, 0xC0, 0xC0, 0xFF];
    const MID: Rgba = [0x80, 0x80, 0x80, 0xFF];
    const DARK: Rgba = [0x40, 0x40, 0x40, 0xFF];

    /// LoadHigh's question 993: the cascade runs over every color, pinned
    /// ones included, and a pin then moves its own color only, so the
    /// stretch's range, the rule and every other color's shade stay.
    #[test]
    fn a_pin_overrides_its_own_color_and_moves_no_other() {
        let colors = [LIGHT, DARK, MID];
        let plain = choose(&file(), None, &colors);
        assert_eq!(plain.shades(&colors), [0, 3, 2]);
        let pinned = choose(&file(), None, &colors).pinned(&[pin(DARK, 0)]);
        assert_eq!(pinned.choice, plain.choice);
        assert_eq!(pinned.shades(&colors), [0, 0, 2]);
        // A color one with the pin in RGB555 is the pinned color.
        assert_eq!(pinned.shade([0x44, 0x44, 0x44, 0xFF]), 0);
        // Under a declared rule too: the pin wins over the dmg line.
        let declared = palette([W, L, D, B]);
        let s = choose(&file(), Some(&declared), &[W, B]).pinned(&[pin(W, 2)]);
        assert_eq!(s.shades(&[W, B]), [2, 3]);
        assert_eq!(s.choice.rule, ShadeRule::Declared);
    }

    /// A pin reaches a palette's color whether or not the art uses it, and
    /// a pinned color nothing uses moves nothing.
    #[test]
    fn a_pin_reaches_a_palette_color_the_art_does_not_use() {
        let s = choose(&file(), None, &[W, L]).pinned(&[pin(B, 1), pin(L, 3), pin(MID, 2)]);
        assert_eq!(s.choice.rule, ShadeRule::Known);
        // W 0, L pinned 3, D known 2, B pinned 1.
        assert_eq!(s.palette_byte(&palette([W, L, D, B])), 0b01_10_11_00);
        assert_eq!(s.shades(&[W, L]), [0, 3]);
    }

    /// A pinned color was pinned on purpose: the stretch names no move.
    #[test]
    fn a_pinned_color_is_never_named_as_moved() {
        let s = choose(&file(), None, &[B, NEAR_B]).pinned(&[pin(NEAR_B, 0)]);
        assert_eq!(s.shades(&[B, NEAR_B]), [3, 0]);
        assert_eq!((s.moved(B), s.moved(NEAR_B)), (None, None));
    }

    /// Every art color and every named palette's color, once each, sorted
    /// by color (the RGB555 word), with whether art uses it and the rule
    /// that decided it; a pinned color nothing uses isn't listed.
    #[test]
    fn color_shades_list_every_art_and_palette_color_with_its_source() {
        let s = choose(&file(), None, &[LIGHT, DARK, MID]).pinned(&[pin(MID, 1), pin(W, 0)]);
        let named = [
            NamedPalette {
                name: "a".into(),
                colors: palette([B, DARK, DARK, MID]),
            },
            NamedPalette {
                name: "b".into(),
                colors: palette([LIGHT, LIGHT, LIGHT, LIGHT]),
            },
        ];
        let c = |c: Rgba| Rgb555::from_rgb888(c[0], c[1], c[2]);
        let listed = s.color_shades(&named);
        assert_eq!(
            listed,
            [
                ColorShade {
                    color: c(B),
                    shade: 3,
                    art: false,
                    rule: ShadeSource::Stretched
                },
                ColorShade {
                    color: c(DARK),
                    shade: 3,
                    art: true,
                    rule: ShadeSource::Stretched
                },
                ColorShade {
                    color: c(MID),
                    shade: 1,
                    art: true,
                    rule: ShadeSource::Pinned
                },
                ColorShade {
                    color: c(LIGHT),
                    shade: 0,
                    art: true,
                    rule: ShadeSource::Stretched
                },
            ]
        );
        let words: Vec<u16> = listed.iter().map(|l| l.color.word()).collect();
        assert!(words.is_sorted(), "{words:?}");
        // Under the known rule, a palette color the art lacks is known's,
        // and only the art's color says art uses it.
        let s = choose(&file(), None, &[W]);
        let named = [NamedPalette {
            name: "a".into(),
            colors: palette([W, W, W, D]),
        }];
        let shades = s.color_shades(&named);
        let rules: Vec<ShadeSource> = shades.iter().map(|l| l.rule).collect();
        let art: Vec<bool> = shades.iter().map(|l| l.art).collect();
        assert_eq!(rules, [ShadeSource::Known, ShadeSource::Known]);
        // D sorts first; it is the palette's alone, W is the art's too.
        assert_eq!(art, [false, true]);
    }

    /// The art flag is membership in the art's colors, not the color's
    /// source: a palette-only color under a pinned rule is still no art's,
    /// and a color both the art and a palette name is the art's.
    #[test]
    fn art_is_membership_in_the_arts_colors_not_the_colors_rule() {
        let s = choose(&file(), None, &[W, LIGHT]).pinned(&[pin(MID, 2)]);
        let named = [NamedPalette {
            name: "a".into(),
            colors: palette([MID, LIGHT, W, B]),
        }];
        let listed = s.color_shades(&named);
        let art: Vec<(u16, bool, ShadeSource)> = listed
            .iter()
            .map(|l| (l.color.word(), l.art, l.rule))
            .collect();
        let c = |c: Rgba| Rgb555::from_rgb888(c[0], c[1], c[2]).word();
        assert_eq!(
            art,
            [
                (c(B), false, ShadeSource::Stretched),
                (c(MID), false, ShadeSource::Pinned),
                (c(LIGHT), true, ShadeSource::Stretched),
                (c(W), true, ShadeSource::Stretched),
            ]
        );
    }

    /// A color only a discarded color sheet carries keeps its shade (the
    /// rule chose over it) but is not art.
    #[test]
    fn a_color_only_a_discarded_sheet_carries_is_not_art() {
        let kept = choose(&file(), None, &[LIGHT, DARK, MID]);
        let shades: Vec<u8> = kept.color_shades(&[]).iter().map(|c| c.shade).collect();
        let s = kept.drawing(&[DARK]);
        let listed = s.color_shades(&[]);
        assert_eq!(listed.iter().map(|c| c.shade).collect::<Vec<_>>(), shades);
        let art: Vec<bool> = listed.iter().map(|c| c.art).collect();
        assert_eq!(art, [true, false, false], "{listed:?}");
    }
}
