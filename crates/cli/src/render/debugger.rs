//! Breakpoint and watch rows: what `break`, `watch` and `delete` made, and
//! every one the session holds, as `show` lists them.

use gbcli_protocol::{Arming, DebuggerConfig, Location, ResultBody, WatchAccess};

use super::row::{Row, addr};

/// The rows for `break`, `watch` and `delete`.
pub fn made(r: &ResultBody) -> String {
    let mut out = String::new();
    if let Some(b) = &r.breakpoint {
        out += &breakpoint(b.id, &b.at, b.condition.as_deref()).line();
    }
    if let Some(w) = &r.watch {
        out += &watch(w.id, w.access, &w.at, w.condition.as_deref()).line();
    }
    if let Some(d) = &r.deleted {
        out += &Row::new("deleted")
            .field("id", d.id)
            .flag("found", d.found)
            .line();
    }
    out
}

/// `show`: a row per breakpoint and watch, each saying whether it is armed.
pub fn held(config: &DebuggerConfig) -> String {
    let mut out = String::new();
    for b in &config.breakpoints {
        out += &breakpoint(b.id, &b.at, b.condition.as_deref())
            .flag("armed", b.arming == Arming::Armed)
            .line();
    }
    for w in &config.watches {
        let mut row = watch(w.id, w.access, &w.at, w.condition.as_deref());
        if w.once {
            row = row.flag("once", true);
        }
        out += &row.flag("armed", w.arming == Arming::Armed).line();
    }
    out
}

fn breakpoint(id: u32, at: &Location, condition: Option<&str>) -> Row {
    location(Row::new("breakpoint").field("id", id), at).opt("condition", condition)
}

fn watch(id: u32, access: WatchAccess, at: &Location, condition: Option<&str>) -> Row {
    let access = match access {
        WatchAccess::Read => "read",
        WatchAccess::Write => "write",
        WatchAccess::Access => "access",
    };
    let row = Row::new("watch").field("id", id).field("access", access);
    location(row, at).opt("condition", condition)
}

fn location(row: Row, at: &Location) -> Row {
    row.field("addr", addr(at.addr))
        .opt("bank", at.bank)
        .opt("symbol", at.symbol.as_deref())
}
