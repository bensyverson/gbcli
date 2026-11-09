//! State SameBoy's callbacks reach through `GB_set_user_data`, and the callbacks.
//!
//! Callbacks run inside `GB_run`, mid-instruction, so they touch only this
//! struct, never the `SameBoy` wrapper, and the wrapper never holds a borrow of
//! it across a `GB_run`. Every body runs under [`guard`]: a panic is caught and
//! recorded, never unwound across the C boundary.

use std::ffi::{CStr, c_char};
use std::panic::{AssertUnwindSafe, catch_unwind};

use gbcli_core::{ApuWrite, AudioSamples, FrameTag, Hits, Layer, Pixels, VblankKind};
use sameboy_sys as sb;

use crate::watch::{Armed, Tracking};

/// Lines of SameBoy log kept for diagnostics; older lines are dropped.
const LOG_LINES: usize = 256;
/// The largest screen SameBoy draws (SGB with border).
pub(crate) const SCREEN_PIXELS: usize = 256 * 224;

/// Where a step that began in HALT left it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Wake {
    /// Not yet: no CPU bus access so far this step.
    Halted,
    /// At the CPU's first bus access, this many ticks into the step: the
    /// dispatch's or the instruction's first read, so the ticks before it
    /// are the HALT cycles SameBoy ran first in the same step.
    At(u32),
}

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
    /// Which layer drew each pixel, when layer capture was on; empty otherwise.
    pub layers: Vec<Layer>,
}

impl FrameCopy {
    /// A copy for the state blob, which never encodes the layer map.
    pub fn without_layers(&self) -> FrameCopy {
        FrameCopy {
            tag: self.tag,
            width: self.width,
            height: self.height,
            shades: self.shades.clone(),
            rgb: self.rgb.clone(),
            layers: Vec::new(),
        }
    }

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
    /// SameBoy writes one tag per drawn pixel here while capture is on
    /// (`gbcli_set_layer_buffer`); the tags of the frame being rendered.
    pub layers: Box<[u8]>,
    /// Whether `layers` is attached to SameBoy.
    pub layers_on: bool,
    /// Whether the next vblank ends a frame that was only partly captured on
    /// this timeline (capture turned on, or a state loaded, mid-frame), so its
    /// tags must not be published.
    pub layers_skip_next: bool,
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
    /// The PC of the instruction now executing (set while filters are armed
    /// or the write log is on).
    pub step_pc: u16,
    /// The filter hits of the current step, in access order.
    pub hit: Option<Hits>,
    pub tracking: Option<Tracking>,
    /// The sound-register write log, while it is on.
    pub apu_log: Option<Vec<ApuWrite>>,
    /// The captured samples, while capture is on.
    pub audio: Option<AudioSamples>,
    /// `now` at the start of the current step, kept while the log or capture
    /// is on (and set before a bus write), to stamp what the callbacks see.
    pub step_ticks: u64,
    /// Whether a run is in progress, so SameBoy's cycle count is this step's.
    pub in_run: bool,
    /// The opcode SameBoy's execution callback saw this step, while a run's
    /// sink wants execution events; `None` when the step ran no instruction.
    pub executed: Option<u8>,
    /// While a step that began in HALT runs under an execution sink: how
    /// far into it the CPU left HALT.
    pub wake: Option<Wake>,
    /// The raster events of the current step, while a run's sink wants them.
    pub raster: Option<Vec<gbcli_core::Event>>,
    /// Set as the LCD turns off, so the line callback SameBoy then fires is
    /// not taken for a line start.
    pub lcd_turning_off: bool,
    /// The LCD-off log, while it is on.
    pub lcd_off_log: Option<Vec<gbcli_core::LcdOff>>,
    /// The LCD toggles of the current step, while the log is on; the run loop
    /// logs an LCD-off from them with the step's PC and time.
    pub lcd_toggles: Option<LcdToggles>,
    /// The ignored-write log, while it is on.
    pub ignored_log: Option<crate::ignored::IgnoredLog>,
    /// The cart's flash chip, on a flash-save cart; it keeps both memory
    /// callbacks installed for the core's life.
    pub flash: Option<gbcli_core::flash::FlashChip>,
    /// On a mask-ROM cart, the watch for a program's flash commands; it
    /// keeps the write callback installed for the core's life.
    pub unlocks: Option<gbcli_core::flash::UnlockWatch>,
}

