//! `assets/palettes.txt`: the project's named palettes and which images
//! they color (the GBC images plan, revision 2), read into the protocol's
//! [`NamedPalettes`]. One palette a line, `name: c0 c1 c2 c3`, index 0 (the
//! DMG's lightest) first, each color `#rrggbb` or CSS's shorthand `#rgb`;
//! one assignment a line, `bg/town.png = forest`, the image by its path
//! under `assets/`. The reserved name `dmg` lists the colors that mean
//! shades 0-3, the first rule of the shade cascade (`shades::choose`).
//!
//! `#` then whitespace (or `#` at the end of a line) starts a comment,
//! anywhere a word could start; any other word starting `#` is a color, so
//! a fifth color after four is an error, not a comment. Blank lines are
//! skipped, and so is a leading UTF-8 byte order mark. Every color is
//! reduced to RGB555 as rgbgfx reads a PNG, each channel's top 5 bits.

use std::collections::BTreeMap;
use std::fs;
use std::path::Path;

use gbcli_protocol::{GbcPalette, ImageLimit, ImageProblem, NamedPalette, NamedPalettes, Rgb555};

use super::census::error;

/// Where the named palettes live, relative to the project.
pub(crate) const PATH: &str = "assets/palettes.txt";

/// The reserved name: the DMG shades, not a GBC palette.
pub(crate) const DMG: &str = "dmg";

const REMEDY: &str = "one palette a line, `name: c0 c1 c2 c3` (#rrggbb or #rgb, index 0 \
                      lightest first; `dmg:` for the colors of DMG shades 0-3), or one image a \
                      line, `bg/town.png = name`; '#' and a space or tab starts a comment \
                      (gbcli help images)";

/// Why a palette file was refused.
#[derive(Debug, PartialEq, Eq)]
pub(crate) struct Bad {
    /// The line at fault, 1-based; none when the file as a whole is.
    pub line: Option<u32>,
    /// What the line or file holds, against [`Bad::max`]: the colors on a
    /// palette line, else 1 against 1.
    pub count: u32,
    pub max: u32,
    pub message: String,
}

impl Bad {
    fn at(line: u32, message: String) -> Bad {
        Bad {
            line: Some(line),
            count: 1,
            max: 1,
            message,
        }
    }
}

/// A file's palettes, with the line of each assignment, for the checks
/// that need the project's images.
#[derive(Debug, Default, PartialEq, Eq)]
pub(crate) struct Parsed {
    pub named: NamedPalettes,
    /// Each assigned image's line.
    pub lines: BTreeMap<String, u32>,
}

/// The project's palettes: none without the file, else what it says or
/// the problem with it. `images` are the project's images by their path
/// under `assets/` (`bg/town.png`), which assignments must name; `shown`
/// is the file's name in messages.
pub(crate) fn read(
    project: &Path,
    shown: &str,
    images: &[String],
) -> Result<Option<NamedPalettes>, ImageProblem> {
    let path = project.join(PATH);
    let whole = |message: &str| {
        problem(
            shown,
            Bad {
                line: None,
                count: 1,
                max: 1,
                message: message.into(),
            },
        )
    };
    if path.is_dir() {
        return Err(whole("is a directory, not a text file"));
    }
    let bytes = match fs::read(&path) {
        Ok(bytes) => bytes,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(e) => return Err(whole(&format!("can't be read: {e}"))),
    };
    let Ok(text) = String::from_utf8(bytes) else {
        return Err(whole("isn't UTF-8 text"));
    };
    let parsed = parse(&text).map_err(|bad| problem(shown, bad))?;
    check_images(&parsed, images).map_err(|bad| problem(shown, bad))?;
    Ok(Some(parsed.named))
}

