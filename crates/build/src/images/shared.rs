//! Colors that share a DMG shade (Ben's rulings, 2026-11-07): each keeps
//! the shade the project's rule gives it (`shades::choose`), so they draw
//! alike, and the build warns (`shared_shade`), naming them, the shade, the
//! rule and the tiles of all but the most used, in case the artist meant
//! the difference.
//!
//! rgbgfx's `-c` gives each palette slot one color, so two colors can't be
//! asked to share an index. An image with a shared shade therefore reaches
//! rgbgfx with its pixels merged: each color drawn as the most used color of
//! its shade, written to a PNG beside the data (`write`), converted, then
//! removed. gbcli's plan already gives them one index, and `agree` still
//! compares what rgbgfx wrote with that plan.

use std::fs;
use std::path::Path;

use gbcli_protocol::{ImageLimit, ImageProblem, Severity, ShadeChoice, ShadeRule};

use super::census::{Key, error, key, tiles_where};
use super::pixels::{Pixels, Rgba, hex};
use super::shades::{Moved, Shading, name};

/// What sharing means for one image.
#[derive(Debug, Default)]
pub(crate) struct Shared {
    /// Each color that shares a shade but isn't its most used, by RGB555
    /// key, and the color rgbgfx is to see instead.
    pub merges: Vec<(Key, Rgba)>,
    /// One `shared_shade` warning a shared shade, lightest first.
    pub warnings: Vec<ImageProblem>,
}

/// The rule as a warning words it.
fn rule(choice: ShadeChoice) -> String {
    match (choice.rule, choice.palette) {
        (ShadeRule::Declared, _) => "declared (assets/palette.txt)".into(),
        (ShadeRule::Known, Some(p)) => format!("known ({})", p.name()),
        (ShadeRule::Known, None) => "known".into(),
        (ShadeRule::Stretched, _) => {
            "stretched (lightness rescaled over the project's colors, then the nearest shade)"
                .into()
        }
        (ShadeRule::Nearest, _) => "nearest (the shade nearest its lightness)".into(),
    }
}

/// The shared shades among `colors` (most used first, as `census::colors`
/// gives them), whose shades are `shades`, chosen by `choice`.
pub(crate) fn find(
    image: &str,
    pixels: &Pixels,
    colors: &[(Rgba, u32)],
    shades: &[u8],
    choice: ShadeChoice,
) -> Shared {
    let mut shared = Shared::default();
    for shade in 0..4u8 {
        let group: Vec<(Rgba, u32)> = colors
            .iter()
            .zip(shades)
            .filter(|&(_, &s)| s == shade)
            .map(|(&c, _)| c)
            .collect();
        let [(most, _), rest @ ..] = &group[..] else {
            continue;
        };
        if rest.is_empty() {
            continue;
        }
        shared
            .merges
            .extend(rest.iter().map(|&(c, _)| (key(c), *most)));
        let tiles = tiles_where(pixels, |p| {
            p[3] != 0 && rest.iter().any(|&(c, _)| key(c) == key(p))
        });
        let listed: Vec<String> = group
            .iter()
            .map(|&(c, n)| format!("{} ({n} px)", hex(c)))
            .collect();
        let others: Vec<String> = rest.iter().map(|&(c, _)| hex(c)).collect();
        let message = format!(
            "{image} draws {} in one shade, shade {shade} ({}), by the project's shade rule, \
             {}, so they look alike. {} {} in {} tile(s) (listed)",
            and(&listed),
            name(shade),
            rule(choice),
            and(&others),
            if others.len() == 1 { "is" } else { "are" },
            tiles.len()
        );
        shared.warnings.push(ImageProblem {
            severity: Severity::Warning,
            tiles,
            ..error(
                image,
                ImageLimit::SharedShade,
                group.len(),
                1,
                message,
                "make them distinct if the difference matters: recolor one, or declare each \
                 shade's color in assets/palette.txt (gbcli help images); if it doesn't, draw \
                 them in one color",
            )
        });
    }
    shared
}

/// One `shade_stretched` warning for each of `colors` (as `census::colors`
/// gives them) the stretch put two or more shades from its nearest (Ben's
/// ruling, 2026-11-07), naming both shades and the tiles holding it.
pub(crate) fn stretched(
    image: &str,
    pixels: &Pixels,
    colors: &[(Rgba, u32)],
    shading: &Shading,
) -> Vec<ImageProblem> {
    colors
        .iter()
        .filter_map(|&(c, n)| {
            let Moved { nearest, given } = shading.moved(c)?;
            let tiles = tiles_where(pixels, |p| p[3] != 0 && key(p) == key(c));
            let message = format!(
                "{image} draws {} ({n} px) at nearest shade {nearest} ({}), drawn as {given} \
                 ({}): the project's shade rule is stretched (lightness rescaled over the \
                 project's colors, the lightest shade 0 and the darkest shade 3). It is in {} \
                 tile(s) (listed)",
                hex(c),
                name(nearest),
                name(given),
                tiles.len()
            );
            Some(ImageProblem {
                severity: Severity::Warning,
                tiles,
                ..error(
                    image,
                    ImageLimit::ShadeStretched,
                    usize::from(nearest.abs_diff(given)),
                    1,
                    message,
                    "declare assets/palette.txt to pick the shades yourself (gbcli help \
                     images); or add colors that span the shades, so the stretch has less to do",
                )
            })
        })
        .collect()
}

