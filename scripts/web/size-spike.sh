#!/bin/sh
# Web spike (Stage 12, spike/web): the .wasm's size raw, stripped, and
# stripped under gzip -9 and brotli -q 11 (brotli if installed), for the
# module as built (with the CLI) and, with --no-default-features, the
# handler alone. Findings: project/2026-11-09-web-spike-findings.md.
#
#   scripts/web/size-spike.sh
set -eu

WASI_SDK="${WASI_SDK:-$HOME/.local/wasi/wasi-sdk-34.0-arm64-macos}"
CARGO="${CARGO:-$HOME/.cargo/bin/cargo}"
ROOT="$(cd "$(dirname "$0")/../.." && pwd)"
cd "$ROOT"
OUT=target/web-spike/size
mkdir -p "$OUT"

build() {
  CC_wasm32_wasip1="$WASI_SDK/bin/clang" AR_wasm32_wasip1="$WASI_SDK/bin/llvm-ar" \
    "$CARGO" build -q -p gbcli-web --target wasm32-wasip1 --release "$@"
}

measure() {
  "$WASI_SDK/bin/llvm-strip" -o "$OUT/$1.wasm" "$2"
  raw=$(wc -c < "$2" | tr -d ' ')
  stripped=$(wc -c < "$OUT/$1.wasm" | tr -d ' ')
  gz=$(gzip -9 -c "$OUT/$1.wasm" | wc -c | tr -d ' ')
  br=-
  if command -v brotli > /dev/null; then
    br=$(brotli -c -q 11 "$OUT/$1.wasm" | wc -c | tr -d ' ')
  fi
  echo "$1: raw $raw, stripped $stripped, gzip -9 $gz, brotli -q 11 $br"
}

build
measure with-cli target/wasm32-wasip1/release/gbcli_web.wasm
build --no-default-features --target-dir target/web-spike/nocli
measure handler-only target/web-spike/nocli/wasm32-wasip1/release/gbcli_web.wasm
