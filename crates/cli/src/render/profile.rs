//! Rows for the cycle profiler: a measurement, a routine profile, a frame
//! budget and a raster timeline. Means print to one decimal and frame shares
//! to three; `--json` carries them whole.

use gbcli_protocol::{
    CycleStats, FrameBudgetMade, MeasureMade, PercentStats, PpuModeView, ProfileMade, RasterMade,
    RasterWriter,
};

use super::row::{CAP, Row, addr, byte, capped};

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

/// The most rows the raster timeline prints, its `more` rows included. With
/// the header, the capped writer rows and the unsupported features, the
/// whole raster text stays within about 64 lines however long the run or
/// big the program.
pub const RASTER_TIMELINE_ROWS: usize = 2 * CAP;

/// `raster frames= lines= written=`, then a `raster_writer reg= pc= writes=`
/// row per register and writer PC counting its writes by the mode they landed
/// in (`unknown=` where the core couldn't tell, `lcd_off=` with the LCD off),
/// riskiest first and capped: the `more` row counts the writers left out and,
/// by mode, any of their writes that landed in OAM scan, drawing or an
/// unknown mode, so a whole run's answer survives the cap. Then the timeline:
/// each line with writes and its writes under it, at most [`CAP`] lines and
/// [`CAP`] writes to a line, and at most [`RASTER_TIMELINE_ROWS`] rows in all
/// (an LCD-off stretch is one line holding every write made while the LCD was
/// off). A line with the LCD off reads `line=off`; a line with no reported
/// start has no `start=` and its writes no `into=`; a write whose mode the
/// core can't know has no `mode=`. `--json` has every line and write.
pub fn raster(t: &RasterMade) -> String {
    let mut out = Row::new("raster")
        .field("frames", t.frames)
        .field("lines", t.lines)
        .field("written", t.rows.len())
        .line();
    out += &raster_writers(t);
    out += &raster_timeline(t);
    for feature in &t.unsupported {
        out += &Row::new("raster_unsupported")
            .field("feature", feature)
            .line();
    }
    out
}

fn raster_writers(t: &RasterMade) -> String {
    let count = |n: u64| (n > 0).then_some(n);
    let writers = t.writers();
    let mut out = String::new();
    for w in writers.iter().take(CAP) {
        out += &Row::new("raster_writer")
            .field("reg", &w.reg)
            .field("pc", addr(w.pc))
            .field("writes", w.writes)
            .opt("hblank", count(w.hblank))
            .opt("vblank", count(w.vblank))
            .opt("oam_scan", count(w.oam_scan))
            .opt("drawing", count(w.drawing))
            .opt("unknown", count(w.unknown))
            .opt("lcd_off", count(w.lcd_off))
            .line();
    }
    let rest = writers.get(CAP..).unwrap_or_default();
    if !rest.is_empty() {
        let sum = |mode: fn(&RasterWriter) -> u64| count(rest.iter().map(mode).sum());
        out += &Row::new("more")
            .field("kind", "raster_writer")
            .field("count", rest.len())
            .opt("oam_scan", sum(|w| w.oam_scan))
            .opt("drawing", sum(|w| w.drawing))
            .opt("unknown", sum(|w| w.unknown))
            .field("see", "--json")
            .line();
    }
    out
}

