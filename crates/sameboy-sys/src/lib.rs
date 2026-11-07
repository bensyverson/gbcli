//! Vendored SameBoy (`SameBoy/Core`), compiled by the cc crate in `build.rs`,
//! with hand-written FFI declarations for the functions gbcli uses (no bindgen).
//!
//! Declarations mirror `SameBoy/Core/gb.h` and `display.h`; add them as
//! callers need them. Everything here is raw and `unsafe`; the SameBoy
//! backend crate owns the safe wrapper.

#![allow(non_camel_case_types)]

use std::ffi::{c_char, c_int, c_uint, c_void};
use std::marker::{PhantomData, PhantomPinned};

/// The vendored SameBoy commit (`SameBoy/COMMIT`, written by `scripts/vendor-sameboy.sh`).
pub const SAMEBOY_COMMIT: &str = include_str!("../SameBoy/COMMIT").trim_ascii_end();

/// The hash of gbcli's patch set (`patches/`), computed by `build.rs`: 16 hex digits.
pub const PATCH_SET: &str = env!("GBCLI_SAMEBOY_PATCH_SET");

/// The core identity a save state records: `<commit>+patches.<PATCH_SET>`.
/// Changing any file in `patches/` changes it, so no number needs bumping by hand.
pub const CORE_VERSION: &str = env!("GBCLI_SAMEBOY_CORE_VERSION");

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
pub const GB_MODEL_SGB: GB_model_t = 0x004;
pub const GB_MODEL_MGB: GB_model_t = 0x100;
pub const GB_MODEL_SGB2: GB_model_t = 0x101;
pub const GB_MODEL_CGB_0: GB_model_t = 0x200;
pub const GB_MODEL_CGB_A: GB_model_t = 0x201;
pub const GB_MODEL_CGB_B: GB_model_t = 0x202;
pub const GB_MODEL_CGB_C: GB_model_t = 0x203;
pub const GB_MODEL_CGB_D: GB_model_t = 0x204;
pub const GB_MODEL_CGB_E: GB_model_t = 0x205;
pub const GB_MODEL_AGB_A: GB_model_t = 0x207;

/// `GB_direct_access_t` (gb.h), a C enum.
pub type GB_direct_access_t = c_uint;
pub const GB_DIRECT_ACCESS_ROM: GB_direct_access_t = 0;
pub const GB_DIRECT_ACCESS_RAM: GB_direct_access_t = 1;
pub const GB_DIRECT_ACCESS_CART_RAM: GB_direct_access_t = 2;
pub const GB_DIRECT_ACCESS_VRAM: GB_direct_access_t = 3;
pub const GB_DIRECT_ACCESS_HRAM: GB_direct_access_t = 4;
pub const GB_DIRECT_ACCESS_IO: GB_direct_access_t = 5;
pub const GB_DIRECT_ACCESS_BOOTROM: GB_direct_access_t = 6;
pub const GB_DIRECT_ACCESS_OAM: GB_direct_access_t = 7;
pub const GB_DIRECT_ACCESS_BGP: GB_direct_access_t = 8;
pub const GB_DIRECT_ACCESS_OBP: GB_direct_access_t = 9;
pub const GB_DIRECT_ACCESS_IE: GB_direct_access_t = 10;
/// The ROM, with the bank mapped at 0000-3FFF in the bank output.
pub const GB_DIRECT_ACCESS_ROM0: GB_direct_access_t = 11;

/// `GB_vblank_type_t` (display.h), a C enum.
pub type GB_vblank_type_t = c_uint;
pub const GB_VBLANK_TYPE_NORMAL_FRAME: GB_vblank_type_t = 0;
pub const GB_VBLANK_TYPE_LCD_OFF: GB_vblank_type_t = 1;
pub const GB_VBLANK_TYPE_ARTIFICIAL: GB_vblank_type_t = 2;
pub const GB_VBLANK_TYPE_REPEAT: GB_vblank_type_t = 3;
pub const GB_VBLANK_TYPE_SKIPPED_FRAME: GB_vblank_type_t = 4;

/// `GB_channel_t` (apu.h), a C enum.
pub type GB_channel_t = c_uint;
pub const GB_SQUARE_1: GB_channel_t = 0;
pub const GB_SQUARE_2: GB_channel_t = 1;
pub const GB_WAVE: GB_channel_t = 2;
pub const GB_NOISE: GB_channel_t = 3;

/// `GB_rtc_mode_t` (timing.h), a C enum.
pub type GB_rtc_mode_t = c_uint;
pub const GB_RTC_MODE_ACCURATE: GB_rtc_mode_t = 1;

/// `GB_key_mask_t` (joypad.h): one bit per key, in the same order as `gbcli_core::Keys`.
pub type GB_key_mask_t = c_uint;

