//! Vendored SameBoy (`SameBoy/Core`), compiled by the cc crate in `build.rs`,
//! with hand-written FFI declarations for the functions gbcli uses (no bindgen).
//!
//! Declarations mirror `SameBoy/Core/gb.h` and `display.h`; add them as
//! callers need them. Everything here is raw and `unsafe`; the SameBoy
//! backend crate owns the safe wrapper.

#![allow(non_camel_case_types)]

use std::ffi::{c_char, c_uint};
use std::marker::{PhantomData, PhantomPinned};

/// The vendored SameBoy commit (`SameBoy/COMMIT`, written by `scripts/vendor-sameboy.sh`).
pub const SAMEBOY_COMMIT: &str = include_str!("../SameBoy/COMMIT").trim_ascii_end();

/// SameBoy's open-source boot ROMs (Expat; `bootroms/LICENSE`), built from
/// `SameBoy/BootROMs/*.asm` by `scripts/build-roms.sh`.
pub mod boot_roms {
    pub const DMG: &[u8] = include_bytes!("../bootroms/dmg_boot.bin");
    pub const MGB: &[u8] = include_bytes!("../bootroms/mgb_boot.bin");
    pub const SGB: &[u8] = include_bytes!("../bootroms/sgb_boot.bin");
    pub const SGB2: &[u8] = include_bytes!("../bootroms/sgb2_boot.bin");
    pub const CGB: &[u8] = include_bytes!("../bootroms/cgb_boot.bin");
    pub const CGB0: &[u8] = include_bytes!("../bootroms/cgb0_boot.bin");
    pub const CGB_FAST: &[u8] = include_bytes!("../bootroms/cgb_boot_fast.bin");
    pub const AGB: &[u8] = include_bytes!("../bootroms/agb_boot.bin");
}

/// Opaque `GB_gameboy_t`; only ever handled by pointer.
#[repr(C)]
pub struct GB_gameboy_t {
    _private: [u8; 0],
    _marker: PhantomData<(*mut u8, PhantomPinned)>,
}

/// `GB_model_t` (model.h), a C enum.
pub type GB_model_t = c_uint;
pub const GB_MODEL_DMG_B: GB_model_t = 0x002;

/// `GB_log_attributes_t` (gb.h), a C enum of bit flags.
pub type GB_log_attributes_t = c_uint;

pub type GB_log_callback_t = Option<
    unsafe extern "C" fn(
        gb: *mut GB_gameboy_t,
        string: *const c_char,
        attributes: GB_log_attributes_t,
    ),
>;

unsafe extern "C" {
    pub fn GB_alloc() -> *mut GB_gameboy_t;
    pub fn GB_init(gb: *mut GB_gameboy_t, model: GB_model_t) -> *mut GB_gameboy_t;
    pub fn GB_free(gb: *mut GB_gameboy_t);
    pub fn GB_dealloc(gb: *mut GB_gameboy_t);

    pub fn GB_load_boot_rom_from_buffer(gb: *mut GB_gameboy_t, buffer: *const u8, size: usize);
    pub fn GB_load_rom_from_buffer(gb: *mut GB_gameboy_t, buffer: *const u8, size: usize);

    /// Runs one CPU step and returns the 8 MHz ticks it took (two per T-cycle at single speed).
    pub fn GB_run(gb: *mut GB_gameboy_t) -> c_uint;

    pub fn GB_set_log_callback(gb: *mut GB_gameboy_t, callback: GB_log_callback_t);
    pub fn GB_set_rendering_disabled(gb: *mut GB_gameboy_t, disabled: bool);
    pub fn GB_set_turbo_mode(gb: *mut GB_gameboy_t, on: bool, no_frame_skip: bool);
    /// `0.0` removes the cap.
    pub fn GB_set_turbo_cap(gb: *mut GB_gameboy_t, multiplier: f64);
}
