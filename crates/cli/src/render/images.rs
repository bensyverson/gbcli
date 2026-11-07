//! Rows for the images a build converted: one per image, one per problem
//! (its tiles shortened; `--json` lists them all), then a summary.

use gbcli_protocol::{ImagesMade, Severity, TileAt};

use super::Row;

/// Tiles a problem row names before it counts the rest.
const TILES_SHOWN: usize = 8;

fn tiles(tiles: &[TileAt]) -> String {
    let mut words: Vec<String> = tiles
        .iter()
        .take(TILES_SHOWN)
        .map(|t| format!("{},{}", t.x, t.y))
        .collect();
    if tiles.len() > TILES_SHOWN {
        words.push(format!("(+{} more)", tiles.len() - TILES_SHOWN));
    }
    words.join(" ")
}

pub fn rows(m: &ImagesMade) -> String {
    let mut out = String::new();
    for i in &m.images {
        out += &Row::new("image")
            .field("kind", i.kind.dir())
            .field("source", &i.source)
            .field("label", &i.label)
            .field("size", format!("{}x{}", i.width, i.height))
            .field("tiles", i.tiles)
            .opt("frames", i.frames)
            .opt("objects", i.objects)
            .field("preview", &i.preview)
            .line();
    }
    for p in &m.problems {
        let severity = match p.severity {
            Severity::Error => "error",
            Severity::Warning => "warning",
        };
        let mut row = Row::new("problem")
            .field("limit", p.limit.name())
            .field("severity", severity)
            .opt("image", p.image.as_deref())
            .field("count", p.count)
            .field("max", p.max);
        if !p.tiles.is_empty() {
            row = row.field("tiles", tiles(&p.tiles));
        }
        row = row.opt("line", p.line);
        out += &row
            .field("message", &p.message)
            .field("remedy", &p.remedy)
            .line();
    }
    let count = |s: Severity| m.problems.iter().filter(|p| p.severity == s).count();
    out += &Row::new("images")
        .opt("include", m.include.as_deref())
        .field("converted", m.images.len())
        .field("errors", count(Severity::Error))
        .field("warnings", count(Severity::Warning))
        .opt("shades", m.shades.map(|s| s.rule.name()))
        .opt(
            "palette",
            m.shades.and_then(|s| s.palette).map(|p| p.name()),
        )
        .line();
    out
}
