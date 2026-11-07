//! Raster events: line starts from SameBoy's LCD line callback and IO-register
//! writes from its write callback, buffered in the hooks during a step for
//! the run loop to pass to the sink in the order they happened.
//!
//! **Times.** A write is stamped as the APU log is: the step's start plus how
//! far SameBoy got into it, which is exact, since SameBoy runs every pending
//! cycle before a write. A line start is stamped where the display state
//! machine stood when it fired: SameBoy hands the display a chunk of cycles
//! and then runs it through them, so that is the chunk's end minus what the
//! display has not run yet (`gbcli_display_unrun`). Where SameBoy splits a
//! chunk (a scheduled WY check, a line overrun) the stamp can be late by the
//! rest of the chunk. A write's line is the display's line counter, except
//! on the tick a line ends, where SameBoy has not yet moved to the next line
//! (`gbcli_ppu_line` counts it as the next one); the line callback then
//! comes after that write, with the same time.
//!
//! **Gaps.** SameBoy fires no line callback for line 153 or for the first
//! line after the LCD is turned on, so those lines have no start; a write on
//! them still carries its line, read from the display's own line counter. It
//! fires one with line 0 as the LCD turns off, which is not a line start and
//! is dropped. No dot (design §2.2) and no mode events: SameBoy has no mode
//! callback, and STAT's mode bits lag while mode-3 cycles are batched.

use gbcli_core::{Event, Ticks};
use sameboy_sys as sb;

use crate::audio::tap_time;
use crate::hooks::{Hooks, guard};

/// The IO registers a raster event covers (FF00-FF7F), not HRAM or IE.
fn is_io(addr: u16) -> bool {
    (0xFF00..0xFF80).contains(&addr)
}

/// Records an IO write, if a raster sink is listening. Called from the write
/// callback with the address the write lands at, once it is known to land.
#[inline(always)]
pub(crate) fn log_write(gb: *mut sb::GB_gameboy_t, hooks: &mut Hooks, addr: u16, value: u8) {
    if hooks.raster.is_none() || !is_io(addr) {
        return;
    }
    let reg = addr as u8;
    if let Some(events) = &mut hooks.raster {
        // A CPU step writes a register at most once, so a second write to it
        // is SameBoy applying the same write in steps (`cycle_write`'s
        // conflict cases): keep the first one's time and the value it
        // settles on.
        let earlier = events.iter_mut().rev().find_map(|e| match e {
            Event::IoWrite { reg: r, value, .. } if *r == reg => Some(value),
            _ => None,
        });
        if let Some(earlier) = earlier {
            *earlier = value;
            return;
        }
    }
    // SAFETY: a side-effect-free accessor on the live instance.
    let line = u8::try_from(unsafe { sb::gbcli_ppu_line(gb) }).ok();
    let event = Event::IoWrite {
        reg,
        value,
        at: tap_time(gb, hooks),
        pc: hooks.step_pc,
        line,
        dot: None,
    };
    if let Some(events) = &mut hooks.raster {
        events.push(event);
    }
}

pub(crate) unsafe extern "C" fn on_line(gb: *mut sb::GB_gameboy_t, line: u8) {
    guard(gb, (), |hooks| {
        if std::mem::take(&mut hooks.lcd_turning_off) || hooks.raster.is_none() {
            return;
        }
        // SAFETY: a plain read of the live instance, inside its callback.
        let unrun = u64::from(unsafe { sb::gbcli_display_unrun(gb) });
        let at = Ticks(tap_time(gb, hooks).0.saturating_sub(unrun));
        if let Some(events) = &mut hooks.raster {
            events.push(Event::Scanline { ly: line, at });
        }
    });
}

/// SameBoy calls this from the LCDC write just before `GB_lcd_off` fires the
/// line callback with line 0, so the flag marks that call as no line start.
/// LCDC still has the LCD on at that point, so the display's line counter is
/// the line the write landed on, which the LCD-off log keeps. Installed for
/// the instance's lifetime: it fires only when the LCD turns on or off.
pub(crate) unsafe extern "C" fn on_lcd_status(gb: *mut sb::GB_gameboy_t, on: bool) {
    guard(gb, (), |hooks| {
        hooks.lcd_turning_off = !on;
        if !on && hooks.in_run && hooks.lcd_off_log.is_some() {
            // SAFETY: a side-effect-free accessor on the live instance.
            hooks.lcd_off_line = u8::try_from(unsafe { sb::gbcli_ppu_line(gb) }).ok();
        }
    });
}
