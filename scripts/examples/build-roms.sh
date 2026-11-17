#!/bin/zsh
# Rebuilds each example game's playable ROM: its dual-mode --gbc build,
# which runs on the original Game Boy and in color on a Game Boy Color.
# Writes examples/<game>/<title>.gb, the file the READMEs link to, and its
# .sym for emulator debuggers; the .map build writes beside them is removed.
#
# Usage: scripts/examples/build-roms.sh [path/to/gbcli]
# Needs RGBDS on PATH. Defaults to the gbcli on PATH.
set -euo pipefail
gbcli=${1:-gbcli}
root=${0:A:h:h:h}

build() {
  local game=$1 title=$2
  "$gbcli" build "$root/examples/$game" --gbc --out "$root/examples/$game/$title.gb"
  rm -f "$root/examples/$game/$title.map"
}

build racer switchback
build shooter vesper-swarm
build rhythm lantern-drum
build rogue lanternfall
build raycaster lantern
build factory teddy-and-toys
