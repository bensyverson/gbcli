//! Smoke test: the vendored SameBoy core links, boots a ROM headless, and
//! writes nothing to stdout once its log callback is routed.
//!
//! This file must stay the only test in its binary: it redirects the process's
//! file descriptor 1, which would swallow output from tests running beside it.

use std::ffi::{CStr, c_char, c_int, c_void};
use std::fs::File;
use std::io::Read;
use std::os::fd::AsRawFd;
use std::sync::Mutex;

use sameboy_sys as sb;

/// Our own test ROM, assembled from `roms/smoke.asm` by `scripts/build-roms.sh`.
const SMOKE_ROM: &[u8] = include_bytes!("roms/smoke.gb");
/// The line SameBoy logs when `smoke.asm` writes $42 to echo RAM.
const EXPECTED_LOG: &str = "Wrote 42 to f000 (RAM Mirror)\n";
/// Frames the cartridge runs after the boot ROM hands over.
const FRAMES: u64 = 60;
/// Upper bound on SameBoy's DMG boot ROM; it hands over after 80-85 frames
/// (measured 2026-11-06 at ea667ad), so this only stops a hang.
const BOOT_FRAMES_MAX: u64 = 300;
/// One DMG frame is 70,224 T-cycles; `GB_run` counts 8 MHz ticks, two per T-cycle at single speed.
const TICKS_PER_FRAME: u64 = 70_224 * 2;

static LOG: Mutex<Vec<String>> = Mutex::new(Vec::new());

extern "C" fn route_log(
    _gb: *mut sb::GB_gameboy_t,
    string: *const c_char,
    _attributes: sb::GB_log_attributes_t,
) {
    // SAFETY: SameBoy passes a valid NUL-terminated string that outlives the call.
    let line = unsafe { CStr::from_ptr(string) }
        .to_string_lossy()
        .into_owned();
    LOG.lock().unwrap().push(line);
}

unsafe extern "C" {
    fn dup(fd: c_int) -> c_int;
    fn dup2(from: c_int, to: c_int) -> c_int;
    fn close(fd: c_int) -> c_int;
    fn fflush(stream: *mut c_void) -> c_int;
}

const STDOUT_FD: c_int = 1;

/// Runs `body` with file descriptor 1 pointed at a temporary file and returns
/// everything written there, including C stdio output that bypasses Rust's capture.
fn capture_stdout(body: impl FnOnce()) -> Vec<u8> {
    let path = std::env::temp_dir().join(format!("gbcli-smoke-stdout-{}", std::process::id()));
    let file = File::create(&path).unwrap();
    // SAFETY: plain POSIX descriptor calls on descriptors this function owns;
    // fflush(NULL) flushes every C stream so nothing buffered crosses the swap.
    let saved = unsafe {
        fflush(std::ptr::null_mut());
        let saved = dup(STDOUT_FD);
        assert!(saved >= 0, "dup(1) failed");
        assert!(
            dup2(file.as_raw_fd(), STDOUT_FD) >= 0,
            "dup2 onto stdout failed"
        );
        saved
    };
    body();
    unsafe {
        fflush(std::ptr::null_mut());
        dup2(saved, STDOUT_FD);
        close(saved);
    }
    drop(file);
    let mut captured = Vec::new();
    File::open(&path)
        .unwrap()
        .read_to_end(&mut captured)
        .unwrap();
    std::fs::remove_file(&path).unwrap();
    captured
}

#[test]
fn boots_a_rom_headless_for_60_frames_without_writing_to_stdout() {
    let mut cartridge_ticks: u64 = 0;
    let stdout = capture_stdout(|| unsafe {
        let gb = sb::GB_init(sb::GB_alloc(), sb::GB_MODEL_DMG_B);
        sb::GB_set_log_callback(gb, Some(route_log));
        sb::GB_set_rendering_disabled(gb, true);
        sb::GB_set_turbo_mode(gb, true, true);
        sb::GB_set_turbo_cap(gb, 0.0);
        let boot = sb::boot_roms::DMG;
        sb::GB_load_boot_rom_from_buffer(gb, boot.as_ptr(), boot.len());
        sb::GB_load_rom_from_buffer(gb, SMOKE_ROM.as_ptr(), SMOKE_ROM.len());

        // Boot: the ROM's log line, written right after handover, is the clock.
        let mut boot_ticks: u64 = 0;
        while LOG.lock().unwrap().is_empty() && boot_ticks < BOOT_FRAMES_MAX * TICKS_PER_FRAME {
            boot_ticks += u64::from(sb::GB_run(gb));
        }
        while cartridge_ticks < FRAMES * TICKS_PER_FRAME {
            cartridge_ticks += u64::from(sb::GB_run(gb));
        }
        sb::GB_free(gb);
        sb::GB_dealloc(gb);
    });

    assert_eq!(
        String::from_utf8_lossy(&stdout),
        "",
        "SameBoy wrote to stdout"
    );
    // Proves the ROM got past the boot ROM and ran, and that logging was routed, not lost.
    assert_eq!(LOG.lock().unwrap().as_slice(), [EXPECTED_LOG]);
    assert!(cartridge_ticks >= FRAMES * TICKS_PER_FRAME);
}
