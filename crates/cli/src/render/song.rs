//! Rows for `song`: the problem that stopped it, or the song read and each
//! file written.

use gbcli_protocol::{ResultBody, SongOutputKind};

use super::Row;

pub fn rows(r: &ResultBody) -> String {
    let mut out = String::new();
    if let Some(p) = &r.song_problem {
        out += &Row::new("problem")
            .field("path", &p.path)
            .opt("line", p.line)
            .opt("column", p.column)
            .opt("offset", p.offset)
            .field("message", &p.message)
            .field("remedy", &p.remedy)
            .line();
    }
    if let Some(s) = &r.song {
        let json = serde_json::to_value(s.format).expect("serializes");
        out += &Row::new("song")
            .field("path", &s.path)
            .field("format", json.as_str().unwrap_or("?"))
            .opt("version", s.uge_version)
            .field("name", &s.name)
            .field("instruments", s.instruments)
            .field("patterns", s.patterns)
            .field("orders", s.orders)
            .line();
        for o in &s.wrote {
            let kind = match o.kind {
                SongOutputKind::Text => "text",
                SongOutputKind::Uge => "uge",
                SongOutputKind::Piano => "piano",
                SongOutputKind::Asm => "asm",
            };
            out += &Row::new(kind)
                .field("path", &o.path)
                .field("bytes", o.bytes)
                .opt("width", o.width)
                .opt("height", o.height)
                .opt("notes", o.notes)
                .opt("rows", o.rows)
                .opt("descriptor", o.descriptor.as_deref())
                .line();
        }
    }
    out
}
