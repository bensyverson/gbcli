//! Rows for the cycle profiler: a measurement, a routine profile, a frame
//! budget and a raster timeline. Means print to one decimal and frame shares
//! to three; `--json` carries them whole.

use gbcli_protocol::{
    CycleStats, FrameBudgetMade, MeasureMade, PercentStats, PpuModeView, ProfileMade, RasterMade,
};

use super::row::{Row, addr, byte, capped};

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
            .field("idle_t", r.idle_t)
            .field("inclusive_t", r.inclusive_t)
            .line();
    }
    out
}

/// `frame_budget frames= [partial=N] lag= lcd_off= active_min= active_max=
/// active_mean= busiest=`: the whole frames, how many partial ones the run
/// saw the end or start of (the frame it started in, the one it stopped in),
/// the lag and LCD-off frames among the whole ones, and the whole LCD-on
/// frames' summary; then `frame_lag from= to= count= active_max= index=`
/// for each run of adjacent lag frames (frames counted as the stop's, `index`
/// the first one's row in `--json`), capped. The frames that made their
/// budget differ only by a few cycles, so their rows would bury the ones
/// that didn't, and a game lags in bursts, so a row per lag frame buried the
/// bursts; `--json` carries every frame, the partial ones too.
pub fn frame_budget(b: &FrameBudgetMade) -> String {
    let partial = b.frames.iter().filter(|f| f.partial).count();
    let out = Row::new("frame_budget")
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
    out + &capped("frame_lag", &b.lag_runs(), |run| {
        Row::new("frame_lag")
            .field("from", run.from)
            .field("to", run.to)
            .field("count", run.count)
            .field("active_max", run.active_max)
            .field("index", run.index)
            .line()
    })
}

