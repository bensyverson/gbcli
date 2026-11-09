//! Writes the hardware ignored: the program's VRAM writes in mode 3 and OAM
//! writes in modes 2-3 are dropped by the PPU, silently in an emulator as on
//! a real Game Boy, so a run or step that makes any warns, once a kind
//! (leaf si5fnG). The core counts them exactly; the engine names the first.

use std::fmt;

use gbcli_core::{IgnoredWriteKind, IgnoredWrites};

use super::Since;
use super::record::{base, symbol_at};
use crate::machine::Machine;

/// The writes of one kind a run made that the PPU ignored, and the first.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct WritesIgnored {
    pub kind: IgnoredWriteKind,
    /// How many, the first included.
    pub count: u64,
    /// The PC of the instruction that made the first.
    pub pc: u16,
    /// The symbol containing `pc`, as a stop record names it.
    pub symbol: Option<String>,
    /// The address the first was aimed at.
    pub addr: u16,
    /// The line (LY) the first landed on.
    pub line: u8,
    /// The frame, counted from the run's time base as a stop record counts.
    pub frame: i64,
}

impl fmt::Display for WritesIgnored {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let (memory, modes, remedy) = match self.kind {
            IgnoredWriteKind::Vram => (
                "VRAM",
                "mode 3",
                "the PPU owns VRAM in mode 3: write it in HBlank or VBlank, or with the LCD off \
                 (gbcli help traps)",
            ),
            IgnoredWriteKind::Oam => (
                "OAM",
                "modes 2-3",
                "the PPU owns OAM in modes 2 and 3: write it in HBlank or VBlank, usually by \
                 OAM DMA in VBlank",
            ),
        };
        let plural = if self.count == 1 { "" } else { "s" };
        write!(
            f,
            "{} {memory} write{plural} ignored in {modes}, first at ${:04X}",
            self.count, self.pc
        )?;
        if let Some(symbol) = &self.symbol {
            write!(f, " ({symbol})")?;
        }
        write!(f, ", line {} of frame {}; {remedy}", self.line, self.frame)
    }
}

/// Turns the core's ignored-write log on for a drive, or off after it. A
/// core without the log runs without the warning.
pub(super) fn watch(m: &mut Machine, on: bool) {
    let _ = m.core_mut().set_ignored_write_log(on);
}

/// Takes what the core counted since the last call into `found`, VRAM first.
pub(super) fn collect(m: &mut Machine, since: &Since, found: &mut Vec<WritesIgnored>) {
    let Ok(counted) = m.core_mut().take_ignored_writes() else {
        return;
    };
    for c in &counted {
        match found.iter_mut().find(|f| f.kind == c.kind) {
            Some(first) => first.count += c.count,
            None => found.push(first_of(m, since, c)),
        }
    }
    found.sort_by_key(|f| f.kind);
}

fn first_of(m: &Machine, since: &Since, c: &IgnoredWrites) -> WritesIgnored {
    let mark = base(m, since).1;
    WritesIgnored {
        kind: c.kind,
        count: c.count,
        pc: c.pc,
        symbol: symbol_at(m, c.pc),
        addr: c.addr,
        line: c.line,
        frame: c.frame as i64 - mark.frame as i64,
    }
}