fn problem(shown: &str, bad: Bad) -> ImageProblem {
    let at = bad.line.map_or(String::new(), |l| format!(", line {l}"));
    ImageProblem {
        line: bad.line,
        ..error(
            shown,
            ImageLimit::PaletteFile,
            bad.count as usize,
            bad.max as usize,
            format!("{shown}{at}: {}", bad.message),
            REMEDY,
        )
    }
}

/// The palettes and assignments of a palette file's text, checked against
/// each other (names, duplicates, `dmg`), not against the project.
pub(crate) fn parse(text: &str) -> Result<Parsed, Bad> {
    let text = text.strip_prefix('\u{FEFF}').unwrap_or(text);
    let mut parsed = Parsed::default();
    // Each palette's line, `dmg`'s included, for duplicates and messages.
    let mut defined: BTreeMap<String, u32> = BTreeMap::new();
    // Each assignment's palette name, checked once every palette is read.
    let mut wanted: Vec<(String, u32)> = Vec::new();
    for (i, raw) in text.lines().enumerate() {
        let line = i as u32 + 1;
        let words = uncommented(raw);
        if words.is_empty() {
            continue;
        }
        let joined = words.join(" ");
        if let Some((image, name)) = joined.split_once('=') {
            let (image, name) = (image.trim(), name.trim());
            assignment(image, name, line)?;
            if let Some(first) = parsed.lines.get(image) {
                return Err(Bad::at(
                    line,
                    format!("{image} is assigned a palette again (first on line {first})"),
                ));
            }
            parsed.lines.insert(image.to_string(), line);
            parsed
                .named
                .assignments
                .insert(image.to_string(), name.to_string());
            wanted.push((name.to_string(), line));
        } else if let Some((name, colors)) = joined.split_once(':') {
            let name = name.trim();
            check_name(name, line)?;
            if let Some(first) = defined.get(name) {
                return Err(Bad::at(
                    line,
                    format!("{name} is defined again (first on line {first})"),
                ));
            }
            defined.insert(name.to_string(), line);
            let colors = palette(name, colors, line)?;
            if name == DMG {
                parsed.named.dmg = Some(colors);
            } else {
                parsed.named.palettes.push(NamedPalette {
                    name: name.to_string(),
                    colors,
                });
            }
        } else {
            return Err(Bad::at(
                line,
                format!(
                    "{:?} is neither a palette (`name: c0 c1 c2 c3`), an assignment \
                     (`bg/town.png = name`) nor a comment ('#', a space or tab, then text)",
                    raw.trim()
                ),
            ));
        }
    }
    for (name, line) in wanted {
        if name == DMG {
            return Err(Bad::at(
                line,
                "dmg is the DMG's shades, not a GBC palette, so no image can be assigned it; \
                 name a palette of its own"
                    .into(),
            ));
        }
        if !defined.contains_key(&name) {
            let known: Vec<&str> = parsed
                .named
                .palettes
                .iter()
                .map(|p| p.name.as_str())
                .collect();
            let known = match known.len() {
                0 => "the file defines none".to_string(),
                _ => format!("the file defines {}", known.join(", ")),
            };
            return Err(Bad::at(
                line,
                format!("{name} is no palette in this file ({known})"),
            ));
        }
    }
    Ok(parsed)
}

/// The words of a line before its comment: `#` alone as a word (it was
/// followed by whitespace or the line's end) starts one.
fn uncommented(raw: &str) -> Vec<&str> {
    raw.split_whitespace().take_while(|w| *w != "#").collect()
}

/// A palette's name: a lowercase letter, then lowercase letters, digits
/// and `_`, so it can become a label (`PaletteForest`, `PALETTE_FOREST`).
fn check_name(name: &str, line: u32) -> Result<(), Bad> {
    let mut chars = name.chars();
    let ok = chars.next().is_some_and(|c| c.is_ascii_lowercase())
        && chars.all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '_');
    if ok {
        Ok(())
    } else {
        Err(Bad::at(
            line,
            format!(
                "{name:?} can't name a palette: a lowercase letter, then lowercase letters, \
                 digits or _"
            ),
        ))
    }
}

