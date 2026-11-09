//! YAFF 1.0's grammar, whole: comments, global properties, and glyph
//! definitions (labels, a glyph's rows, per-glyph properties). Nothing
//! here judges a label or a size; `strict` does. A document a grammar
//! error stops is refused at the line (and column) at fault.

use crate::error::FontError;

/// A YAFF file as written, every part with its line.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Document {
    /// The comment at the top of the file, before a blank line.
    pub comment: Vec<String>,
    pub properties: Vec<Prop>,
    pub glyphs: Vec<GlyphDef>,
    /// Comments after the last glyph.
    pub tail: Vec<String>,
}

/// A property, global or per-glyph.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Prop {
    pub line: u32,
    pub comment: Vec<String>,
    pub key: String,
    /// Its lines joined by `\n`, each trimmed.
    pub value: String,
}

/// A glyph definition: its labels, its rows and its properties.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct GlyphDef {
    pub comment: Vec<String>,
    pub labels: Vec<Label>,
    /// The line of the first row.
    pub rows_line: u32,
    /// The rows, without indent or trailing whitespace; empty for YAFF's
    /// empty glyph, `-`.
    pub rows: Vec<String>,
    pub properties: Vec<Prop>,
}

/// A label as written, before its `:`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Label {
    pub line: u32,
    pub text: String,
}

const GRAMMAR: &str = "`gbcli help fonts` has the format: comments (#), then properties \
                       (key: value), then glyphs, each a label ('A':) over its indented rows \
                       of . and @";

fn err(line: usize, column: usize, message: impl Into<String>) -> FontError {
    FontError::new(line as u32 + 1, column as u32, message, GRAMMAR)
}

fn is_blank(line: &str) -> bool {
    line.chars().all(|c| c == ' ' || c == '\t')
}

fn indented(line: &str) -> bool {
    line.starts_with([' ', '\t'])
}

fn indent_of(line: &str) -> &str {
    &line[..line.len() - line.trim_start_matches([' ', '\t']).len()]
}

/// Whether an indented line's content opens a glyph's rows.
fn opens_rows(line: &str) -> bool {
    let content = line.trim();
    content.starts_with(['.', '@']) || content == "-"
}

/// A property key: ASCII letters, digits, `_`, `-` and `.`.
fn is_key(key: &str) -> bool {
    !key.is_empty()
        && key
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '_' | '-' | '.'))
}

fn comment_text(after_hash: &str) -> String {
    after_hash
        .strip_prefix([' ', '\t'])
        .unwrap_or(after_hash)
        .to_string()
}

/// Reads `text` by YAFF 1.0's grammar.
pub fn parse(text: &str) -> Result<Document, FontError> {
    let text = text.strip_prefix('\u{FEFF}').unwrap_or(text);
    let lines: Vec<&str> = text
        .split('\n')
        .map(|l| l.strip_suffix('\r').unwrap_or(l))
        .collect();
    for (n, line) in lines.iter().enumerate() {
        if let Some((i, c)) = line
            .chars()
            .enumerate()
            .find(|(_, c)| c.is_control() && *c != '\t')
        {
            return Err(err(
                n,
                i + 1,
                format!(
                    "u+{:04X} is a control character, which a YAFF file can't hold",
                    u32::from(c)
                ),
            ));
        }
    }
    let mut doc = Document::default();
    let mut pending: Vec<String> = Vec::new();
    let mut i = 0;
    while i < lines.len() {
        let line = lines[i];
        if is_blank(line) {
            let first_item = doc.properties.is_empty() && doc.glyphs.is_empty();
            if first_item && doc.comment.is_empty() {
                doc.comment = std::mem::take(&mut pending);
            }
            i += 1;
            continue;
        }
        if let Some(c) = line.strip_prefix('#') {
            pending.push(comment_text(c));
            i += 1;
            continue;
        }
        if indented(line) {
            return Err(err(
                i,
                1,
                "this indented line belongs to nothing: rows follow a glyph's label, and a \
                 value of several lines its property's `key:` line",
            ));
        }
        let trimmed = line.trim_end();
        if !trimmed.ends_with(':') {
            let Some((key, value)) = line.split_once(':') else {
                return Err(err(
                    i,
                    1,
                    "this line is neither a comment, a property (key: value) nor a label ('A':)",
                ));
            };
            let key = key.trim_end();
            if !is_key(key) {
                return Err(err(
                    i,
                    1,
                    format!(
                        "`{key}` isn't a property key: a key holds only ASCII letters, digits, \
                         `_`, `-` and `.`"
                    ),
                ));
            }
            global(&doc, i)?;
            doc.properties.push(Prop {
                line: i as u32 + 1,
                comment: std::mem::take(&mut pending),
                key: key.to_string(),
                value: value.trim().to_string(),
            });
            i += 1;
            continue;
        }
        // A run of lines ending in `:` is a glyph's labels when rows follow
        // it, or one property whose value is the indented lines below.
        let start = i;
        let mut j = i;
        while j < lines.len()
            && !is_blank(lines[j])
            && !indented(lines[j])
            && !lines[j].starts_with('#')
            && lines[j].trim_end().ends_with(':')
        {
            j += 1;
        }
        let next = lines.get(j).filter(|l| indented(l) && !is_blank(l));
        match next {
            Some(l) if opens_rows(l) => {
                let labels = lines[start..j]
                    .iter()
                    .enumerate()
                    .map(|(k, l)| {
                        let t = l.trim_end();
                        Label {
                            line: (start + k) as u32 + 1,
                            text: t[..t.len() - 1].trim_end().to_string(),
                        }
                    })
                    .collect();
                let (glyph, after) = glyph(&lines, j, labels, std::mem::take(&mut pending))?;
                doc.glyphs.push(glyph);
                i = after;
            }
            Some(_) if j == start + 1 => {
                let key = trimmed[..trimmed.len() - 1].trim_end();
                if !is_key(key) {
                    return Err(err(
                        j,
                        1,
                        format!(
                            "`{key}:` reads as a label, but the lines under it aren't a glyph's \
                             rows (only . and @)"
                        ),
                    ));
                }
                global(&doc, i)?;
                let (value, after) = value_lines(&lines, j, 0);
                doc.properties.push(Prop {
                    line: i as u32 + 1,
                    comment: std::mem::take(&mut pending),
                    key: key.to_string(),
                    value,
                });
                i = after;
            }
            Some(_) => {
                return Err(err(
                    j,
                    1,
                    "the lines under these labels aren't a glyph's rows (only . and @)",
                ));
            }
            None => {
                return Err(err(
                    start,
                    1,
                    format!(
                        "`{trimmed}` has nothing under it: a label needs its glyph's rows, \
                         indented, on the next line"
                    ),
                ));
            }
        }
    }
    doc.tail = pending;
    Ok(doc)
}

