//! Rows for the RGBDS loop: `build`'s diagnostics and outputs, `doctor`'s
//! checks and `new`'s files.

use gbcli_protocol::{ResultBody, Severity};

use super::Row;

pub fn rows(r: &ResultBody) -> String {
    let mut out = String::new();
    if let Some(b) = &r.build {
        for d in &b.diagnostics {
            let json = serde_json::to_value(d).expect("serializes");
            out += &Row::new("diagnostic")
                .field("severity", json["severity"].as_str().unwrap_or("?"))
                .field("tool", d.tool.name())
                .opt("file", d.file.as_deref())
                .opt("line", d.line)
                .field("message", &d.message)
                .line();
        }
        for o in &b.outputs {
            let json = serde_json::to_value(o).expect("serializes");
            out += &Row::new("output")
                .field("kind", json["kind"].as_str().unwrap_or("?"))
                .field("path", &o.path)
                .field("bytes", o.bytes)
                .line();
        }
        let count = |s: Severity| b.diagnostics.iter().filter(|d| d.severity == s).count();
        out += &Row::new("build")
            .field("rgbds", &b.rgbds)
            .field("sources", b.sources.len())
            .field("errors", count(Severity::Error))
            .field("warnings", count(Severity::Warning))
            .line();
    }
    if let Some(d) = &r.doctor {
        for c in &d.checks {
            let json = serde_json::to_value(c).expect("serializes");
            out += &Row::new("check")
                .field("name", json["name"].as_str().unwrap_or("?"))
                .field("status", json["status"].as_str().unwrap_or("?"))
                .field("detail", &c.detail)
                .opt("remedy", c.remedy.as_deref())
                .line();
        }
    }
    if let Some(n) = &r.created {
        for file in &n.files {
            out += &Row::new("created").field("path", file).line();
        }
        out += &Row::new("new")
            .field("name", &n.name)
            .field("dir", &n.dir)
            .line();
        for command in &n.next {
            out += &Row::new("next").field("command", command).line();
        }
    }
    out
}