/// "a", "a and b", "a, b and c".
fn and(items: &[String]) -> String {
    match items {
        [] => String::new(),
        [one] => one.clone(),
        [head @ .., last] => format!("{} and {last}", head.join(", ")),
    }
}

/// Writes `pixels` to `path` as an 8-bit RGBA PNG, each color in `merges`
/// drawn as the color it merges into, alpha kept.
pub(crate) fn write(pixels: &Pixels, merges: &[(Key, Rgba)], path: &Path) -> std::io::Result<()> {
    let mut data = Vec::with_capacity(pixels.rgba.len() * 4);
    for &p in &pixels.rgba {
        let into = merges.iter().find(|(k, _)| p[3] != 0 && *k == key(p));
        let p = into.map_or(p, |&(_, c)| [c[0], c[1], c[2], p[3]]);
        data.extend_from_slice(&p);
    }
    let file = fs::File::create(path)?;
    let mut encoder = png::Encoder::new(std::io::BufWriter::new(file), pixels.width, pixels.height);
    encoder.set_color(png::ColorType::Rgba);
    encoder.set_depth(png::BitDepth::Eight);
    let mut writer = encoder.write_header().map_err(std::io::Error::other)?;
    writer
        .write_image_data(&data)
        .map_err(std::io::Error::other)?;
    writer.finish().map_err(std::io::Error::other)
}

#[cfg(test)]
mod tests {
    use super::*;

    const W: Rgba = [255, 255, 255, 255];
    const B: Rgba = [0, 0, 0, 255];
    const NEAR_B: Rgba = [0x10, 0x10, 0x10, 255];
    const STRETCHED: ShadeChoice = ShadeChoice {
        rule: ShadeRule::Stretched,
        palette: None,
    };

    fn pixels(rgba: Vec<Rgba>, width: u32) -> Pixels {
        let height = rgba.len() as u32 / width;
        Pixels {
            width,
            height,
            rgba,
        }
    }

    #[test]
    fn a_lone_color_per_shade_shares_nothing() {
        let p = pixels(vec![W; 64], 8);
        let s = find("a.png", &p, &[(W, 60), (B, 4)], &[0, 3], STRETCHED);
        assert!(s.merges.is_empty() && s.warnings.is_empty(), "{s:?}");
    }

    #[test]
    fn the_less_used_colors_merge_into_the_most_used_and_are_listed() {
        // 16x8: the left tile black, the right near-black.
        let rgba = (0..128)
            .map(|i| if i % 16 < 8 { B } else { NEAR_B })
            .collect();
        let p = pixels(rgba, 16);
        let s = find("a.png", &p, &[(B, 64), (NEAR_B, 64)], &[3, 3], STRETCHED);
        assert_eq!(s.merges, [(key(NEAR_B), B)]);
        let [w] = &s.warnings[..] else {
            panic!("{s:?}")
        };
        assert_eq!(w.tiles, [gbcli_protocol::TileAt { x: 1, y: 0 }]);
        assert!(
            w.message
                .contains("#000000 (64 px) and #101010 (64 px) in one shade, shade 3 (black)"),
            "{}",
            w.message
        );
        assert!(w.message.contains("#101010 is in 1 tile"), "{}", w.message);
        assert!(w.message.contains("rule, stretched"), "{}", w.message);
        assert!(w.remedy.contains("assets/palette.txt"), "{}", w.remedy);
    }

    #[test]
    fn the_warning_names_a_known_palette_by_its_wire_name() {
        let choice = ShadeChoice {
            rule: ShadeRule::Known,
            palette: Some(gbcli_protocol::KnownPalette::PeaGreens),
        };
        assert_eq!(rule(choice), "known (pea_greens)");
    }

    #[test]
    fn lists_read_as_english() {
        let s = |v: &[&str]| v.iter().map(|s| s.to_string()).collect::<Vec<_>>();
        assert_eq!(and(&s(&["a"])), "a");
        assert_eq!(and(&s(&["a", "b"])), "a and b");
        assert_eq!(and(&s(&["a", "b", "c"])), "a, b and c");
    }
}