/// `raster frames= lines= written=`, then each line with writes and its
/// writes under it. A line with the LCD off reads `line=off`; a line with no
/// reported start has no `start=` and its writes no `into=`; a write whose
/// mode the core can't know has no `mode=`. Before the lines, a
/// `raster_writer reg= pc= writes=` row per register and writer PC counts
/// its writes by the mode they landed in (`unknown=` where the core
/// couldn't tell), so a whole run's answer survives the lines' cap; `--json`
/// has every line.
pub fn raster(t: &RasterMade) -> String {
    let mut out = Row::new("raster")
        .field("frames", t.frames)
        .field("lines", t.lines)
        .field("written", t.rows.len())
        .line();
    let count = |n: u64| (n > 0).then_some(n);
    for w in t.writers() {
        out += &Row::new("raster_writer")
            .field("reg", &w.reg)
            .field("pc", addr(w.pc))
            .field("writes", w.writes)
            .opt("hblank", count(w.hblank))
            .opt("vblank", count(w.vblank))
            .opt("oam_scan", count(w.oam_scan))
            .opt("drawing", count(w.drawing))
            .opt("unknown", count(w.unknown))
            .line();
    }
    out += &capped("raster_line", &t.rows, |r| {
        let line = r.line.map_or_else(|| "off".to_string(), |l| l.to_string());
        let mut rows = Row::new("raster_line")
            .field("frame", r.frame)
            .field("line", line)
            .opt("start", r.start)
            .line();
        for w in &r.writes {
            rows += &Row::new("raster_write")
                .field("reg", &w.reg)
                .field("value", byte(w.value))
                .field("pc", addr(w.pc))
                .opt("into", w.ticks_into_line)
                .opt("mode", w.mode.map(PpuModeView::name))
                .line();
        }
        rows
    });
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
    use crate::render::row::CAP;

    fn budget(frames: Vec<FrameBudgetRow>) -> FrameBudgetMade {
        FrameBudgetMade {
            frames,
            lag: 0,
            lcd_off: 0,
            active_t: None,
            busiest: None,
        }
    }

    fn frame(n: i64, active_t: u64, lag: Option<bool>, partial: bool) -> FrameBudgetRow {
        FrameBudgetRow {
            frame: n,
            active_t,
            idle_t: 70224u64.saturating_sub(active_t),
            lag,
            lcd_off: false,
            partial,
        }
    }

    /// Only the lag frames get a row: sixty near-identical rows for sixty
    /// healthy frames were the flood two agent evals complained of, and the
    /// header already sums them up (leaf k22J86). An LCD-off frame is
    /// counted in the header, apart from lag, and gets no row (leaf emtFvQ).
    /// A row stands for a run of adjacent lag frames, not one frame
    /// (raycaster DX review C-7), so this lone lag frame is a run of one.
    #[test]
    fn rows_list_only_the_lag_runs_under_the_summary() {
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
             frame_lag from=42 to=42 count=1 active_max=70224 index=2\n"
        );
    }

    /// A long `--raster` run printed a row per line with writes; the
    /// question it answers (does every write land in HBlank?) is a row per
    /// writer, counted by mode, then the lines, capped.
    #[test]
    fn a_raster_sums_up_each_writer_by_mode_then_caps_its_lines() {
        use gbcli_protocol::{PpuModeView, RasterLineRow, RasterMade, RasterWriteRow};
        let rows = (0..CAP as u8 + 2)
            .map(|line| RasterLineRow {
                frame: 0,
                line: Some(line),
                start: None,
                writes: vec![RasterWriteRow {
                    reg: "rSCX".into(),
                    value: line,
                    pc: 0x16B,
                    ticks_into_line: None,
                    mode: Some(if line == 3 {
                        PpuModeView::Drawing
                    } else {
                        PpuModeView::Hblank
                    }),
                }],
            })
            .collect();
        let text = super::raster(&RasterMade {
            frames: 1,
            lines: 154,
            rows,
            unsupported: vec!["ppu_dot".into()],
        });
        let lines: Vec<&str> = text.lines().collect();
        assert_eq!(
            lines[0],
            format!("raster frames=1 lines=154 written={}", CAP + 2)
        );
        assert_eq!(
            lines[1],
            format!(
                "raster_writer reg=rSCX pc=$016B writes={} hblank={} drawing=1",
                CAP + 2,
                CAP + 1
            )
        );
        assert_eq!(lines[2], "raster_line frame=0 line=0");
        assert!(
            text.ends_with(
                "\nmore kind=raster_line count=2 see=--json\nraster_unsupported feature=ppu_dot\n"
            ),
            "{text}"
        );
    }

    /// Lantern's 600-frame walk printed 545 `frame_cost` rows (33 KB) that
    /// fell into 27 runs: adjacent lag frames are one row, and past the cap
    /// one row says how many runs `--json` has besides.
    #[test]
    fn adjacent_lag_frames_are_one_row_and_the_runs_are_capped() {
        let burst: Vec<FrameBudgetRow> = (100..=126)
            .map(|n| frame(n, 70224 + (n as u64 % 3), Some(true), false))
            .collect();
        let mut frames = vec![frame(99, 30000, Some(false), false)];
        frames.extend(burst);
        frames.push(frame(127, 30000, Some(false), false));
        let made = FrameBudgetMade {
            lag: 27,
            ..budget(frames)
        };
        assert!(
            frame_budget(&made)
                .ends_with("\nframe_lag from=100 to=126 count=27 active_max=70226 index=1\n"),
            "{}",
            frame_budget(&made)
        );

        // Every other frame lags: CAP + 3 runs of one frame.
        let n = 2 * (CAP as i64 + 3);
        let frames = (0..n)
            .map(|i| frame(i, 70224, Some(i % 2 == 0), false))
            .collect();
        let text = frame_budget(&budget(frames));
        assert_eq!(text.lines().count(), 1 + CAP + 1, "{text}");
        assert!(
            text.ends_with("\nmore kind=frame_lag count=3 see=--json\n"),
            "{text}"
        );
    }
}