/// An assignment's two sides, as written.
fn assignment(image: &str, name: &str, line: u32) -> Result<(), Bad> {
    if image.is_empty() || name.is_empty() || name.contains(char::is_whitespace) {
        return Err(Bad::at(
            line,
            format!(
                "{:?} isn't an assignment: one image, '=', one palette name",
                format!("{image} = {name}")
            ),
        ));
    }
    check_name(name, line)
}

/// The four colors after `name:`.
fn palette(name: &str, colors: &str, line: u32) -> Result<GbcPalette, Bad> {
    let mut read: Vec<Rgb555> = Vec::new();
    for word in colors.split_whitespace() {
        let Some(c) = color(word) else {
            return Err(Bad {
                count: read.len() as u32,
                max: 4,
                ..Bad::at(
                    line,
                    format!("{word:?} in {name} is no #rrggbb or #rgb color"),
                )
            });
        };
        if name == DMG
            && let Some(at) = read.iter().position(|&d| d == c)
        {
            return Err(Bad {
                count: read.len() as u32 + 1,
                max: 4,
                ..Bad::at(
                    line,
                    format!(
                        "{word} is {} (shade {at}) to rgbgfx, which keeps 5 bits a channel, so \
                         the two can't name two shades",
                        colors.split_whitespace().nth(at).unwrap_or("?"),
                    ),
                )
            });
        }
        read.push(c);
    }
    match read[..] {
        [a, b, c, d] => Ok(GbcPalette([a, b, c, d])),
        _ => Err(Bad {
            count: read.len() as u32,
            max: 4,
            ..Bad::at(
                line,
                format!(
                    "{name} lists {} colors; a palette has 4, index 0 first",
                    read.len()
                ),
            )
        }),
    }
}

/// `#rrggbb` or `#rgb` (either case), as RGB555.
pub(crate) fn color(word: &str) -> Option<Rgb555> {
    let digits = word.strip_prefix('#')?;
    if !digits.bytes().all(|b| b.is_ascii_hexdigit()) {
        return None;
    }
    let v = u32::from_str_radix(digits, 16).ok()?;
    let [r, g, b] = match digits.len() {
        6 => [(v >> 16) as u8, (v >> 8) as u8, v as u8],
        // CSS: each digit doubled, #8c4 = #88cc44.
        3 => [(v >> 8) & 0xF, (v >> 4) & 0xF, v & 0xF].map(|d| (d * 0x11) as u8),
        _ => return None,
    };
    Some(Rgb555::from_rgb888(r, g, b))
}