/// `GB_registers_t` (gb.h): AF, BC, DE, HL, SP, PC as 16-bit words (little-endian host).
#[repr(C)]
#[derive(Clone, Copy, Debug, Default)]
pub struct GB_registers_t {
    pub af: u16,
    pub bc: u16,
    pub de: u16,
    pub hl: u16,
    pub sp: u16,
    pub pc: u16,
}

/// `GB_palette_t` (display.h): colors 0–3 are shades darkest to lightest, 4 is the LCD-off color.
#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub struct GB_palette_t {
    pub colors: [[u8; 3]; 5],
}

/// `GB_log_attributes_t` (gb.h), a C enum of bit flags.
pub type GB_log_attributes_t = c_uint;

pub type GB_log_callback_t = Option<
    unsafe extern "C" fn(
        gb: *mut GB_gameboy_t,
        string: *const c_char,
        attributes: GB_log_attributes_t,
    ),
>;

pub type GB_input_callback_t = Option<unsafe extern "C" fn(gb: *mut GB_gameboy_t) -> *mut c_char>;
pub type GB_vblank_callback_t =
    Option<unsafe extern "C" fn(gb: *mut GB_gameboy_t, kind: GB_vblank_type_t)>;
pub type GB_rgb_encode_callback_t =
    Option<unsafe extern "C" fn(gb: *mut GB_gameboy_t, r: u8, g: u8, b: u8) -> u32>;
pub type GB_read_memory_callback_t =
    Option<unsafe extern "C" fn(gb: *mut GB_gameboy_t, addr: u16, data: u8) -> u8>;
/// Returns false to prevent the write.
pub type GB_write_memory_callback_t =
    Option<unsafe extern "C" fn(gb: *mut GB_gameboy_t, addr: u16, data: u8) -> bool>;
pub type GB_execution_callback_t =
    Option<unsafe extern "C" fn(gb: *mut GB_gameboy_t, addr: u16, opcode: u8)>;

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

    /// Process-global: seed before each `GB_init`, under a lock.
    pub fn GB_random_seed(seed: u64);
    pub fn GB_get_model(gb: *mut GB_gameboy_t) -> GB_model_t;
    pub fn GB_is_cgb(gb: *const GB_gameboy_t) -> bool;
    pub fn GB_set_user_data(gb: *mut GB_gameboy_t, data: *mut c_void);
    pub fn GB_get_user_data(gb: *mut GB_gameboy_t) -> *mut c_void;
    /// A no-op when the debugger is compiled out, as it is here.
    pub fn GB_set_input_callback(gb: *mut GB_gameboy_t, callback: GB_input_callback_t);
    /// A no-op when the debugger is compiled out, as it is here.
    pub fn GB_set_async_input_callback(gb: *mut GB_gameboy_t, callback: GB_input_callback_t);
    pub fn GB_set_rtc_mode(gb: *mut GB_gameboy_t, mode: GB_rtc_mode_t);
    pub fn GB_set_emulate_joypad_bouncing(gb: *mut GB_gameboy_t, emulate: bool);
    pub fn GB_set_key_mask(gb: *mut GB_gameboy_t, mask: GB_key_mask_t);

    pub fn GB_set_vblank_callback(gb: *mut GB_gameboy_t, callback: GB_vblank_callback_t);
    pub fn GB_set_rgb_encode_callback(gb: *mut GB_gameboy_t, callback: GB_rgb_encode_callback_t);
    /// SameBoy keeps the pointer: the palette must outlive the instance.
    pub fn GB_set_palette(gb: *mut GB_gameboy_t, palette: *const GB_palette_t);
    /// SameBoy keeps the pointer: the buffer must outlive the instance.
    pub fn GB_set_pixels_output(gb: *mut GB_gameboy_t, output: *mut u32);
    pub fn GB_get_screen_width(gb: *mut GB_gameboy_t) -> c_uint;
    pub fn GB_get_screen_height(gb: *mut GB_gameboy_t) -> c_uint;

    pub fn GB_set_read_memory_callback(gb: *mut GB_gameboy_t, callback: GB_read_memory_callback_t);
    pub fn GB_set_write_memory_callback(
        gb: *mut GB_gameboy_t,
        callback: GB_write_memory_callback_t,
    );
    pub fn GB_set_execution_callback(gb: *mut GB_gameboy_t, callback: GB_execution_callback_t);

    pub fn GB_get_direct_access(
        gb: *mut GB_gameboy_t,
        access: GB_direct_access_t,
        size: *mut usize,
        bank: *mut u16,
    ) -> *mut c_void;
    pub fn GB_get_registers(gb: *mut GB_gameboy_t) -> *mut GB_registers_t;
    /// A full bus read: open bus, the OAM bug, callbacks.
    pub fn GB_read_memory(gb: *mut GB_gameboy_t, addr: u16) -> u8;
    /// A full bus write with IO side effects.
    pub fn GB_write_memory(gb: *mut GB_gameboy_t, addr: u16, value: u8);

    pub fn GB_get_save_state_size(gb: *mut GB_gameboy_t) -> usize;
    pub fn GB_save_state_to_buffer(gb: *mut GB_gameboy_t, buffer: *mut u8);
    /// Returns 0 on success.
    pub fn GB_load_state_from_buffer(
        gb: *mut GB_gameboy_t,
        buffer: *const u8,
        length: usize,
    ) -> c_int;

    // APU getters (apu.h): plain reads of APU state, which lags while SameBoy
    // batches APU cycles (`gbcli_apu_synced`). `GB_get_channel_edge_triggered`
    // is left out: it clears the flag it reads.
    pub fn GB_get_channel_volume(gb: *mut GB_gameboy_t, channel: GB_channel_t) -> u8;
    pub fn GB_get_channel_amplitude(gb: *mut GB_gameboy_t, channel: GB_channel_t) -> u8;
    pub fn GB_get_channel_period(gb: *mut GB_gameboy_t, channel: GB_channel_t) -> u16;
    /// Fills 32 bytes: wave RAM as nibbles, high nibble first.
    pub fn GB_get_apu_wave_table(gb: *mut GB_gameboy_t, wave_table: *mut u8);
}

