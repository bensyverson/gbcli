//! Colors that share a DMG shade (Ben's rulings, 2026-11-07): each keeps
//! the shade the project's rule gives it (`shades::choose`), so they draw
//! alike, and the build warns (`shared_shade`), naming them, the shade, the
//! rule and the tiles of all but the most used, in case the artist meant
//! the difference. A shade whose colors fall into groups no image joins
//! gets one project-level warning too, listing each group with the images
//! holding it (`across`).
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

use super::census::{self, Key, error, key, tiles_where};
use super::pixels::{Pixels, Rgba, hex};
use super::shades::{Moved, Shading, name};

const REMEDY: &str = "make them distinct if the difference matters: recolor one, or declare \
                      each shade's color on the dmg line of assets/palettes.txt (gbcli help images); if it \
                      doesn't, draw them in one color";

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
        (ShadeRule::Declared, _) => "declared (the dmg line of assets/palettes.txt)".into(),
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
                REMEDY,
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
                    "declare the dmg line of assets/palettes.txt to pick the shades yourself \
                     (gbcli help images); or add colors that span the shades, so the stretch has less to do",
                )
            })
        })
        .collect()
}

/// One project-level `shared_shade` warning (no image) a shade whose colors
/// fall into two or more groups no image joins (leaves GTXXyr, L3PdEW).
/// Colors one image holds together, or chained through shared colors, are
/// one group, which the images' own warnings (`find`) already name; colors
/// in different groups draw alike yet no image's warning names them
/// together. The warning lists each group (colors joined by `/`) with the
/// images holding it, and counts the groups. `images` are every image read with its name, `colors`
/// the project's colors as `shades::choose` was given them.
pub(crate) fn across(
    images: &[(&str, &Pixels)],
    colors: &[Rgba],
    shading: &Shading,
) -> Vec<ImageProblem> {
    let held: Vec<Vec<Key>> = images
        .iter()
        .map(|(_, p)| census::colors(p).iter().map(|&(c, _)| key(c)).collect())
        .collect();
    let shade_of = shading.shades(colors);
    let mut warnings = Vec::new();
    for shade in 0..4u8 {
        let group: Vec<Rgba> = colors
            .iter()
            .zip(&shade_of)
            .filter(|&(_, &s)| s == shade)
            .map(|(&c, _)| c)
            .collect();
        let clusters = clusters(&group, &held);
        if clusters.len() < 2 {
            continue;
        }
        let listed: Vec<String> = clusters
            .iter()
            .map(|cluster| {
                let names: Vec<&str> = images
                    .iter()
                    .zip(&held)
                    .filter(|(_, k)| cluster.iter().any(|&c| k.contains(&key(c))))
                    .map(|((name, _), _)| *name)
                    .collect();
                let hexes: Vec<String> = cluster.iter().map(|&c| hex(c)).collect();
                format!("{} ({})", hexes.join("/"), names.join(", "))
            })
            .collect();
        let message = format!(
            "the project draws {} in one shade, shade {shade} ({}), by its shade rule, {}, so \
             they look alike on screen; no image holds colors from two of these groups, so no \
             image's own shared_shade warning names them together",
            and(&listed),
            name(shade),
            rule(shading.choice),
        );
        warnings.push(ImageProblem {
            image: None,
            severity: Severity::Warning,
            ..error(
                "",
                ImageLimit::SharedShade,
                clusters.len(),
                1,
                message,
                REMEDY,
            )
        });
    }
    warnings
}

/// `group`'s colors split into the groups images join: two colors are in
/// one group when some image in `held` holds both, or through a chain of
/// such colors. Groups and their colors keep `group`'s order.
fn clusters(group: &[Rgba], held: &[Vec<Key>]) -> Vec<Vec<Rgba>> {
    let mut of: Vec<usize> = (0..group.len()).collect();
    for keys in held {
        let joined: Vec<usize> = (0..group.len())
            .filter(|&i| keys.contains(&key(group[i])))
            .map(|i| of[i])
            .collect();
        let Some(&into) = joined.first() else {
            continue;
        };
        for o in &mut of {
            if joined.contains(o) {
                *o = into;
            }
        }
    }
    let mut out: Vec<(usize, Vec<Rgba>)> = Vec::new();
    for (&c, &id) in group.iter().zip(&of) {
        match out.iter_mut().find(|(g, _)| *g == id) {
            Some((_, v)) => v.push(c),
            None => out.push((id, vec![c])),
        }
    }
    out.into_iter().map(|(_, v)| v).collect()
}

/// "a", "a and b", "a, b and c".
pub(crate) fn and(items: &[String]) -> String {
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
        assert!(w.remedy.contains("assets/palettes.txt"), "{}", w.remedy);
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

    /// Black in one image, near-black in another, both black: one warning
    /// for the project naming each with its image; black and near-black in
    /// one image are that image's warning, not the project's.
    #[test]
    fn colors_in_one_shade_only_across_images_are_one_project_warning() {
        let colors = [W, B, NEAR_B];
        let shading = super::super::shades::choose(None, &colors);
        let sky = pixels(vec![W, B], 2);
        let bug = pixels(vec![NEAR_B, NEAR_B], 2);
        let [w] = &across(&[("sky.png", &sky), ("bug.png", &bug)], &colors, &shading)[..] else {
            panic!("one warning")
        };
        assert_eq!((w.image.as_deref(), w.count, w.max), (None, 2, 1));
        assert_eq!(w.severity, Severity::Warning);
        assert!(
            w.message
                .contains("#000000 (sky.png) and #101010 (bug.png)"),
            "{}",
            w.message
        );
        let cave = pixels(vec![B, NEAR_B], 2);
        assert!(across(&[("cave.png", &cave), ("bug.png", &bug)], &colors, &shading).is_empty());
    }

    /// X and Y in a, Y and Z in b, W in c, one shade: the chain through Y
    /// makes X, Y and Z one group, so two groups, not three.
    #[test]
    fn colors_chained_through_a_shared_color_are_one_group() {
        let (x, y, z, w) = (B, NEAR_B, [0x18, 0x18, 0x18, 255], [0x08, 0x08, 0x08, 255]);
        let colors = [W, x, y, z, w];
        let shading = super::super::shades::choose(None, &colors);
        assert_eq!(shading.shades(&colors)[1..], [3, 3, 3, 3]);
        let a = pixels(vec![W, x, y, y], 2);
        let b = pixels(vec![y, z], 2);
        let c = pixels(vec![w, w], 2);
        let [warning] = &across(
            &[("a.png", &a), ("b.png", &b), ("c.png", &c)],
            &colors,
            &shading,
        )[..] else {
            panic!("one warning")
        };
        assert_eq!(warning.count, 2);
        assert!(
            warning
                .message
                .contains("#000000/#101010/#181818 (a.png, b.png) and #080808 (c.png)"),
            "{}",
            warning.message
        );
    }

    #[test]
    fn a_shade_held_whole_by_one_image_is_no_project_warning() {
        let colors = [W, B, NEAR_B];
        let shading = super::super::shades::choose(None, &colors);
        let bug = pixels(vec![NEAR_B, NEAR_B], 2);
        let cave = pixels(vec![B, NEAR_B], 2);
        assert!(across(&[("cave.png", &cave), ("bug.png", &bug)], &colors, &shading).is_empty());
    }
}
