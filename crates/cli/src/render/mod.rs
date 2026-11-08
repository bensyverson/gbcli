//! Responses as rows (the default) or as the response line `gbcli serve`
//! writes (`--json`). stdout carries the answer; warnings and errors go to stderr.

mod audio;
mod battery;
mod equiv;
mod images;
mod opcode;
mod profile;
mod row;
mod song;
mod stop;
mod toolchain;
mod version;
mod video;

use std::io::Write;

use gbcli_protocol::{ErrorBody, EventLogLine, Response, ResultBody};

use crate::exec::emit;
pub use row::Row;
use row::{addr, byte, bytes, hex, utc};

/// Prints a response and returns the exit code: 0, or the error's own.
pub fn response(response: &Response, json: bool, out: &mut dyn Write, err: &mut dyn Write) -> u8 {
    if json {
        // Exactly the line `gbcli serve` writes, so `rev` comes along.
        let text = serde_json::to_string(response).expect("a response always serializes");
        emit(out, &format!("{text}\n"));
        return match &response.outcome {
            Ok(_) => 0,
            Err(body) => error(err, body),
        };
    }
    for warning in &response.warnings {
        emit(err, &format!("warning: {warning}\n"));
    }
    let rev = || match response.rev {
        Some(rev) => Row::new("rev").word(&rev.to_string()).line(),
        None => String::new(),
    };
    match &response.outcome {
        Ok(result) => {
            emit(out, &rows(result));
            emit(out, &rev());
            0
        }
        Err(body) => {
            if let Some(result) = &body.result {
                // An assertion's answer: what it read when it said no.
                emit(out, &rows(result));
            }
            if let Some(stop) = &body.stop {
                // The state the failure left is still the answer's data.
                emit(out, &stop::rows(stop));
                emit(out, &video::unapplied(body.keys_unapplied.as_deref()));
                emit(out, &rev());
            }
            error(err, body)
        }
    }
}

/// Prints an error's code, message and remedy; returns its exit code.
pub fn error(err: &mut dyn Write, body: &ErrorBody) -> u8 {
    emit(
        err,
        &format!(
            "gbcli: {}: {}\nremedy: {}\n",
            body.code, body.message, body.remedy
        ),
    );
    body.exit
}

