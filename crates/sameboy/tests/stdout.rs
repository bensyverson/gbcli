//! Nothing the backend does reaches stdout.
//!
//! This file must stay the only test in its binary: it redirects the process's
//! file descriptor 1, which would swallow output from tests running beside it.

mod common;

use std::ffi::{c_int, c_void};
use std::fs::File;
use std::io::Read;
use std::os::fd::AsRawFd;

use common::*;
use gbcli_core::{Budget, Core, CoreConfig, Model, StopReason};
use gbcli_sameboy::SameBoy;

unsafe extern "C" {
    fn dup(fd: c_int) -> c_int;
    fn dup2(from: c_int, to: c_int) -> c_int;
    fn close(fd: c_int) -> c_int;
    fn fflush(stream: *mut c_void) -> c_int;
}

const STDOUT_FD: c_int = 1;

/// Runs `body` with descriptor 1 pointed at a file and returns what reached it,
/// including C stdio output that bypasses Rust's capture.
fn capture_stdout(body: impl FnOnce()) -> Vec<u8> {
    let path = std::env::temp_dir().join(format!("gbcli-sameboy-stdout-{}", std::process::id()));
    let file = File::create(&path).unwrap();
    // SAFETY: plain POSIX descriptor calls on descriptors this function owns.
    let saved = unsafe {
        fflush(std::ptr::null_mut());
        let saved = dup(STDOUT_FD);
        assert!(saved >= 0);
        assert!(dup2(file.as_raw_fd(), STDOUT_FD) >= 0);
        saved
    };
    body();
    // SAFETY: restores the stdout descriptor saved above, which is still open.
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
fn nothing_reaches_stdout_even_when_sameboy_logs() {
    let mut logged = Vec::new();
    let stdout = capture_stdout(|| {
        // An illegal opcode (stopped before SameBoy would log its lock).
        let mut core = quick_dmg(ILLEGAL);
        assert!(matches!(
            run(&mut core, Budget::frames(1)).reason,
            StopReason::Locked { .. }
        ));
        // A boot through every built-in boot ROM, and a refused load.
        for model in [Model::DmgB, Model::Mgb, Model::Sgb, Model::CgbE, Model::Agb] {
            let mut core = SameBoy::new(CoreConfig::new(model, BUSY.to_vec())).unwrap();
            run(&mut core, Budget::frames(300));
            let _ = core.load(b"nope");
            let _ = core.bus_read(0xFE00);
            core.bus_write(0xF000, 0x42).unwrap(); // SameBoy logs echo-RAM writes
            logged.extend(core.take_log());
        }
    });
    assert_eq!(
        String::from_utf8_lossy(&stdout),
        "",
        "something wrote to stdout"
    );
    assert!(
        logged.iter().any(|line| line.contains("RAM Mirror")),
        "SameBoy's log lines were lost, not routed: {logged:?}"
    );
}
