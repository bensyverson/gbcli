//! Building the stop record: place, time with its named base, frame shown.

use gbcli_core::Usage;
use gbcli_protocol::{FrameShown, StopReason, StopRecord, StopTime, TimeBase, TimeMark};

use super::Since;
use crate::addr::label;
use crate::cond::eval::frame_number;
use crate::machine::Machine;
use crate::mem;

/// The time base a record counts from, and its mark. `Boot` before the
/// handoff has been seen falls back to power-on, and the record says so.
pub fn base(m: &Machine, since: &Since) -> (TimeBase, TimeMark) {
    let power_on = (TimeBase::PowerOn, TimeMark { ticks: 0, frame: 0 });
    match since {
        Since::PowerOn => power_on,
        Since::Boot => m.boot().map_or(power_on, |mark| (TimeBase::Boot, mark)),
        Since::Checkpoint { name, mark } => (TimeBase::Checkpoint(name.clone()), *mark),
    }
}

pub fn record(m: &Machine, reason: StopReason, used: Usage, since: &Since) -> StopRecord {
    let core = m.core();
    let regs = core.registers();
    let bank = mem::bank_at(core, regs.pc);
    let symbol = m
        .symbols()
        .and_then(|s| s.containing(bank, regs.pc))
        .map(|s| label(&s.name, i64::from(regs.pc - s.addr)));
    let (since, mark) = base(m, since);
    let ticks = core.now().0;
    let per_t = if regs.double_speed { 1 } else { 2 };
    let ppu = core.ppu();
    StopRecord {
        reason,
        pc: regs.pc,
        bank,
        symbol,
        time: StopTime {
            ticks,
            t: (ticks as i64 - mark.ticks as i64) / per_t,
            since,
            frame: frame_number(core) as i64 - mark.frame as i64,
            ly: ppu.ly,
            dot: ppu.dot,
        },
        frame_shown: core.last_complete_frame().map(|f| FrameShown {
            n: f.tag.number as i64 - mark.frame as i64,
            vblank: f.tag.kind,
        }),
        budget: used.into(),
    }
}