/// The lines in order until [`RASTER_TIMELINE_ROWS`] would be passed, always
/// keeping a row for the `more` row the lines left out need.
fn raster_timeline(t: &RasterMade) -> String {
    let more = |kind: &str, count: usize| {
        Row::new("more")
            .field("kind", kind)
            .field("count", count)
            .field("see", "--json")
            .line()
    };
    let mut out = String::new();
    let mut left = RASTER_TIMELINE_ROWS;
    let mut shown = 0;
    for (i, r) in t.rows.iter().enumerate() {
        let reserve = usize::from(i + 1 < t.rows.len());
        let room = left - reserve;
        // A line needs room for one of its writes and, past that, a more row.
        if i == CAP || room < (1 + r.writes.len()).min(3) {
            break;
        }
        let line = r.line.map_or_else(|| "off".to_string(), |l| l.to_string());
        out += &Row::new("raster_line")
            .field("frame", r.frame)
            .field("line", line)
            .opt("start", r.start)
            .line();
        // Rows left under the line row, for its writes and any more row.
        let under = room - 1;
        let take = if r.writes.len() <= under.min(CAP) {
            r.writes.len()
        } else {
            CAP.min(under - 1)
        };
        for w in &r.writes[..take] {
            out += &Row::new("raster_write")
                .field("reg", &w.reg)
                .field("value", byte(w.value))
                .field("pc", addr(w.pc))
                .opt("into", w.ticks_into_line)
                .opt("mode", w.mode.map(PpuModeView::name))
                .line();
        }
        let cut = r.writes.len() - take;
        if cut > 0 {
            out += &more("raster_write", cut);
        }
        left -= 1 + take + usize::from(cut > 0);
        shown += 1;
    }
    if shown < t.rows.len() {
        out += &more("raster_line", t.rows.len() - shown);
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
            drawn: Vec::new(),
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
                "\nmore kind=raster_line count=3 see=--json\nraster_unsupported feature=ppu_dot\n"
            ),
            "{text}"
        );
    }

    fn raster_write(
        reg: &str,
        pc: u16,
        mode: Option<gbcli_protocol::PpuModeView>,
    ) -> gbcli_protocol::RasterWriteRow {
        gbcli_protocol::RasterWriteRow {
            reg: reg.into(),
            value: 0,
            pc,
            ticks_into_line: None,
            mode,
        }
    }

    fn raster_of(lines: Vec<Vec<gbcli_protocol::RasterWriteRow>>) -> gbcli_protocol::RasterMade {
        gbcli_protocol::RasterMade {
            frames: 1,
            lines: 154,
            rows: lines
                .into_iter()
                .enumerate()
                .map(|(i, writes)| gbcli_protocol::RasterLineRow {
                    frame: 0,
                    line: Some(i as u8),
                    start: None,
                    writes,
                })
                .collect(),
            drawn: Vec::new(),
            unsupported: Vec::new(),
        }
    }

    /// An adventure game has hundreds of writer sites (Ben, 2026-11-09):
    /// the `raster_writer` rows are capped like any list, the risky writers
    /// first, and the `more` row counts the writers left out and the writes
    /// among them that weren't safe, so `count=` alone says the rest passed.
    #[test]
    fn raster_writers_are_capped_riskiest_first_and_the_more_row_sums_the_rest() {
        use gbcli_protocol::PpuModeView::{Drawing, Hblank};
        // Twenty-five safe writers, then twenty-two that wrote while drawing.
        let mut lines: Vec<_> = (0..25u16)
            .map(|i| vec![raster_write("rSCX", 0x1000 + i, Some(Hblank))])
            .collect();
        lines.extend((0..22u16).map(|i| vec![raster_write("rBGP", 0x2000 + i, Some(Drawing))]));
        let text = super::raster(&raster_of(lines));
        let writers: Vec<&str> = text
            .lines()
            .filter(|l| l.starts_with("raster_writer "))
            .collect();
        assert_eq!(writers.len(), CAP, "{text}");
        assert_eq!(
            writers[0],
            "raster_writer reg=rBGP pc=$2000 writes=1 drawing=1"
        );
        assert!(writers.iter().all(|l| l.ends_with(" drawing=1")), "{text}");
        assert!(
            text.contains("\nmore kind=raster_writer count=27 drawing=2 see=--json\n"),
            "{text}"
        );
        let safe_only = raster_of(
            (0..CAP as u16 + 3)
                .map(|i| vec![raster_write("rSCX", 0x1000 + i, Some(Hblank))])
                .collect(),
        );
        assert!(
            super::raster(&safe_only).contains("\nmore kind=raster_writer count=3 see=--json\n"),
            "a more row over safe writers names no mode"
        );
    }

    /// The timeline shares one budget of rows, its `more` rows included: 20
    /// lines of 20 writes each was 440 rows. A line cut short says how many
    /// of its writes are left; the last row, how many lines.
    #[test]
    fn the_raster_timeline_stops_at_its_row_budget() {
        use gbcli_protocol::PpuModeView::Hblank;
        let lines = (0..30)
            .map(|_| {
                (0..30)
                    .map(|_| raster_write("rSCX", 0x150, Some(Hblank)))
                    .collect()
            })
            .collect();
        let text = super::raster(&raster_of(lines));
        let timeline: Vec<&str> = text
            .lines()
            .skip_while(|l| !l.starts_with("raster_line "))
            .collect();
        assert_eq!(timeline.len(), 2 * CAP, "{text}");
        assert_eq!(timeline[21], "more kind=raster_write count=10 see=--json");
        assert_eq!(timeline[22], "raster_line frame=0 line=1");
        assert_eq!(timeline[38], "more kind=raster_write count=15 see=--json");
        assert_eq!(timeline[39], "more kind=raster_line count=28 see=--json");
        // A run that fits prints whole, with no more rows.
        let few = raster_of(vec![vec![raster_write("rSCX", 0x150, Some(Hblank)); 3]; 4]);
        assert!(!super::raster(&few).contains("more"));
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