/// The LCD's on/off toggles during one CPU step. One program write to LCDC
/// can toggle the LCD several times: on SGB, SameBoy applies it as the new
/// value, the old one and the new one again (`GB_CONFLICT_SGB_LCDC` in
/// sm83_cpu.c), so an LCD-off fires off, on, off and a turn-on fires on,
/// off, on. The step's one logical transition is the first toggle's
/// direction, and only if the last toggle agrees with it.
#[derive(Clone, Copy, Debug)]
pub struct LcdToggles {
    /// The direction of the step's first toggle (`true` turned the LCD on).
    pub first_on: bool,
    /// The display's line at the first toggle, read when it turned the LCD
    /// off: after that, `GB_lcd_off` has reset the line counter to 0.
    pub line: Option<u8>,
    /// The direction of the step's last toggle.
    pub last_on: bool,
}

impl LcdToggles {
    /// The line the step turned the LCD off on, if its net effect turned
    /// the LCD off.
    pub fn off_line(self) -> Option<u8> {
        (!self.first_on && !self.last_on)
            .then_some(self.line)
            .flatten()
    }
}

impl Hooks {
    pub fn new(dmg_shades: bool, rendering: bool) -> Self {
        Hooks {
            screen: vec![0; SCREEN_PIXELS].into_boxed_slice(),
            layers: vec![sb::GBCLI_LAYER_BACKGROUND; SCREEN_PIXELS].into_boxed_slice(),
            layers_on: false,
            layers_skip_next: false,
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
            apu_log: None,
            audio: None,
            step_ticks: 0,
            in_run: false,
            executed: None,
            wake: None,
            raster: None,
            lcd_turning_off: false,
            lcd_off_log: None,
            lcd_toggles: None,
            ignored_log: None,
            flash: None,
            unlocks: None,
        }
    }

    /// Whether something outside a run needs SameBoy's write callback.
    pub fn write_hook_wanted(&self) -> bool {
        self.tracking.is_some()
            || self.apu_log.is_some()
            || self.flash.is_some()
            || self.unlocks.is_some()
    }

    /// Whether something outside a run needs SameBoy's read callback.
    pub fn read_hook_wanted(&self) -> bool {
        self.flash.is_some()
    }

    /// Starts collecting tags from a clean buffer. Unless the frame in
    /// progress is `fresh` (no pixel of it drawn yet), it is only partly
    /// captured on this timeline, so its map is dropped at the next vblank.
    pub fn restart_capture(&mut self, fresh: bool) {
        self.layers.fill(sb::GBCLI_LAYER_BACKGROUND);
        self.layers_skip_next = !fresh;
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
    // SAFETY: non-null, and the only live reference to the hooks (see above).
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

/// Records the opcode of the instruction the step runs. SameBoy calls this
/// for every instruction and never for an interrupt dispatch or a HALT tick,
/// which is how a step is told apart from those.
pub(crate) unsafe extern "C" fn on_exec(gb: *mut sb::GB_gameboy_t, _addr: u16, opcode: u8) {
    guard(gb, (), |hooks| hooks.executed = Some(opcode));
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
        let n = ((width * height) as usize).min(SCREEN_PIXELS);
        let publish = hooks.rendering && vblank_kind(kind).is_some();
        if publish {
            let live = &hooks.screen[..n];
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
        }
        if hooks.layers_on {
            if publish {
                // Not every published frame was rendered: an LCD-off or
                // artificial frame, and the blank frame SameBoy substitutes at
                // the first vblank after the LCD is turned on, get no layer
                // map. Nor does a frame only partly captured on this timeline.
                let rendered = vblank_kind(kind) == Some(VblankKind::Normal)
                    && !hooks.layers_skip_next
                    // SAFETY: a plain read of the live instance, inside its callback.
                    && !unsafe { sb::gbcli_frame_scrubbed(gb) };
                hooks.frame.layers.clear();
                if rendered {
                    let tags = &hooks.layers[..n];
                    hooks.frame.layers.extend(tags.iter().map(|&t| layer(t)));
                }
            }
            // Every vblank, published or not (a CGB repeat keeps the previous
            // frame on screen), ends the frame the buffer was collecting: the
            // next one is captured whole, and starts clean, so whatever the
            // renderer doesn't tag (an LCD-off frame, a border) is background.
            hooks.layers_skip_next = false;
            hooks.layers.fill(sb::GBCLI_LAYER_BACKGROUND);
        }
        hooks.frame_copied = publish;
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
pub(crate) fn shade(pixel: u32) -> u8 {
    match pixel & 0xFF {
        index @ 0..=3 => 3 - index as u8,
        _ => 0,
    }
}

/// SameBoy's side-buffer tag (`GBCLI_LAYER_*`, the patched `display.h`) back
/// to a [`Layer`]: the one place the encoding is decoded.
fn layer(tag: u8) -> Layer {
    match tag {
        sb::GBCLI_LAYER_BACKGROUND => Layer::Background,
        sb::GBCLI_LAYER_WINDOW => Layer::Window,
        n => Layer::Object {
            index: n - sb::GBCLI_LAYER_OBJECT,
        },
    }
}
