//! Links the wasm32-wasip1 cdylib as a WASI *reactor*: wasi-libc's
//! `crt1-reactor.o` defines `_initialize`, which runs the C and Rust
//! constructors once. Without it wasm-ld wraps every export in a call to
//! `__wasm_call_ctors`, so SameBoy's `band_limited_init` constructor (8,192
//! `sin`/`cos` calls) ran on every `gbw_alloc`, `gbw_free` and
//! `gbw_request`, and dominated a one-frame request (findings doc ▸
//! Per-request cost). Spike code: disposable.

use std::path::PathBuf;
use std::process::Command;

fn main() {
    println!("cargo:rerun-if-changed=build.rs");
    let target = std::env::var("TARGET").unwrap_or_default();
    if target != "wasm32-wasip1" {
        return;
    }
    let rustc = std::env::var("RUSTC").unwrap_or_else(|_| "rustc".into());
    let out = Command::new(rustc)
        .args(["--print", "sysroot"])
        .output()
        .expect("rustc --print sysroot");
    let sysroot = PathBuf::from(String::from_utf8(out.stdout).expect("UTF-8").trim());
    let crt = sysroot.join("lib/rustlib/wasm32-wasip1/lib/self-contained/crt1-reactor.o");
    assert!(crt.exists(), "no {}", crt.display());
    println!("cargo:rustc-link-arg-cdylib={}", crt.display());
    println!("cargo:rustc-link-arg-cdylib=--export=_initialize");
}
