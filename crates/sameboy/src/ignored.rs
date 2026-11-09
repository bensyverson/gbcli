//! The ignored-write log ([`Feature::IgnoredWriteLog`]): the program's CPU
//! writes the PPU drops, which the mode-blocked-writes patch reports from
//! inside the write. The callback is installed for the instance's lifetime;
//! SameBoy calls it only on the blocked path, so it costs nothing while no
//! write is dropped.
//!
//! [`Feature::IgnoredWriteLog`]: gbcli_core::Feature::IgnoredWriteLog

use gbcli_core::{IgnoredWriteKind, IgnoredWrites, Ticks};
use sameboy_sys as sb;

use crate::SameBoy;
use crate::hooks::guard;

/// What the log has counted, per kind, and the current step's share.
#[derive(Debug, Default)]
pub(crate) struct IgnoredLog {
    /// The drops of the step in progress, not yet stamped with its PC.
    step: [Option<Pending>; 2],
    /// Counted since the log was turned on or last taken.
    found: [Option<IgnoredWrites>; 2],
}

/// One kind's drops during one step.
#[derive(Clone, Copy, Debug)]
struct Pending {
    count: u64,
    /// The first's address and line.
    addr: u16,
    line: u8,
}

fn slot(kind: IgnoredWriteKind) -> usize {
    match kind {
        IgnoredWriteKind::Vram => 0,
        IgnoredWriteKind::Oam => 1,
    }
}

impl IgnoredLog {
    fn note(&mut self, kind: IgnoredWriteKind, addr: u16, line: u8) {
        match &mut self.step[slot(kind)] {
            Some(pending) => pending.count += 1,
            empty => {
                *empty = Some(Pending {
                    count: 1,
                    addr,
                    line,
                })
            }
        }
    }

    /// Folds the step's drops into the counts, the step having begun at
    /// `pc` and `at` with `frame` the last complete frame's number.
    fn stamp(&mut self, pc: u16, at: Ticks, frame: u64) {
        for (i, kind) in [IgnoredWriteKind::Vram, IgnoredWriteKind::Oam]
            .into_iter()
            .enumerate()
        {
            let Some(pending) = self.step[i].take() else {
                continue;
            };
            match &mut self.found[i] {
                Some(found) => found.count += pending.count,
                empty => {
                    *empty = Some(IgnoredWrites {
                        kind,
                        count: pending.count,
                        pc,
                        addr: pending.addr,
                        line: pending.line,
                        at,
                        frame,
                    })
                }
            }
        }
    }

    /// Everything counted, VRAM first, leaving the log empty.
    pub fn take(&mut self) -> Vec<IgnoredWrites> {
        self.found.iter_mut().filter_map(Option::take).collect()
    }

    /// Forgets everything, as after a load.
    pub fn clear(&mut self) {
        *self = IgnoredLog::default();
    }
}

/// Which kind a dropped write to `addr` is, or `None` for $FEA0-$FEFF,
/// which SameBoy blocks with OAM but which holds no OAM.
fn kind_of(addr: u16) -> Option<IgnoredWriteKind> {
    match addr {
        0x8000..=0x9FFF => Some(IgnoredWriteKind::Vram),
        0xFE00..=0xFE9F => Some(IgnoredWriteKind::Oam),
        _ => None,
    }
}

/// SameBoy calls this as a blocked write returns, the display synced to the
/// write. Only a run's writes are the program's: one by gbcli itself
/// (`bus_write`) is not logged.
pub(crate) unsafe extern "C" fn on_ignored_write(gb: *mut sb::GB_gameboy_t, addr: u16) {
    guard(gb, (), |hooks| {
        if !hooks.in_run {
            return;
        }
        let (Some(log), Some(kind)) = (&mut hooks.ignored_log, kind_of(addr)) else {
            return;
        };
        // SAFETY: a side-effect-free accessor on the live instance, inside
        // its callback. A write is blocked only with the LCD on, where the
        // line is never -1.
        let line = u8::try_from(unsafe { sb::gbcli_ppu_line(gb) }).unwrap_or(0);
        log.note(kind, addr, line);
    });
}

/// Stamps the drops of the step at `pc` with the step's start and the last
/// published frame's number, as the LCD-off log does.
pub(crate) fn stamp(core: &mut SameBoy, pc: u16) {
    let at = Ticks(core.ticks);
    let hooks = core.hooks_mut();
    let frame = hooks.frame.tag.map_or(0, |tag| tag.number);
    if let Some(log) = &mut hooks.ignored_log {
        log.stamp(pc, at, frame);
    }
}