// gbcli's own accessors over SameBoy internals (`shim/gbcli_shim.c`). The
// peeks are side-effect free and return -1 where a value needs a bus read.
unsafe extern "C" {
    pub fn gbcli_ime(gb: *mut GB_gameboy_t) -> bool;
    pub fn gbcli_set_ime(gb: *mut GB_gameboy_t, ime: bool);
    pub fn gbcli_halted(gb: *mut GB_gameboy_t) -> bool;
    pub fn gbcli_set_halted(gb: *mut GB_gameboy_t, halted: bool);
    pub fn gbcli_stopped(gb: *mut GB_gameboy_t) -> bool;
    pub fn gbcli_double_speed(gb: *mut GB_gameboy_t) -> bool;
    pub fn gbcli_is_mmm01(gb: *mut GB_gameboy_t) -> bool;
    pub fn gbcli_clear_rtc_epoch(gb: *mut GB_gameboy_t);
    pub fn gbcli_ppu_batched(gb: *mut GB_gameboy_t) -> bool;
    /// Attaches (or with null, detaches) the per-pixel layer buffer; SameBoy
    /// keeps the pointer, so it must outlive the instance.
    pub fn gbcli_set_layer_buffer(gb: *mut GB_gameboy_t, buffer: *mut u8);
    /// Whether the frame whose vblank is now running was replaced with the
    /// blank LCD-off screen after the LCD was turned on.
    pub fn gbcli_frame_scrubbed(gb: *mut GB_gameboy_t) -> bool;
    pub fn gbcli_bank_for(gb: *mut GB_gameboy_t, addr: u16) -> u16;
    pub fn gbcli_cart_ram_mirrored(gb: *mut GB_gameboy_t) -> bool;
    pub fn gbcli_cart_ram_offset(gb: *mut GB_gameboy_t, addr: u16) -> i32;
    /// The address a CPU write at `addr` lands at, from inside the write
    /// callback (before OAM DMA's bus-conflict redirect), or -1 when the
    /// write reaches no memory but OAM.
    pub fn gbcli_write_target(gb: *mut GB_gameboy_t, addr: u16) -> i32;
    pub fn gbcli_apu_synced(gb: *mut GB_gameboy_t) -> bool;
    pub fn gbcli_io_peek(gb: *mut GB_gameboy_t, reg: u8) -> c_int;
    pub fn gbcli_peek(gb: *mut GB_gameboy_t, addr: u16) -> c_int;
    pub fn gbcli_next_opcode(gb: *mut GB_gameboy_t) -> c_int;
    /// One of the `GBCLI_PRE_*` codes; fills `opcode` for `GBCLI_PRE_ILLEGAL`.
    pub fn gbcli_pre_run(gb: *mut GB_gameboy_t, opcode: *mut u8) -> c_int;
}

/// A per-pixel layer tag (`GBCLI_LAYER_*`, the patched `display.h`): what
/// SameBoy writes to the buffer `gbcli_set_layer_buffer` attaches.
pub const GBCLI_LAYER_BACKGROUND: u8 = 0;
pub const GBCLI_LAYER_WINDOW: u8 = 1;
/// An object's tag is this plus its OAM index (0-39).
pub const GBCLI_LAYER_OBJECT: u8 = 2;

unsafe extern "C" {
    /// The C values of the three `GBCLI_LAYER_*` names, in that order
    /// (`shim/gbcli_layers.c`), so a test can check the constants above.
    pub static gbcli_layer_tags: [u8; 3];
}

/// `gbcli_pre_run`: run the next step.
pub const GBCLI_PRE_RUN: c_int = 0;
/// `gbcli_pre_run`: the next step would execute an illegal opcode.
pub const GBCLI_PRE_ILLEGAL: c_int = 1;
/// `gbcli_pre_run`: in STOP mode with nothing to wake it.
pub const GBCLI_PRE_STOPPED: c_int = 2;
