#!/bin/sh
# Web spike (Stage 12, spike/web): builds crates/web for wasm32-wasip1 with
# wasi-sdk's clang for SameBoy's C, and the native references the Node
# checks compare against, then runs the checks. Findings and setup:
# project/2026-11-09-web-spike-findings.md.
#
#   scripts/web/build-spike.sh            build and check
#   WASI_SDK=/path/to/wasi-sdk-34.0-arm64-macos scripts/web/build-spike.sh
set -eu

WASI_SDK="${WASI_SDK:-$HOME/.local/wasi/wasi-sdk-34.0-arm64-macos}"
CARGO="${CARGO:-$HOME/.cargo/bin/cargo}"
ROOT="$(cd "$(dirname "$0")/../.." && pwd)"
cd "$ROOT"

CC_wasm32_wasip1="$WASI_SDK/bin/clang" AR_wasm32_wasip1="$WASI_SDK/bin/llvm-ar" \
  "$CARGO" build -p gbcli-web --target wasm32-wasip1 --release
"$CARGO" build -p gbcli-web --release --examples

WASM=target/wasm32-wasip1/release/gbcli_web.wasm
ls -l "$WASM"

check() {
  node crates/web/node/check.mjs "$WASM" "$1" target/release/examples/native_ref "$2" ${3:+"$3"}
}
check crates/sameboy/tests/roms/layers.gb 600
check crates/sameboy/tests/roms/mode_sweep_win.gb 3000
check crates/sameboy/tests/roms/banked.gb 1200
check crates/audio/tests/roms/tune.gb 600
check crates/engine/tests/roms/joypad.gb 600
check crates/engine/tests/roms/gbc_palette.gb 600 gbc
check crates/sameboy/tests/roms/layers_cgb.gb 1200 gbc
node crates/web/node/colors.mjs "$WASM" crates/engine/tests/roms/gbc_palette.gb target/release/examples/native_colors
