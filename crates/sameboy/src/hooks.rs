//! State SameBoy's callbacks reach through `GB_set_user_data`, and the callbacks.
//!
//! Callbacks run inside `GB_run`, mid-instruction, so they touch only this
//! struct, never the `SameBoy` wrapper, and the wrapper never holds a borrow of
//! it across a `GB_run`. Every body runs under [`guard`]: a panic is caught and
//! recorded, never unwound across the C boundary.

use std::ffi::{CStr, c_char};
use std::panic::{AssertUnwindSafe, catch_unwind};

use gbcli_core::{FrameTag, Hit, Pixels, VblankKind};
use sameboy_sys as sb;

use crate::watch::{Armed, Tracking};

/// Lines of SameBoy log kept for diagnostics; older lines are dropped.
const LOG_LINES: usize = 256;
/// The largest screen SameBoy draws (SGB with border).
pub(crate) const SCREEN_PIXELS: usize = 256 * 224;

/// The DMG palette whose "colors" are shade indices, so the encoder hands back
/// raw shades: SameBoy maps shade `s` (0 lightest) to `colors[3 - s]`, and
/// `colors[4]` is the LCD-off color.
pub(crate) static SENTINEL_PALETTE: sb::GB_palette_t = sb::GB_palette_t {
    colors: [[0, 0, 0], [0, 0, 1], [0, 0, 2], [0, 0, 3], [0, 0, 4]],
};

/// The last complete frame, kept apart from SameBoy's live buffer.
#[derive(Clone, Debug, Default)]
pub(crate) struct FrameCopy {
    pub tag: Option<FrameTag>,
    pub width: u32,
    pub height: u32,
    pub shades: Vec<u8>,
    pub rgb: Vec<u32>,
}

impl FrameCopy {
    pub fn pixels(&self) -> Pixels<'_> {
        if self.rgb.is_empty() {
            Pixels::Shades(&self.shades)
        } else {
            Pixels::Rgb(&self.rgb)
        }
    }
}

pub(crate) struct Hooks {
    /// SameBoy draws here (`GB_set_pixels_output`); live and partial.
    pub screen: Box<[u32]>,
    pub dmg_shades: bool,
    pub rendering: bool,
    pub frame: FrameCopy,
    /// The vblank seen during the current `GB_run`, if any.
    pub vblank: Option<sb::GB_vblank_type_t>,
    /// Whether that vblank's pixels were copied into `frame` (tag pending).
    pub frame_copied: bool,
    pub log: Vec<String>,
    /// Set when a callback panicked; the run turns it into an error.
    pub panicked: bool,
    /// The filters of the run in progress, if any.
    pub armed: Option<Armed>,
    /// The PC of the instruction now executing (set while filters are armed).
    pub step_pc: u16,
    /// The first filter hit of the current step.
    pub hit: Option<Hit>,
    pub tracking: Option<Tracking>,
}

impl Hooks {
    pub fn new(dmg_shades: bool, rendering: bool) -> Self {
        Hooks {
            screen: vec![0; SCREEN_PIXELS].into_boxed_slice(),
            dmg_shades,
            rendering,
            frame: FrameCopy::default(),
            vblank: None,
            frame_copied: false,
            log: Vec::new(),
            panicked: false,
            armed: None,
            step_pc: 0,
            hit: None,
            tracking: None,
        }
    }
}

/// Runs `body` on the instance's hooks, catching any panic. Returns `fallback`
/// if the hooks are missing or the body panicked.
pub(crate) fn guard<T>(
    gb: *mut sb::GB_gameboy_t,
    fallback: T,
    body: impl FnOnce(&mut Hooks) -> T,
) -> T {
    // SAFETY: user data is the `Hooks` box the wrapper installed and keeps alive
    // until after GB_free; callbacks run on the thread inside GB_run, while the
    // wrapper holds no reference to it.
    let hooks = unsafe { sb::GB_get_user_data(gb) as *mut Hooks };
    if hooks.is_null() {
        return fallback;
    }
    let hooks = unsafe { &mut *hooks };
    match catch_unwind(AssertUnwindSafe(|| body(hooks))) {
        Ok(value) => value,
        Err(_) => {
            hooks.panicked = true;
            fallback
        }
    }
}

pub(crate) unsafe extern "C" fn on_log(
    gb: *mut sb::GB_gameboy_t,
    string: *const c_char,
    _attributes: sb::GB_log_attributes_t,
) {
    guard(gb, (), |hooks| {
        if string.is_null() {
            return;
        }
        // SAFETY: SameBoy passes a NUL-terminated string valid for the call.
        let line = unsafe { CStr::from_ptr(string) }.to_string_lossy();
        if hooks.log.len() == LOG_LINES {
            hooks.log.remove(0);
        }
        hooks.log.push(line.into_owned());
    });
}

/// SameBoy's input callbacks read stdin by default; ours never does.
pub(crate) unsafe extern "C" fn no_input(_gb: *mut sb::GB_gameboy_t) -> *mut c_char {
    std::ptr::null_mut()
}

pub(crate) unsafe extern "C" fn encode_rgb(_gb: *mut sb::GB_gameboy_t, r: u8, g: u8, b: u8) -> u32 {
    (u32::from(r) << 16) | (u32::from(g) << 8) | u32::from(b)
}

pub(crate) unsafe extern "C" fn on_vblank(gb: *mut sb::GB_gameboy_t, kind: sb::GB_vblank_type_t) {
    // SAFETY: plain getters on the live instance.
    let (width, height) = unsafe { (sb::GB_get_screen_width(gb), sb::GB_get_screen_height(gb)) };
    guard(gb, (), |hooks| {
        hooks.vblank = Some(kind);
        hooks.frame_copied = false;
        if !hooks.rendering || vblank_kind(kind).is_none() {
            return;
        }
        let n = (width * height) as usize;
        let live = &hooks.screen[..n.min(SCREEN_PIXELS)];
        let frame = &mut hooks.frame;
        frame.width = width;
        frame.height = height;
        if hooks.dmg_shades {
            frame.rgb.clear();
            frame.shades.clear();
            frame.shades.extend(live.iter().map(|&p| shade(p)));
        } else {
            frame.shades.clear();
            frame.rgb.clear();
            frame.rgb.extend_from_slice(live);
        }
        hooks.frame_copied = true;
    });
}

/// The frame kinds worth keeping: a repeat keeps the previous frame on screen,
/// and skipped frames aren't drawn.
pub(crate) fn vblank_kind(kind: sb::GB_vblank_type_t) -> Option<VblankKind> {
    match kind {
        sb::GB_VBLANK_TYPE_NORMAL_FRAME => Some(VblankKind::Normal),
        sb::GB_VBLANK_TYPE_LCD_OFF => Some(VblankKind::LcdOff),
        sb::GB_VBLANK_TYPE_ARTIFICIAL => Some(VblankKind::Artificial),
        _ => None,
    }
}

/// A sentinel-palette pixel back to its DMG shade; the LCD-off color reads as
/// shade 0 (the frame's tag says the LCD was off).
fn shade(pixel: u32) -> u8 {
    match pixel & 0xFF {
        index @ 0..=3 => 3 - index as u8,
        _ => 0,
    }
}
