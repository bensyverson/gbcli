//! BDF in. Each glyph is placed in its 8x8 cell by its bounding box: its
//! top row sits `ascent - (y offset + height)` rows down, where the
//! font's ascent is `FONT_ASCENT` (else the font's bounding box's top),
//! and its pixels start `x offset` columns right of the pen. It advances
//! by its `DWIDTH`. So a trimmed glyph (Misaki's) lands where it draws.
//!
//! Glyphs are keyed by `ENCODING` as Unicode (the file's
//! `CHARSET_REGISTRY` must be ISO10646, or ISO8859 with encoding 1, or
//! absent), or, unencoded, by their `STARTCHAR` name when it is a gbcli
//! glyph name. Properties become YAFF properties, named in lower case
//! with `-` for `_`; `FONT_ASCENT` and `FONT_DESCENT` become `ascent` and
//! `descent`.

use crate::key::{Key, said};
use crate::model::{Font, Glyph, HEIGHT, MAX_WIDTH, Property};
use crate::problem::Problem;

const FORMAT: &str = "check the file is BDF 2.1 (STARTFONT, then glyphs from STARTCHAR to \
                      ENDCHAR); gbcli reads the subset bitmap font editors write";

/// A font read from BDF.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Imported {
    pub font: Font,
    /// Glyphs with no gbcli key (an unencoded glyph whose name isn't a
    /// glyph name, a control character, a repeat), by name, skipped.
    pub skipped: Vec<String>,
    /// Keys `only` asked for that the file has no glyph for.
    pub missing: Vec<Key>,
}

struct Char {
    line: usize,
    name: String,
    encoding: Option<i64>,
    dwidth: Option<i64>,
    bbx: Option<[i64; 4]>,
    bitmap: Vec<String>,
}