/// Every assignment names one of the project's images.
pub(crate) fn check_images(parsed: &Parsed, images: &[String]) -> Result<(), Bad> {
    for (image, &line) in &parsed.lines {
        if !images.iter().any(|i| i == image) {
            let read = if images.is_empty() {
                "the project has no images".to_string()
            } else {
                format!("its images are {}", images.join(", "))
            };
            return Err(Bad::at(
                line,
                format!(
                    "{image} is no image of this project: an assignment names a PNG by its \
                     path under assets/, in bg/ or sprites/ ({read})"
                ),
            ));
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rgb(v: u32) -> Rgb555 {
        Rgb555::from_rgb888((v >> 16) as u8, (v >> 8) as u8, v as u8)
    }

    fn dmg(text: &str) -> Result<GbcPalette, Bad> {
        parse(text).map(|p| p.named.dmg.expect("a dmg line"))
    }

    const GRAYS: &str = "dmg: #ffffff #aaaaaa #555555 #000000\n";

    #[test]
    fn the_dmg_line_with_comments_and_blank_lines() {
        let text = "# my greens, shade 0 first\n\ndmg: #E0F8CF #86c06c  # mid\n";
        // A comment cuts the line: only two colors before it.
        assert_eq!(dmg(text).unwrap_err().count, 2);
        let text = "# my greens, shade 0 first\n\ndmg: #E0F8CF #86c06c #306850 #071821 # dark\n";
        assert_eq!(
            dmg(text),
            Ok(GbcPalette(
                [0xE0F8CF, 0x86C06C, 0x306850, 0x071821].map(rgb)
            ))
        );
    }

    /// CSS shorthand, then RGB555: #8c4 is #88cc44, channels 17, 25 and 8.
    #[test]
    fn a_three_digit_color_is_css_shorthand_then_rgb555() {
        assert_eq!(color("#8c4"), Some(rgb(0x88CC44)));
        assert_eq!(color("#8c4").unwrap().channels(), [17, 25, 8]);
        assert_eq!(color("#FFF"), Some(Rgb555::WHITE));
        for bad in ["#8c", "#8c44", "#gggggg", "8c4", "#", "#+12"] {
            assert_eq!(color(bad), None, "{bad}");
        }
    }

    #[test]
    fn palettes_in_file_order_and_assignments_by_image() {
        let text = "dmg:    #e0f8d0 #88c070 #346856 #081820   # reserved\n\
                    forest: #fff #8c4 #363 #021\n\
                    ember:  #ffe #f80 #a20 #200\n\
                    bg/town.png = forest\n\
                    sprites/hero.png=ember\n";
        let parsed = parse(text).unwrap();
        let names: Vec<&str> = parsed
            .named
            .palettes
            .iter()
            .map(|p| p.name.as_str())
            .collect();
        assert_eq!(names, ["forest", "ember"]);
        assert_eq!(
            parsed.named.palettes[0].colors,
            GbcPalette([0xFFFFFF, 0x88CC44, 0x336633, 0x002211].map(rgb))
        );
        assert_eq!(parsed.named.assignments["bg/town.png"], "forest");
        assert_eq!(parsed.named.assignments["sprites/hero.png"], "ember");
        assert_eq!(parsed.lines["sprites/hero.png"], 5);
        assert!(parsed.named.dmg.is_some());
    }

    /// No `dmg` line: no declared shades, and the cascade moves on.
    #[test]
    fn a_file_without_dmg_or_with_nothing_declares_no_shades() {
        let named = parse("forest: #fff #8c4 #363 #021\n").unwrap().named;
        assert_eq!((named.dmg, named.palettes.len()), (None, 1));
        assert_eq!(parse("# nothing yet\n\n").unwrap(), Parsed::default());
    }

    /// Each refusal names its line and what is wrong on it.
    #[test]
    fn every_palette_file_error_names_its_line() {
        for (text, line, said) in [
            ("dmg: #ffffff #aaaa #555555 #000000\n", 1, "#aaaa"),
            (&format!("# x\n{GRAYS}white\n"), 3, "white"),
            ("#comment\n", 1, "#comment"),
            ("dmg: #ffffff #aaaaaa #555555 #000000 extra\n", 1, "extra"),
            ("dmg: #gggggg #aaaaaa #555555 #000000\n", 1, "#gggggg"),
            ("Forest: #fff #8c4 #363 #021\n", 1, "\"Forest\" can't name"),
            (
                "a: #fff #8c4 #363 #021\nb: #fff #8c4 #363 #021\na: #fff #8c4 #363 #021\n",
                3,
                "first on line 1",
            ),
            (
                &format!("{GRAYS}\ndmg: #fff #aaa #555 #000\n"),
                3,
                "dmg is defined again",
            ),
            (
                "a: #fff #8c4 #363 #021\nbg/t.png = a\nbg/t.png = a\n",
                3,
                "first on line 2",
            ),
            (
                "a: #fff #8c4 #363 #021\n\nbg/t.png = b\n",
                3,
                "b is no palette in this file (the file defines a)",
            ),
            ("bg/t.png = b\n", 1, "the file defines none"),
            (
                &format!("{GRAYS}bg/t.png = dmg\n"),
                2,
                "dmg is the DMG's shades",
            ),
            ("bg/t.png = \n", 1, "isn't an assignment"),
            ("bg/t.png = a b\n", 1, "isn't an assignment"),
        ] {
            let bad = parse(text).unwrap_err();
            assert_eq!(bad.line, Some(line), "{text:?}: {}", bad.message);
            assert!(bad.message.contains(said), "{said:?} in {}", bad.message);
        }
    }

    /// The review of d0e891a: `#ffffff #aaaaaa` read the second color as a
    /// comment. Now any `#` word but `#` alone is a color, so a fifth is an
    /// error, and so is a line of colors with no name.
    #[test]
    fn a_color_after_the_fourth_is_an_error_not_a_comment() {
        let bad = parse("dmg: #ffffff #aaaaaa #555555 #000000 #123456\n").unwrap_err();
        assert_eq!((bad.line, bad.count, bad.max), (Some(1), 5, 4));
        assert!(bad.message.contains("5 colors"), "{}", bad.message);
        let bad = parse("#ffffff #aaaaaa\n").unwrap_err();
        assert_eq!(bad.line, Some(1));
        assert!(bad.message.contains("neither a palette"), "{}", bad.message);
        // `#` then a space is still a comment after the colors.
        assert!(parse("dmg: #ffffff #aaaaaa #555555 #000000 # #123456\n").is_ok());
    }

    #[test]
    fn other_than_four_colors_is_an_error_counting_them() {
        let bad = parse("dmg: #ffffff #aaaaaa #000000\n").unwrap_err();
        assert_eq!((bad.count, bad.max, bad.line), (3, 4, Some(1)));
        assert!(bad.message.contains("3 colors"), "{}", bad.message);
    }

    /// Two `dmg` entries rgbgfx can't tell apart (RGB555) can't name two
    /// shades; a named palette may repeat a color.
    #[test]
    fn two_dmg_colors_one_in_rgb555_are_an_error_at_the_second() {
        let bad = parse("dmg: #ffffff #000000 #555555 #040404\n").unwrap_err();
        assert_eq!(bad.line, Some(1));
        assert!(bad.message.contains("#040404"), "{}", bad.message);
        assert!(bad.message.contains("#000000"), "{}", bad.message);
        assert!(parse("twin: #000 #000 #fff #fff\n").is_ok());
    }

    /// A UTF-8 BOM (Notepad's habit) isn't part of the first line.
    #[test]
    fn a_leading_byte_order_mark_is_skipped() {
        let text = format!("\u{FEFF}{GRAYS}");
        assert_eq!(dmg(&text).map(|p| p.0[0]), Ok(Rgb555::WHITE));
    }

    #[test]
    fn a_hash_then_any_whitespace_is_a_comment() {
        for text in [
            format!("#\tlightest first\n{GRAYS}"),
            format!("# lightest first\n{GRAYS}"),
            format!("#\n{GRAYS}"),
            "dmg: #ffffff #aaaaaa #555555 #000000\t#\tdark\n".to_string(),
        ] {
            assert!(dmg(&text).is_ok(), "{text:?}");
        }
    }

    #[test]
    fn an_assignment_must_name_an_image_of_the_project() {
        let parsed = parse("a: #fff #8c4 #363 #021\n\nbg/town.png = a\n").unwrap();
        assert_eq!(check_images(&parsed, &["bg/town.png".into()]), Ok(()));
        let bad = check_images(&parsed, &["bg/sky.png".into()]).unwrap_err();
        assert_eq!(bad.line, Some(3));
        assert!(
            bad.message.contains("bg/town.png is no image"),
            "{}",
            bad.message
        );
        assert!(bad.message.contains("bg/sky.png"), "{}", bad.message);
        let bad = check_images(&parsed, &[]).unwrap_err();
        assert!(bad.message.contains("has no images"), "{}", bad.message);
    }
}
