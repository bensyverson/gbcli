//! Rows for the cycle profiler: a measurement, a routine profile, a frame
//! budget and a raster timeline. Means print to one decimal and frame shares
//! to three; `--json` carries them whole.

use gbcli_protocol::{
    CycleStats, FrameBudgetMade, MeasureMade, PercentStats, PpuModeView, ProfileMade, RasterMade,
};

use super::row::{Row, addr, byte};

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

/// `frame_budget frames= [partial=N] lag= lcd_off= active_min= active_max=
/// active_mean= busiest=`: the whole frames, how many partial ones the run
/// saw the end or start of (the frame it started in, the one it stopped in),
/// the lag and LCD-off frames among the whole ones, and the whole LCD-on
/// frames' summary; then a `frame_cost` row for each lag frame, by row index
/// and by frame (counted as the stop's). The frames that made their budget differ only by a few cycles, so
/// their rows would bury the ones that didn't; `--json` carries every frame,
/// the partial ones too.
pub fn frame_budget(b: &FrameBudgetMade) -> String {
    let partial = b.frames.iter().filter(|f| f.partial).count();
    let mut out = Row::new("frame_budget")
        .field("frames", b.frames.len() - partial)
        .opt("partial", (partial > 0).then_some(partial))
        .field("lag", b.lag)
        .field("lcd_off", b.lcd_off)
        .opt("active_min", b.active_t.map(|s| s.min))
        .opt("active_max", b.active_t.map(|s| s.max))
        .opt(
            "active_mean",
            b.active_t.map(|s| format!("{:.1}", s.mean.0)),
        )
        .opt("busiest", b.busiest)
        .line();
    for (i, f) in b.frames.iter().enumerate() {
        if f.lag != Some(true) {
            continue;
        }
        out += &Row::new("frame_cost")
            .field("index", i)
            .field("frame", f.frame)
            .field("active_t", f.active_t)
            .field("idle_t", f.idle_t)
            .field("lag", 1)
            .line();
    }
    out
}

/// `raster frames= lines= written=`, then each line with writes and its
/// writes under it. A line with the LCD off reads `line=off`; a line with no
/// reported start has no `start=` and its writes no `into=`; a write whose
/// mode the core can't know has no `mode=`.
pub fn raster(t: &RasterMade) -> String {
    let mut out = Row::new("raster")
        .field("frames", t.frames)
        .field("lines", t.lines)
        .field("written", t.rows.len())
        .line();
    for r in &t.rows {
        let line = r.line.map_or_else(|| "off".to_string(), |l| l.to_string());
        out += &Row::new("raster_line")
            .field("frame", r.frame)
            .field("line", line)
            .opt("start", r.start)
            .line();
        for w in &r.writes {
            out += &Row::new("raster_write")
                .field("reg", &w.reg)
                .field("value", byte(w.value))
                .field("pc", addr(w.pc))
                .opt("into", w.ticks_into_line)
                .opt("mode", w.mode.map(PpuModeView::name))
                .line();
        }
    }
    for feature in &t.unsupported {
        out += &Row::new("raster_unsupported")
            .field("feature", feature)
            .line();
    }
    out
}

#[cfg(test)]
mod tests {
    use gbcli_protocol::{CycleStats, FrameBudgetMade, FrameBudgetRow, Real};

    use super::frame_budget;

    fn frame(n: i64, active_t: u64, lag: Option<bool>, partial: bool) -> FrameBudgetRow {
        FrameBudgetRow {
            frame: n,
            active_t,
            idle_t: 70224 - active_t,
            lag,
            lcd_off: false,
            partial,
        }
    }

    /// Only the lag frames get a row: sixty near-identical rows for sixty
    /// healthy frames were the flood two agent evals complained of, and the
    /// header already sums them up (leaf k22J86). An LCD-off frame is
    /// counted in the header, apart from lag, and gets no row (leaf emtFvQ).
    #[test]
    fn rows_list_only_the_lag_frames_under_the_summary() {
        let loading = FrameBudgetRow {
            lcd_off: true,
            ..frame(40, 70224, Some(false), false)
        };
        let made = FrameBudgetMade {
            frames: vec![
                loading,
                frame(41, 30000, Some(false), false),
                frame(42, 70224, Some(true), false),
                frame(43, 31000, Some(false), false),
                frame(44, 1000, None, true),
            ],
            lag: 1,
            lcd_off: 1,
            active_t: Some(CycleStats {
                min: 30000,
                max: 70224,
                mean: Real(43741.3),
            }),
            busiest: Some(2),
        };
        assert_eq!(
            frame_budget(&made),
            "frame_budget frames=4 partial=1 lag=1 lcd_off=1 active_min=30000 \
             active_max=70224 active_mean=43741.3 busiest=2\n\
             frame_cost index=2 frame=42 active_t=70224 idle_t=0 lag=1\n"
        );
    }
}