/// The font `text` holds, its glyphs limited to `only` when given (in the
/// file's order).
pub fn read_bdf(text: &str, only: Option<&[Key]>) -> Result<Imported, Vec<Problem>> {
    let lines: Vec<&str> = text.lines().map(|l| l.trim_end_matches('\r')).collect();
    let bad = |n: usize, message: String| vec![Problem::new(message, FORMAT).at_line(n + 1, None)];
    if !lines.first().is_some_and(|l| l.starts_with("STARTFONT")) {
        return Err(bad(0, "a BDF file starts with STARTFONT".into()));
    }
    let mut comment = Vec::new();
    let mut props: Vec<(String, String)> = Vec::new();
    let mut font_box: Option<[i64; 4]> = None;
    let mut chars: Vec<Char> = Vec::new();
    let mut i = 1;
    let mut ended = false;
    while i < lines.len() {
        let line = lines[i];
        let (word, rest) = line.split_once(' ').unwrap_or((line, ""));
        let rest = rest.trim();
        match word {
            "COMMENT" if chars.is_empty() => comment.push(rest.to_string()),
            "FONTBOUNDINGBOX" => font_box = numbers(rest),
            "STARTPROPERTIES" => {
                i += 1;
                while i < lines.len() && lines[i].trim() != "ENDPROPERTIES" {
                    let l = lines[i].trim();
                    let (name, value) = l.split_once(' ').unwrap_or((l, ""));
                    props.push((name.to_string(), unquote(value.trim())));
                    i += 1;
                }
                if i == lines.len() {
                    return Err(bad(i - 1, "STARTPROPERTIES has no ENDPROPERTIES".into()));
                }
            }
            "STARTCHAR" => {
                let mut c = Char {
                    line: i + 1,
                    name: rest.to_string(),
                    encoding: None,
                    dwidth: None,
                    bbx: None,
                    bitmap: Vec::new(),
                };
                i += 1;
                let mut in_bitmap = false;
                loop {
                    let Some(l) = lines.get(i) else {
                        return Err(bad(
                            c.line - 1,
                            format!("STARTCHAR {} has no ENDCHAR", c.name),
                        ));
                    };
                    let (w, r) = l.split_once(' ').unwrap_or((l, ""));
                    match w {
                        "ENDCHAR" => break,
                        _ if in_bitmap => c.bitmap.push(l.trim().to_string()),
                        "ENCODING" => {
                            c.encoding = r.split_whitespace().next().and_then(|n| n.parse().ok())
                        }
                        "DWIDTH" => {
                            c.dwidth = r.split_whitespace().next().and_then(|n| n.parse().ok())
                        }
                        "BBX" => c.bbx = numbers(r),
                        "BITMAP" => in_bitmap = true,
                        _ => {}
                    }
                    i += 1;
                }
                chars.push(c);
            }
            "ENDFONT" => {
                ended = true;
                break;
            }
            _ => {}
        }
        i += 1;
    }
    if !ended {
        return Err(bad(
            lines.len().saturating_sub(1),
            "the file has no ENDFONT".into(),
        ));
    }
    let prop = |name: &str| {
        props
            .iter()
            .find(|(n, _)| n == name)
            .map(|(_, v)| v.as_str())
    };
    if let Some(registry) = prop("CHARSET_REGISTRY") {
        let upper = registry.to_ascii_uppercase();
        let latin1 = upper == "ISO8859" && prop("CHARSET_ENCODING") == Some("1");
        if upper != "ISO10646" && !latin1 {
            return Err(vec![Problem::new(
                format!(
                    "the font is encoded in {registry}-{}; gbcli keys glyphs by Unicode",
                    prop("CHARSET_ENCODING").unwrap_or("?")
                ),
                "convert it to a Unicode (ISO10646-1) BDF first, in an editor such as FontForge \
                 or monobit's monobit-convert",
            )]);
        }
    }
    let ascent = prop("FONT_ASCENT")
        .and_then(|a| a.parse::<i64>().ok())
        .or(font_box.map(|b| b[1] + b[3]))
        .unwrap_or(i64::from(crate::model::DEFAULT_ASCENT));
    let mut problems = Vec::new();
    let mut glyphs: Vec<Glyph> = Vec::new();
    let mut skipped = Vec::new();
    for c in &chars {
        let key = match c.encoding {
            Some(n) if n >= 0 => u32::try_from(n)
                .ok()
                .and_then(char::from_u32)
                .and_then(|ch| Key::from_wire(&ch.to_string()).ok()),
            _ => match Key::from_wire(&c.name) {
                Ok(k @ Key::Name(_)) => Some(k),
                _ => None,
            },
        };
        let Some(key) = key else {
            skipped.push(c.name.clone());
            continue;
        };
        if only.is_some_and(|o| !o.contains(&key)) {
            continue;
        }
        if glyphs.iter().any(|g| g.key == key) {
            skipped.push(c.name.clone());
            continue;
        }
        match place(c, &key, ascent) {
            Ok(g) => glyphs.push(g),
            Err(p) => problems.push(p.at_line(c.line, None).of(&key)),
        }
    }
    if !problems.is_empty() {
        return Err(problems);
    }
    let missing = only
        .unwrap_or(&[])
        .iter()
        .filter(|k| !glyphs.iter().any(|g| &g.key == *k))
        .cloned()
        .collect();
    Ok(Imported {
        font: Font {
            comment,
            properties: properties(&props),
            glyphs,
            tail: Vec::new(),
        },
        skipped,
        missing,
    })
}

