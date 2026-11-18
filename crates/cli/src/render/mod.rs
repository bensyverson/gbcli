//! Responses as rows (the default) or as the response line `gbcli serve`
//! writes (`--json`). stdout carries the answer; warnings and errors go to stderr.

mod audio;
mod battery;
#[cfg(test)]
mod bounded_tests;
mod created;
mod debugger;
mod equiv;
mod font;
mod images;
mod opcode;
mod pixels;
mod profile;
#[cfg(test)]
mod raster_property_tests;
mod row;
mod song;
mod stop;
mod toolchain;
mod version;
mod video;

use std::io::Write;

use gbcli_protocol::{CgbFlag, ErrorBody, EventLogLine, PpuModeView, Response, ResultBody};

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
    emit(out, &session_created(response));
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
            // An assertion's answer: what it read when it said no. A run's
            // (--expect) moved the head, so it names the rev.
            let answered_stop = body.result.as_deref().and_then(|result| {
                emit(out, &rows(result));
                result.stop.as_ref()
            });
            if let Some(stop) = &body.stop {
                // The state the failure left is still the answer's data,
                // printed once when the answer already showed it.
                if answered_stop != Some(stop) {
                    emit(out, &stop::rows(stop));
                    emit(out, &video::unapplied(body.keys_unapplied.as_deref()));
                }
                emit(out, &rev());
            } else if answered_stop.is_some() {
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

/// The rows for the session the invocation created, if it created one:
/// first, as the creation came before the request.
pub fn session_created(response: &Response) -> String {
    response
        .session_created
        .as_ref()
        .map(created::rows)
        .unwrap_or_default()
}

/// The rows for a result, in a fixed order; a field the op didn't fill is
/// absent and prints nothing.
pub fn rows(r: &ResultBody) -> String {
    let mut out = String::new();
    if let Some(m) = &r.machine {
        let json = serde_json::to_value(m).expect("serializes");
        out += &Row::new("machine")
            .field("model", json["model"].as_str().unwrap_or("?"))
            .opt("cgb", m.cgb.map(CgbFlag::name))
            .line();
    }
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
        out += &Row::new("ppu")
            .field("ly", ppu.ly)
            .field("mode", ppu.mode.map_or("unknown", PpuModeView::name))
            .flag("lcd_on", ppu.lcd_on)
            .opt("dot", ppu.dot)
            .line();
    }
    if let Some(config) = &r.debugger {
        out += &debugger::held(config);
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
            .opt("hex", read.value.and_then(hex))
            .opt("error", read.error.as_ref().map(|e| e.code))
            .opt("message", read.error.as_ref().map(|e| &e.message))
            .opt("remedy", read.error.as_ref().map(|e| &e.remedy));
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
    for e in r.expect.iter().flatten() {
        out += &Row::new("expect")
            .field("expr", &e.expr)
            .opt("value", e.value)
            .flag("ok", e.ok)
            .opt("error", e.error.as_ref().map(|e| e.code))
            .opt("message", e.error.as_ref().map(|e| &e.message))
            .opt("remedy", e.error.as_ref().map(|e| &e.remedy))
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
    out += &pixels::rows(r);
    out += &font::rows(r);
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
    out += &debugger::made(r);
    if let Some(sessions) = &r.sessions {
        for s in sessions {
            let json = serde_json::to_value(s).expect("serializes");
            out += &Row::new("session")
                .field("name", &s.name)
                .field("rom", &s.rom)
                .field("model", json["model"].as_str().unwrap_or("?"))
                .opt("cgb", s.cgb.map(CgbFlag::name))
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

#[cfg(test)]
mod tests {
    use gbcli_protocol::{PpuModeView, PpuView, ResultBody};

    fn ppu_row(mode: Option<PpuModeView>) -> String {
        let r = ResultBody {
            ppu: Some(PpuView {
                ly: 50,
                mode,
                lcd_on: true,
                dot: None,
            }),
            ..ResultBody::default()
        };
        super::rows(&r)
    }

    #[test]
    fn an_expect_row_names_the_expression_its_value_and_the_verdict() {
        use gbcli_protocol::{ErrorCode, ExpectValue, ReadFailed};
        let r = ResultBody {
            expect: Some(vec![
                ExpectValue {
                    expr: "[wFail] == 0".into(),
                    value: Some(0),
                    ok: false,
                    error: None,
                },
                ExpectValue {
                    expr: "[wNever]".into(),
                    value: None,
                    ok: false,
                    error: Some(ReadFailed {
                        code: ErrorCode::UninitializedRead,
                        message: "never written".into(),
                        remedy: "--allow-uninit".into(),
                    }),
                },
            ]),
            ..ResultBody::default()
        };
        assert_eq!(
            super::rows(&r),
            "expect expr=\"[wFail] == 0\" value=0 ok=0\n\
             expect expr=[wNever] ok=0 error=uninitialized_read message=\"never written\" \
             remedy=--allow-uninit\n"
        );
    }

    fn place(addr: u16, symbol: Option<&str>) -> gbcli_protocol::Location {
        gbcli_protocol::Location {
            addr,
            bank: None,
            symbol: symbol.map(String::from),
        }
    }

    #[test]
    fn break_and_watch_rows_echo_the_condition() {
        use gbcli_protocol::{BreakpointMade, WatchAccess, WatchMade};
        let r = ResultBody {
            breakpoint: Some(BreakpointMade {
                id: 1,
                at: place(0x0170, Some("Work.done")),
                condition: Some("[wFrame] == 3".into()),
            }),
            watch: Some(WatchMade {
                id: 2,
                access: WatchAccess::Write,
                at: place(0xC000, None),
                condition: None,
            }),
            ..ResultBody::default()
        };
        assert_eq!(
            super::rows(&r),
            "breakpoint id=1 addr=$0170 symbol=Work.done condition=\"[wFrame] == 3\"\n\
             watch id=2 access=write addr=$C000\n"
        );
    }

    /// `show` lists every breakpoint and watch, each saying whether it is
    /// armed: "nothing fired" is only a proof when the traps were armed.
    #[test]
    fn show_rows_list_every_breakpoint_and_watch_and_whether_it_is_armed() {
        use gbcli_protocol::{Arming, BreakpointSpec, DebuggerConfig, WatchAccess, WatchSpec};
        let r = ResultBody {
            debugger: Some(DebuggerConfig {
                breakpoints: vec![BreakpointSpec {
                    id: 1,
                    at: place(0x0170, Some("Work.done")),
                    condition: Some("mode != 1".into()),
                    arming: Arming::Armed,
                }],
                watches: vec![WatchSpec {
                    id: 3,
                    access: WatchAccess::Write,
                    at: place(0xC003, Some("wGone")),
                    condition: None,
                    once: true,
                    arming: Arming::Disarmed,
                }],
                next_id: 4,
            }),
            ..ResultBody::default()
        };
        assert_eq!(
            super::rows(&r),
            "breakpoint id=1 addr=$0170 symbol=Work.done condition=\"mode != 1\" armed=1\n\
             watch id=3 access=write addr=$C003 symbol=wGone once=1 armed=0\n"
        );
        let none = ResultBody {
            debugger: Some(DebuggerConfig::default()),
            ..ResultBody::default()
        };
        assert_eq!(super::rows(&none), "");
    }

    #[test]
    fn the_ppu_row_says_when_the_mode_is_unknown() {
        assert_eq!(ppu_row(None), "ppu ly=50 mode=unknown lcd_on=1\n");
        assert_eq!(
            ppu_row(Some(PpuModeView::Drawing)),
            "ppu ly=50 mode=drawing lcd_on=1\n"
        );
    }
}
