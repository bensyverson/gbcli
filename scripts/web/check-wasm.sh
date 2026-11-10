#!/bin/sh
# Checks that main's library crates still compile for wasm32-wasip1, as the
# web-target ruling asks (project/2026-11-09-web-target.md). Compiling isn't
# running: wasip1's std compiles threads, processes and file locks and fails
# at run time (project/2026-11-09-web-spike-findings.md), so this catches
# only what doesn't build.
#
# Needs rustup's stable toolchain with the target, and wasi-sdk for
# SameBoy's C (setup in docs/development.md):
#   rustup target add --toolchain stable wasm32-wasip1
#   wasi-sdk 34 unpacked under ~/.local/wasi
# Homebrew's cargo can't add targets, so this calls rustup's directly.
set -eu

sysroot=$(rustup run stable rustc --print sysroot)
wasi=${WASI_SDK:-$HOME/.local/wasi/wasi-sdk-34.0-arm64-macos}
repo=$(cd "$(dirname "$0")/../.." && pwd)

cd "$repo"
RUSTC="$sysroot/bin/rustc" \
CC_wasm32_wasip1="$wasi/bin/clang" \
AR_wasm32_wasip1="$wasi/bin/llvm-ar" \
  "$sysroot/bin/cargo" check --target wasm32-wasip1 --target-dir target/wasm-check \
  -p gbcli-protocol -p gbcli-bundle -p gbcli-server -p gbcli-cli "$@"