/// The glyph `c` draws, in its cell.
fn place(c: &Char, key: &Key, ascent: i64) -> Result<Glyph, Problem> {
    let who = match key {
        Key::Char(ch) => said(*ch),
        Key::Name(n) => format!("\"{n}\""),
    };
    let fix = "draw it inside an 8x8 cell in an editor, or leave it out with --chars";
    let width = c
        .dwidth
        .ok_or_else(|| Problem::new(format!("{who} has no DWIDTH"), FORMAT))?;
    if !(1..=i64::from(MAX_WIDTH)).contains(&width) {
        return Err(Problem::new(
            format!("{who} advances {width} pixels (DWIDTH); a gbcli glyph is 1 to 8"),
            fix,
        ));
    }
    let [w, h, xo, yo] = c
        .bbx
        .ok_or_else(|| Problem::new(format!("{who} has no BBX"), FORMAT))?;
    if c.bitmap.len() as i64 != h {
        return Err(Problem::new(
            format!(
                "{who}'s BITMAP has {} rows; its BBX says {h}",
                c.bitmap.len()
            ),
            FORMAT,
        ));
    }
    let top = ascent - (yo + h);
    let mut rows = [0u8; HEIGHT];
    for (i, hex) in c.bitmap.iter().enumerate() {
        let bits = u128::from_str_radix(hex, 16)
            .ok()
            .filter(|_| !hex.is_empty() && hex.len() <= 32)
            .ok_or_else(|| Problem::new(format!("{who}'s BITMAP row `{hex}` isn't hex"), FORMAT))?;
        let total = hex.len() as i64 * 4;
        for x in 0..w.min(total) {
            if bits >> (total - 1 - x) & 1 == 0 {
                continue;
            }
            let (col, row) = (xo + x, top + i as i64);
            let off = if row < 0 {
                Some(format!("{} rows above the cell's top", -row))
            } else if row >= HEIGHT as i64 {
                Some(format!(
                    "{} rows below the cell's bottom",
                    row - HEIGHT as i64 + 1
                ))
            } else if col < 0 {
                Some("left of the pen".to_string())
            } else if col >= width {
                Some(format!("right of its width ({width})"))
            } else {
                None
            };
            if let Some(off) = off {
                return Err(Problem::new(
                    format!(
                        "{who} has ink {off} (ascent {ascent}): a gbcli glyph is drawn within its \
                         8-row cell and its width"
                    ),
                    fix,
                ));
            }
            rows[row as usize] |= 0x80 >> col;
        }
    }
    Ok(Glyph {
        comment: Vec::new(),
        key: key.clone(),
        width: width as u8,
        rows,
    })
}

/// The BDF properties as YAFF properties the strict reader takes: named
/// in lower case, each once, none empty, none that moves glyphs.
fn properties(props: &[(String, String)]) -> Vec<Property> {
    let mut out: Vec<Property> = Vec::new();
    for (name, value) in props {
        let key = match name.as_str() {
            "FONT_ASCENT" => "ascent".to_string(),
            "FONT_DESCENT" => "descent".to_string(),
            "CHARSET_REGISTRY" | "CHARSET_ENCODING" => continue,
            _ => name
                .to_ascii_lowercase()
                .replace('_', "-")
                .chars()
                .map(|c| {
                    if c.is_ascii_alphanumeric() || matches!(c, '-' | '.') {
                        c
                    } else {
                        '-'
                    }
                })
                .collect(),
        };
        let value: String = value
            .chars()
            .map(|c| if c.is_control() { ' ' } else { c })
            .collect::<String>()
            .trim()
            .to_string();
        let moves = key == "yaff" || key.contains("bearing") || key == "tracking";
        if key.is_empty() || value.is_empty() || moves || out.iter().any(|p| p.key == key) {
            continue;
        }
        out.push(Property {
            comment: Vec::new(),
            key,
            value,
        });
    }
    out
}

/// Four whole numbers.
fn numbers(text: &str) -> Option<[i64; 4]> {
    let v: Vec<i64> = text
        .split_whitespace()
        .map(str::parse)
        .collect::<Result<_, _>>()
        .ok()?;
    v.try_into().ok()
}

/// A property's value: a quoted string with `""` for a quote, or as is.
fn unquote(value: &str) -> String {
    match value.strip_prefix('"').and_then(|v| v.strip_suffix('"')) {
        Some(inner) => inner.replace("\"\"", "\""),
        None => value.to_string(),
    }
}