/// Refuses a property after the glyphs began.
fn global(doc: &Document, i: usize) -> Result<(), FontError> {
    if doc.glyphs.is_empty() {
        return Ok(());
    }
    Err(err(
        i,
        1,
        "this property follows a glyph; properties come before the glyphs",
    ))
}

/// The value of several lines from line `j`: the indented lines deeper
/// than `indent` characters, each trimmed, and the line after them.
fn value_lines(lines: &[&str], mut j: usize, indent: usize) -> (String, usize) {
    let mut value: Vec<&str> = Vec::new();
    while j < lines.len()
        && !is_blank(lines[j])
        && indented(lines[j])
        && indent_of(lines[j]).len() > indent
    {
        value.push(lines[j].trim());
        j += 1;
    }
    (value.join("\n"), j)
}

/// The glyph whose rows start at line `j`, and the line after it and its
/// properties.
fn glyph(
    lines: &[&str],
    j: usize,
    labels: Vec<Label>,
    comment: Vec<String>,
) -> Result<(GlyphDef, usize), FontError> {
    let indent = indent_of(lines[j]);
    let mut rows: Vec<String> = Vec::new();
    let mut k = j;
    while k < lines.len() && !is_blank(lines[k]) && indented(lines[k]) {
        let line = lines[k];
        if indent_of(line) != indent {
            return Err(err(
                k,
                1,
                "this row's indent differs from the first row's; a glyph's rows share one indent",
            ));
        }
        let row = line[indent.len()..].trim_end();
        let first = indent.chars().count();
        if row == "-" {
            if k != j
                || lines
                    .get(k + 1)
                    .is_some_and(|l| indented(l) && !is_blank(l))
            {
                return Err(err(k, first + 1, "`-`, the empty glyph, stands alone"));
            }
            k += 1;
            break;
        }
        if let Some((at, c)) = row
            .chars()
            .enumerate()
            .find(|(_, c)| !matches!(c, '.' | '@'))
        {
            return Err(err(
                k,
                first + at + 1,
                format!("`{c}` in a glyph's row: a row holds only . (paper) and @ (ink)"),
            ));
        }
        if let Some(above) = rows.first() {
            let (w, want) = (row.chars().count(), above.chars().count());
            if w != want {
                let px = |n: usize| if n == 1 { "pixel" } else { "pixels" };
                return Err(err(
                    k,
                    first + 1,
                    format!(
                        "this row is {w} {} wide; the rows above it are {want}",
                        px(w)
                    ),
                ));
            }
        }
        rows.push(row.to_string());
        k += 1;
    }
    // Per-glyph properties follow a blank line, at the rows' indent.
    let mut properties = Vec::new();
    let mut m = k;
    while m < lines.len() && is_blank(lines[m]) {
        m += 1;
    }
    if m > k && m < lines.len() && indented(lines[m]) {
        while m < lines.len() && !is_blank(lines[m]) && indented(lines[m]) {
            let line = lines[m];
            let content = line.trim();
            let Some((key, value)) = content.split_once(':') else {
                return Err(err(
                    m,
                    1,
                    "this line follows a glyph but is neither a row nor a property (key: value)",
                ));
            };
            let key = key.trim_end();
            if !is_key(key) {
                return Err(err(m, 1, format!("`{key}` isn't a property key")));
            }
            let value = value.trim();
            let (value, after) = if value.is_empty() {
                value_lines(lines, m + 1, indent_of(line).len())
            } else {
                (value.to_string(), m + 1)
            };
            properties.push(Prop {
                line: m as u32 + 1,
                comment: Vec::new(),
                key: key.to_string(),
                value,
            });
            m = after;
        }
        k = m;
    }
    Ok((
        GlyphDef {
            comment,
            labels,
            rows_line: j as u32 + 1,
            rows,
            properties,
        },
        k,
    ))
}
