//! The LCD turned off outside VBlank: harmless in an emulator, but it can
//! damage a real DMG's screen (Pan Docs, LCDC bit 7), so a run or step that
//! does it warns (design §11, leaf jXavVc). The core logs every LCD-off with
//! the line it landed on; the engine decides which are outside VBlank.

use std::fmt;

use gbcli_core::LcdOff;

use super::Since;
use super::record::{base, symbol_at};
use crate::machine::Machine;

/// The first line of VBlank; lines 144-153 are VBlank.
pub const VBLANK_FIRST_LINE: u8 = 144;

/// The first time a run turned the LCD off outside VBlank, and how many
/// more times it did.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LcdOffOutsideVblank {
    /// The PC of the instruction that wrote LCDC.
    pub pc: u16,
    /// The symbol containing `pc`, as a stop record names it.
    pub symbol: Option<String>,
    /// The line (LY) the write landed on.
    pub line: u8,
    /// The frame, counted from the run's time base as a stop record counts.
    pub frame: i64,
    /// How many more times the same run did it.
    pub more: u64,
}

impl fmt::Display for LcdOffOutsideVblank {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "the LCD was turned off outside VBlank at ${:04X}",
            self.pc
        )?;
        if let Some(symbol) = &self.symbol {
            write!(f, " ({symbol})")?;
        }
        write!(f, ", line {} of frame {}", self.line, self.frame)?;
        if self.more > 0 {
            write!(f, ", and {} more times", self.more)?;
        }
        f.write_str(
            "; on a real DMG this can damage the screen: turn the LCD off only in VBlank: \
             wait for LY >= 144 or use the VBlank interrupt",
        )
    }
}

/// Turns the core's LCD-off log on for a drive, or off after it. A core
/// without the log runs without the warning.
pub(super) fn watch(m: &mut Machine, on: bool) {
    let _ = m.core_mut().set_lcd_off_log(on);
}

/// Takes what the core logged since the last call into `found`.
pub(super) fn collect(m: &mut Machine, since: &Since, found: &mut Option<LcdOffOutsideVblank>) {
    let Ok(offs) = m.core_mut().take_lcd_offs() else {
        return;
    };
    for off in offs.iter().filter(|o| o.line < VBLANK_FIRST_LINE) {
        match found {
            Some(first) => first.more += 1,
            None => *found = Some(first_of(m, since, off)),
        }
    }
}

fn first_of(m: &Machine, since: &Since, off: &LcdOff) -> LcdOffOutsideVblank {
    let mark = base(m, since).1;
    LcdOffOutsideVblank {
        pc: off.pc,
        symbol: symbol_at(m, off.pc),
        line: off.line,
        frame: off.vblanks as i64 - mark.frame as i64,
        more: 0,
    }
}
