//! `run --raster` on the wire: the engine's timeline as the lines with writes
//! on them, each write by its register's hardware.inc name.

use gbcli_engine::inspect::hwreg;
use gbcli_engine::profile::{RasterLine, RasterTimeline};
use gbcli_protocol::{RasterFrameScroll, RasterLineRow, RasterMade, RasterWriteRow};

pub(super) fn raster_made(t: &RasterTimeline) -> RasterMade {
    let frames = match (t.lines.first(), t.lines.last()) {
        (Some(first), Some(last)) => last.frame - first.frame + 1,
        _ => 0,
    };
    RasterMade {
        frames,
        lines: t.lines.len() as u64,
        rows: t
            .lines
            .iter()
            .filter(|l| !l.writes.is_empty())
            .map(row)
            .collect(),
        drawn: drawn(t),
        unsupported: t
            .gaps
            .iter()
            .map(|g| g.feature.name().to_string())
            .collect(),
    }
}

/// The drawn lines, a run of consecutive lines to an entry, since an
/// array's index is its line.
fn drawn(t: &RasterTimeline) -> Vec<RasterFrameScroll> {
    let mut out: Vec<RasterFrameScroll> = Vec::new();
    for l in &t.lines {
        let (Some(line), Some(scroll)) = (l.line, l.scroll) else {
            continue;
        };
        let next = |f: &RasterFrameScroll| usize::from(f.first_line) + f.scx.len();
        match out.last_mut() {
            Some(f) if f.frame == l.frame && next(f) == usize::from(line) => {
                f.scx.push(scroll.scx);
                f.scy.push(scroll.scy);
            }
            _ => out.push(RasterFrameScroll {
                frame: l.frame,
                first_line: line,
                scx: vec![scroll.scx],
                scy: vec![scroll.scy],
            }),
        }
    }
    out
}

fn row(l: &RasterLine) -> RasterLineRow {
    RasterLineRow {
        frame: l.frame,
        line: l.line,
        start: l.start.map(|t| t.0),
        writes: l
            .writes
            .iter()
            .map(|w| {
                let addr = 0xFF00 | u16::from(w.reg.0);
                RasterWriteRow {
                    reg: hwreg::by_addr(addr)
                        .map_or_else(|| format!("${addr:04X}"), |r| r.name.to_string()),
                    value: w.value,
                    pc: w.pc,
                    ticks_into_line: l.ticks_into_line(w),
                    mode: w.mode.map(Into::into),
                }
            })
            .collect(),
    }
}

#[cfg(test)]
mod tests {
    use gbcli_engine::profile::LineScroll;

    use super::*;

    fn line(frame: u32, line: Option<u8>, scroll: Option<(u8, u8)>) -> RasterLine {
        RasterLine {
            frame,
            line,
            start: None,
            modes: Vec::new(),
            writes: Vec::new(),
            scroll: scroll.map(|(scx, scy)| LineScroll { scx, scy }),
        }
    }

    #[test]
    fn drawn_lines_group_by_frame_from_the_first_line_seen() {
        let t = RasterTimeline {
            lines: vec![
                line(0, Some(142), Some((1, 2))),
                line(0, Some(143), Some((3, 4))),
                line(0, Some(144), None),
                line(1, Some(0), Some((5, 6))),
            ],
            gaps: Vec::new(),
        };
        assert_eq!(
            raster_made(&t).drawn,
            vec![
                RasterFrameScroll {
                    frame: 0,
                    first_line: 142,
                    scx: vec![1, 3],
                    scy: vec![2, 4],
                },
                RasterFrameScroll {
                    frame: 1,
                    first_line: 0,
                    scx: vec![5],
                    scy: vec![6],
                },
            ]
        );
    }

    /// An array's index is its line, so a hole can't be closed up.
    #[test]
    fn a_line_with_no_scroll_inside_a_frame_starts_a_new_run_of_lines() {
        let t = RasterTimeline {
            lines: vec![
                line(0, Some(10), Some((1, 0))),
                line(0, Some(11), None),
                line(0, Some(12), Some((2, 0))),
            ],
            gaps: Vec::new(),
        };
        let firsts: Vec<u8> = raster_made(&t).drawn.iter().map(|f| f.first_line).collect();
        assert_eq!(firsts, vec![10, 12]);
    }
}