/// The rows for a result, in a fixed order; a field the op didn't fill is
/// absent and prints nothing.
pub fn rows(r: &ResultBody) -> String {
    let mut out = String::new();
    if let Some(regs) = &r.registers {
        let f = &regs.flags;
        let flags: String = [(f.z, 'Z'), (f.n, 'N'), (f.h, 'H'), (f.c, 'C')]
            .iter()
            .map(|&(set, c)| if set { c } else { '-' })
            .collect();
        out += &Row::new("registers")
            .field("a", byte(regs.a))
            .field("f", byte(regs.f))
            .field("b", byte(regs.b))
            .field("c", byte(regs.c))
            .field("d", byte(regs.d))
            .field("e", byte(regs.e))
            .field("h", byte(regs.h))
            .field("l", byte(regs.l))
            .field("sp", addr(regs.sp))
            .field("pc", addr(regs.pc))
            .field("flags", flags)
            .flag("ime", regs.ime)
            .flag("halted", regs.halted)
            .flag("double_speed", regs.double_speed)
            .line();
    }
    if let Some(ppu) = &r.ppu {
        let json = serde_json::to_value(ppu).expect("serializes");
        out += &Row::new("ppu")
            .field("ly", ppu.ly)
            .field("mode", json["mode"].as_str().unwrap_or("?"))
            .flag("lcd_on", ppu.lcd_on)
            .opt("dot", ppu.dot)
            .line();
    }
    for w in r.writes.iter().flatten() {
        let json = serde_json::to_value(w).expect("serializes");
        out += &Row::new("write")
            .field("kind", json["kind"].as_str().unwrap_or("?"))
            .field("target", &w.target)
            .field("bytes", bytes(&w.bytes))
            .opt("addr", w.addr.map(addr))
            .line();
    }
    if let Some(stop) = &r.stop {
        out += &stop::rows(stop);
    }
    for read in r.reads.iter().flatten() {
        let row = Row::new("read")
            .field("expr", &read.expr)
            .opt("value", read.value)
            .opt("hex", read.value.and_then(hex));
        out += &match &read.oam {
            Some(o) => row
                .field("y", o.y)
                .field("x", o.x)
                .field("tile", byte(o.tile))
                .field("attr", byte(o.attr))
                .flag("priority", o.priority)
                .flag("yflip", o.yflip)
                .flag("xflip", o.xflip)
                .opt("palette", o.palette)
                .opt("cgb_bank", o.cgb_bank)
                .opt("cgb_palette", o.cgb_palette),
            None => row,
        }
        .line();
    }
    if let Some(m) = &r.measure {
        out += &profile::measure(m);
    }
    if let Some(p) = &r.profile {
        out += &profile::profile(p);
    }
    if let Some(b) = &r.frame_budget {
        out += &profile::frame_budget(b);
    }
    if let Some(t) = &r.raster {
        out += &profile::raster(t);
    }
    if let Some(e) = &r.equiv {
        out += &equiv::rows(e);
    }
    out += &video::rows(r);
    if let Some(sound) = &r.audio {
        out += &audio::rows(sound);
    }
    out += &toolchain::rows(r);
    out += &song::rows(r);
    out += &opcode::rows(r);
    out += &version::rows(r);
    out += &battery::rows(r);
    if let Some(cp) = &r.checkpoint {
        out += &Row::new("checkpoint")
            .field("name", &cp.name)
            .field("path", &cp.path)
            .opt("frame", cp.frame)
            .field("rev", cp.rev)
            .line();
    }
    if let Some(restored) = &r.restored {
        out += &Row::new("restored")
            .opt("name", restored.name.as_deref())
            .opt("frame", restored.frame)
            .field("rev", restored.rev)
            .line();
    }
    if let Some(b) = &r.breakpoint {
        out += &location(Row::new("breakpoint").field("id", b.id), &b.at).line();
    }
    if let Some(w) = &r.watch {
        let json = serde_json::to_value(w).expect("serializes");
        let row = Row::new("watch")
            .field("id", w.id)
            .field("access", json["access"].as_str().unwrap_or("?"));
        out += &location(row, &w.at).line();
    }
    if let Some(d) = &r.deleted {
        out += &Row::new("deleted")
            .field("id", d.id)
            .flag("found", d.found)
            .line();
    }
    if let Some(sessions) = &r.sessions {
        for s in sessions {
            out += &Row::new("session")
                .field("name", &s.name)
                .field("rom", &s.rom)
                .field("rev", s.rev)
                .opt("head_frame", s.head_frame)
                .field("checkpoints", s.checkpoints)
                .field("size_bytes", s.size_bytes)
                .field("last_used", utc(s.last_used))
                .line();
        }
        out += none_if_empty(sessions.is_empty());
    }
    if let Some(events) = &r.events {
        for e in events {
            out += &event(e);
        }
        out += none_if_empty(events.is_empty());
    }
    if let Some(gaps) = &r.gaps {
        for g in gaps {
            out += &Row::new("gap")
                .field("feature", &g.feature)
                .field("count", g.count)
                .line();
        }
        out += none_if_empty(gaps.is_empty());
    }
    let pruned = [("pruned", &r.pruned), ("kept", &r.kept), ("busy", &r.busy)];
    for (kind, names) in pruned {
        for name in names.iter().flatten() {
            out += &Row::new(kind).field("name", name).line();
        }
    }
    if r.pruned.is_some() && pruned.iter().all(|(_, n)| n.iter().flatten().count() == 0) {
        out += "pruned none\n";
    }
    if let Some(name) = &r.removed {
        out += &Row::new("removed").field("name", name).line();
    }
    out
}

fn none_if_empty(empty: bool) -> &'static str {
    if empty { "none\n" } else { "" }
}

fn location(row: Row, at: &gbcli_protocol::Location) -> Row {
    row.field("addr", addr(at.addr))
        .opt("bank", at.bank)
        .opt("symbol", at.symbol.as_deref())
}

fn event(e: &EventLogLine) -> String {
    // The op and its args as the request spelled them (design §10).
    let json = serde_json::to_value(e).expect("serializes");
    let mut row = Row::new("event")
        .field("at", utc(e.at))
        .opt("as", e.actor.as_deref())
        .field("op", json["op"].as_str().unwrap_or("?"))
        .flag("ok", e.ok)
        .opt("code", e.code)
        .field("rev_before", e.rev_before)
        .field("rev_after", e.rev_after);
    if e.allow_rom_change {
        row = row.flag("allow_rom_change", true);
    }
    if let Some(stop) = &e.stop {
        row = row
            .field("stop", json["stop"]["reason"].as_str().unwrap_or("?"))
            .field("pc", addr(stop.pc))
            .field("frame", stop.frame);
    }
    for (feature, count) in &e.gaps {
        row = row.field(&format!("gap.{feature}"), count);
    }
    if let Some(args) = json.get("args").filter(|a| !a.is_null()) {
        row = row.json("args", args);
    }
    row.line()
}
