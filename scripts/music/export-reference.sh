#!/bin/sh
# Exports songs with hUGETracker's own RGBDS export (uge2source), the
# reference that gbcli-music's emitter is compared against (leaf pukTN6).
#
# uge2source is built natively from hUGETracker 1.0.11's source (0623f3f7)
# with Free Pascal 3.2.2 and Lazarus 3.0's LazUtils, the pair hUGETracker's
# own CI builds with. The release's uge2source is x86_64 only, and Rosetta
# isn't a requirement we want. The one change: codegen.pas imports LCL's
# Dialogs unit but never calls it, so an empty Dialogs unit stands in.
#
# Needs: fpc 3.2.2 on PATH (`brew install fpc`), curl, and the network on
# first use. Sources are cached in tests/corpus/.downloads (sha256-checked);
# the binary is built under target/uge2source.
#
# Usage: scripts/music/export-reference.sh [--out DIR] [SONG.uge ...]
#   With no songs, exports every tests/corpus/fetched/hugetracker-samples/*.uge
#   into tests/corpus/fetched/hugetracker-exports/ (gitignored: the songs'
#   license is unstated). Each export's descriptor is `song_theme`, the
#   name the starter's player calls.
set -eu

REPO=$(cd "$(dirname "$0")/../.." && pwd)
CACHE="$REPO/tests/corpus/.downloads"
WORK="$REPO/target/uge2source"
OUT="$REPO/tests/corpus/fetched/hugetracker-exports"

HT_URL="https://codeload.github.com/SuperDisk/hUGETracker/tar.gz/0623f3f7"
HT_FILE="hugetracker-0623f3f7.tar.gz"
HT_SHA="3a1c3b3eebeabdff5c0a389e0e8f3a67ce1c7898d4fd157a3e80969610355e9d"
LAZ_URL="https://gitlab.com/freepascal.org/lazarus/lazarus/-/archive/lazarus_3_0/lazarus-lazarus_3_0.tar.gz?path=components/lazutils"
LAZ_FILE="lazarus-3.0-lazutils.tar.gz"
LAZ_SHA="6832767dea994ab64ceb20938b3daecfb65e4a1a8fa0901c52a90f8e0fa22789"

if [ "${1:-}" = "--out" ]; then
    OUT=$2
    shift 2
fi

command -v fpc >/dev/null || { echo "fpc not found: brew install fpc" >&2; exit 1; }
[ "$(fpc -iV)" = "3.2.2" ] || echo "warning: fpc $(fpc -iV), hUGETracker builds with 3.2.2" >&2

fetch() { # url file sha
    mkdir -p "$CACHE"
    if [ ! -f "$CACHE/$2" ]; then
        curl -fsSL -o "$CACHE/$2.part" "$1"
        mv "$CACHE/$2.part" "$CACHE/$2"
    fi
    got=$(shasum -a 256 "$CACHE/$2" | cut -d' ' -f1)
    if [ "$got" != "$3" ]; then
        echo "$CACHE/$2: sha256 $got, expected $3" >&2
        exit 1
    fi
}

BIN="$WORK/out/uge2source"
if [ ! -x "$BIN" ]; then
    fetch "$HT_URL" "$HT_FILE" "$HT_SHA"
    fetch "$LAZ_URL" "$LAZ_FILE" "$LAZ_SHA"
    rm -rf "$WORK"
    mkdir -p "$WORK/out" "$WORK/stub"
    tar -xzf "$CACHE/$HT_FILE" -C "$WORK" hUGETracker-0623f3f7/src
    tar -xzf "$CACHE/$LAZ_FILE" -C "$WORK"
    printf 'unit Dialogs;\n{$mode objfpc}\ninterface\nimplementation\nend.\n' >"$WORK/stub/dialogs.pas"
    SRC="$WORK/hUGETracker-0623f3f7/src"
    LAZ="$WORK/lazarus-lazarus_3_0-components-lazutils/components/lazutils"
    fpc -Mobjfpc -Sh -O1 -vw0 \
        -Fu"$WORK/stub" -Fu"$LAZ" -Fi"$LAZ" -Fu"$SRC" -Fi"$SRC" \
        -FU"$WORK/out" -FE"$WORK/out" "$SRC/uge2source/uge2source.lpr" >"$WORK/build.log" 2>&1 \
        || { cat "$WORK/build.log" >&2; exit 1; }
fi

if [ $# -eq 0 ]; then
    set -- "$REPO"/tests/corpus/fetched/hugetracker-samples/*.uge
    [ -f "$1" ] || { echo "no samples: python3 -I scripts/corpus/fetch.py --only hugetracker-samples" >&2; exit 1; }
fi

mkdir -p "$OUT"
for song in "$@"; do
    stem=$(basename "$song" .uge)
    "$BIN" "$song" song_theme "$OUT/$stem.asm"
    echo "$OUT/$stem.asm"
done
