//! The live framebuffer, the color mode and SameBoy's own CGB color conversion.

use gbcli_core::{ColorMode, CoreError, LiveFrame, PixelBuf};
use sameboy_sys as sb;

use crate::SameBoy;
use crate::hooks::{SCREEN_PIXELS, shade};

pub(crate) fn live_frame(core: &SameBoy) -> LiveFrame {
    // SAFETY: plain getters on a live instance.
    let (width, height) = unsafe {
        (
            sb::GB_get_screen_width(core.raw()),
            sb::GB_get_screen_height(core.raw()),
        )
    };
    let n = ((width * height) as usize).min(SCREEN_PIXELS);
    let hooks = core.hooks();
    let screen = &hooks.screen[..n];
    let pixels = if hooks.dmg_shades {
        PixelBuf::Shades(screen.iter().map(|&p| shade(p)).collect())
    } else {
        PixelBuf::Rgb(screen.to_vec())
    };
    LiveFrame {
        width,
        height,
        pixels,
    }
}

pub(crate) fn color_mode(core: &SameBoy) -> ColorMode {
    let raw = core.raw();
    // SAFETY: plain getters on a live instance.
    unsafe {
        if sb::GB_is_sgb(raw) {
            ColorMode::Sgb
        } else if !sb::GB_is_cgb(raw) {
            ColorMode::DmgShades
        } else if sb::GB_is_cgb_in_cgb_mode(raw) {
            ColorMode::CgbPalettes
        } else {
            ColorMode::CgbDmgCompat
        }
    }
}

pub(crate) fn rgb555_color(core: &SameBoy, color: u16) -> Result<u32, CoreError> {
    if core.hooks().dmg_shades {
        return Err(CoreError::Unsupported {
            feature: gbcli_core::Feature::DisplayColors,
            reason: "DMG and MGB frames hold shades, not colors",
        });
    }
    // SAFETY: GB_convert_rgb15 only reads the instance (its color correction
    // mode and model) and calls our encode callback, which touches nothing.
    Ok(unsafe { sb::GB_convert_rgb15(core.raw(), color & 0x7FFF, false) })
}
