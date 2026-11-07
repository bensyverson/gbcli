//! `assets/palette.txt`: the project's declared shades (Ben's ruling,
//! 2026-11-07), the first rule of `shades::choose`. Four `#rrggbb` lines,
//! shade 0 (lightest on the DMG) first; a line that starts `#` and
//! whitespace (or is `#` alone) is a comment, as is anything after a color
//! and a space-`#`. Blank lines are skipped, and so is a leading UTF-8 byte
//! order mark.

use std::fs;
use std::path::Path;

use gbcli_protocol::{ImageLimit, ImageProblem};

use super::census::{error, key};
use super::pixels::{Rgba, hex};

/// Where the declared shades live, relative to the project.
pub(crate) const PATH: &str = "assets/palette.txt";

const REMEDY: &str = "write four colors, one #rrggbb a line, shade 0 (white on the DMG) first; \
                      a line starting with '#' and a space or tab is a comment";

/// Why a palette file was refused.
#[derive(Debug, PartialEq, Eq)]
pub(crate) struct Bad {
    /// The line at fault, 1-based; none when the file as a whole is.
    pub line: Option<u32>,
    /// Colors read before stopping.
    pub count: u32,
    pub message: String,
}

/// The project's declared shades: none without the file, else its four
/// colors or the problem with it. `shown` is the file's name in messages.
pub(crate) fn read(project: &Path, shown: &str) -> Result<Option<[Rgba; 4]>, ImageProblem> {
    let path = project.join(PATH);
    let whole = |message: &str| {
        problem(
            shown,
            Bad {
                line: None,
                count: 0,
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
    parse(&text).map(Some).map_err(|bad| problem(shown, bad))
}

fn problem(shown: &str, bad: Bad) -> ImageProblem {
    let at = bad.line.map_or(String::new(), |l| format!(", line {l}"));
    ImageProblem {
        line: bad.line,
        ..error(
            shown,
            ImageLimit::PaletteFile,
            bad.count as usize,
            4,
            format!("{shown}{at}: {}", bad.message),
            REMEDY,
        )
    }
}

/// The four colors of a palette file's text.
pub(crate) fn parse(text: &str) -> Result<[Rgba; 4], Bad> {
    let text = text.strip_prefix('\u{FEFF}').unwrap_or(text);
    let mut colors: Vec<(Rgba, u32)> = Vec::new();
    for (i, raw) in text.lines().enumerate() {
        let line = i as u32 + 1;
        let bad = |message: String| Bad {
            line: Some(line),
            count: colors.len() as u32,
            message,
        };
        let trimmed = raw.trim();
        let comment = trimmed
            .strip_prefix('#')
            .is_some_and(|r| r.is_empty() || r.starts_with(char::is_whitespace));
        if trimmed.is_empty() || comment {
            continue;
        }
        let (word, rest) = trimmed
            .split_once(char::is_whitespace)
            .unwrap_or((trimmed, ""));
        let rest = rest.trim_start();
        let Some(c) = color(word) else {
            return Err(bad(format!(
                "{word:?} is neither a #rrggbb color nor a comment ('#', a space or tab, then text)"
            )));
        };
        if !rest.is_empty() && !rest.starts_with('#') {
            return Err(bad(format!(
                "{rest:?} follows the color; only a comment ('#') may"
            )));
        }
        if colors.len() == 4 {
            return Err(Bad {
                count: 5,
                ..bad(format!("{} is a fifth color; the DMG has 4 shades", hex(c)))
            });
        }
        if let Some((same, at)) = colors.iter().find(|(d, _)| key(*d) == key(c)) {
            return Err(bad(format!(
                "{} is {} (line {at}) to rgbgfx, which keeps 5 bits a channel, so the two \
                 can't name two shades",
                hex(c),
                hex(*same)
            )));
        }
        colors.push((c, line));
    }
    match colors[..] {
        [(a, _), (b, _), (c, _), (d, _)] => Ok([a, b, c, d]),
        _ => Err(Bad {
            line: None,
            count: colors.len() as u32,
            message: format!(
                "it lists {} colors; the DMG has 4 shades, and each needs one",
                colors.len()
            ),
        }),
    }
}

/// `#rrggbb` (either case) as a color.
fn color(word: &str) -> Option<Rgba> {
    let digits = word.strip_prefix('#')?;
    if digits.len() != 6 || !digits.bytes().all(|b| b.is_ascii_hexdigit()) {
        return None;
    }
    let v = u32::from_str_radix(digits, 16).ok()?;
    Some([(v >> 16) as u8, (v >> 8) as u8, v as u8, 0xFF])
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn four_colors_lightest_first_with_comments_and_blank_lines() {
        let text = "# my greens, shade 0 first\n\n#E0F8CF\n#86c06c   # mid\n  #306850\n#071821\n";
        assert_eq!(
            parse(text),
            Ok([
                [0xE0, 0xF8, 0xCF, 0xFF],
                [0x86, 0xC0, 0x6C, 0xFF],
                [0x30, 0x68, 0x50, 0xFF],
                [0x07, 0x18, 0x21, 0xFF],
            ])
        );
    }

    #[test]
    fn a_line_that_is_neither_a_color_nor_a_comment_names_its_line() {
        for (text, line, said) in [
            ("#ffffff\n#aaaa\n", 2, "#aaaa"),
            ("#ffffff\nwhite\n", 2, "white"),
            ("#comment\n", 1, "#comment"),
            ("#ffffff extra\n", 1, "extra"),
            ("#gggggg\n", 1, "#gggggg"),
        ] {
            let bad = parse(text).unwrap_err();
            assert_eq!(bad.line, Some(line), "{text:?}");
            assert!(bad.message.contains(said), "{said:?} in {}", bad.message);
        }
    }

    #[test]
    fn other_than_four_colors_is_an_error_counting_them() {
        let bad = parse("#ffffff\n#aaaaaa\n#000000\n").unwrap_err();
        assert_eq!((bad.count, bad.line), (3, None));
        assert!(bad.message.contains("3 colors"), "{}", bad.message);
        let bad = parse("#ffffff\n#aaaaaa\n#555555\n#000000\n#000000\n").unwrap_err();
        assert_eq!((bad.count, bad.line), (5, Some(5)));
    }

    /// Two entries rgbgfx can't tell apart (RGB555) can't name two shades.
    #[test]
    fn two_colors_one_in_rgb555_are_an_error_at_the_second() {
        let bad = parse("#ffffff\n#000000\n#555555\n#040404\n").unwrap_err();
        assert_eq!(bad.line, Some(4));
        assert!(bad.message.contains("#000000"), "{}", bad.message);
    }

    /// A UTF-8 BOM (Notepad's habit) isn't part of the first line.
    #[test]
    fn a_leading_byte_order_mark_is_skipped() {
        let text = "\u{FEFF}#ffffff\n#aaaaaa\n#555555\n#000000\n";
        assert_eq!(parse(text).map(|c| c[0]), Ok([0xFF, 0xFF, 0xFF, 0xFF]));
    }

    /// `#` then any whitespace is a comment: a tab as well as a space.
    #[test]
    fn a_hash_then_any_whitespace_is_a_comment() {
        let text = "#\tshade 0 first\n#ffffff\n#aaaaaa\n#555555\n#000000\n";
        assert!(parse(text).is_ok(), "{:?}", parse(text));
    }
}
