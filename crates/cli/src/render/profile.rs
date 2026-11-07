//! Rows for the cycle profiler: a measurement, a routine profile and a frame
//! budget. Means print to one decimal and frame shares to three; `--json`
//! carries them whole.

use gbcli_protocol::{CycleStats, FrameBudgetMade, MeasureMade, PercentStats, ProfileMade};

use super::row::{Row, addr};

pub fn measure(m: &MeasureMade) -> String {
    let speed = serde_json::to_value(m.speed).expect("serializes");
    let cycles = |kind: &str, s: &CycleStats| {
        Row::new(kind)
            .field("min", s.min)
            .field("max", s.max)
            .field("mean", format!("{:.1}", s.mean.0))
            .line()
    };
    let pct = |s: &PercentStats| {
        Row::new("frame_pct")
            .field("min", format!("{:.3}", s.min.0))
            .field("max", format!("{:.3}", s.max.0))
            .field("mean", format!("{:.3}", s.mean.0))
            .line()
    };
    Row::new("measure")
        .field("from", &m.from)
        .field("to", &m.to)
        .field("n", m.n)
        .field("speed", speed.as_str().unwrap_or("?"))
        .opt("max_t", m.max_t)
        .line()
        + &cycles("t", &m.t)
        + &cycles("m", &m.m)
        + &pct(&m.frame_pct)
}

pub fn profile(p: &ProfileMade) -> String {
    let mut out = Row::new("profile")
        .field("total_t", p.total_t)
        .field("idle_t", p.idle_t)
        .field("routines", p.routines.len())
        .opt("omitted", (p.omitted > 0).then_some(p.omitted))
        .field("unwound", p.unwound)
        .field("mismatched", p.mismatched)
        .field("ret_jumps", p.ret_jumps)
        .field("open", p.open)
        .line();
    for r in &p.routines {
        out += &Row::new("routine")
            .opt("name", r.name.as_deref())
            .field("bank", r.bank)
            .field("addr", addr(r.addr))
            .field("calls", r.calls)
            .field("exclusive_t", r.exclusive_t)
            .field("inclusive_t", r.inclusive_t)
            .line();
    }
    out
}

pub fn frame_budget(b: &FrameBudgetMade) -> String {
    let mut out = Row::new("frame_budget")
        .field("frames", b.frames.len())
        .field("lag", b.lag)
        .line();
    for (i, f) in b.frames.iter().enumerate() {
        out += &Row::new("frame_cost")
            .field("index", i)
            .field("active_t", f.active_t)
            .field("idle_t", f.idle_t)
            .flag("lag", f.lag)
            .line();
    }
    out
}
