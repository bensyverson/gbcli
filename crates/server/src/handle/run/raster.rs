//! `run --raster` on the wire: the engine's timeline as the lines with writes
//! on them, each write by its register's hardware.inc name.

use gbcli_engine::inspect::hwreg;
use gbcli_engine::profile::{RasterLine, RasterTimeline};
use gbcli_protocol::{RasterLineRow, RasterMade, RasterWriteRow};

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
        unsupported: t
            .gaps
            .iter()
            .map(|g| g.feature.name().to_string())
            .collect(),
    }
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
                }
            })
            .collect(),
    }
}
