//! Rows for the RGBDS loop: `build`'s diagnostics, songs and outputs,
//! `doctor`'s checks and `new`'s files.

use gbcli_protocol::{ResultBody, Severity, SourceLine};

use super::Row;

fn source_line(kind: &str, at: &SourceLine) -> String {
    Row::new(kind)
        .field("file", &at.file)
        .field("line", at.line)
        .opt("within", at.within.as_deref())
        .line()
}

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
                .opt("within", d.within.as_deref())
                .field("message", &d.message)
                .line();
            // How the line was reached, then each other place the message
            // names with how that was reached, indented under the message.
            for at in &d.chain {
                out += &source_line("  from", at);
            }
            for place in &d.also {
                for (i, at) in place.iter().enumerate() {
                    out += &source_line(if i == 0 { "  also" } else { "    from" }, at);
                }
            }
        }
        if let Some(o) = &b.overflow {
            for s in &o.sections {
                out += &Row::new("past_bank_1")
                    .field("section", &s.name)
                    .field("bank", s.bank)
                    .field("start", format!("${:04X}", s.start))
                    .field("bytes", s.bytes)
                    .line();
            }
        }
        if let Some(images) = &b.images {
            out += &super::images::rows(images);
        }
        for s in &b.songs {
            out += &Row::new("song")
                .field("path", &s.path)
                .field("asm", &s.asm)
                .field("descriptor", &s.descriptor)
                .opt("tma", s.timer)
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
