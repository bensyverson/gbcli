#!/bin/zsh
# What gbcli's SameBoy patches cost with the layer buffer detached (capture off):
# scripts/bench/sameboy-throughput/sb_bench.c, timed in process CPU seconds, built once
# against the vendored tree and once against the same tree with every patch in
# crates/sameboy-sys/patches/ reverse-applied (no network, no git). Both use build.rs's
# flags. Each pair of runs starts simultaneously, so load on a shared machine hits
# both alike; the script prints each pair's cost and the median.
#
# Usage (from anywhere):
#   scripts/bench/layer-patch-cost/run.sh ROM [pairs (default 8)] [frames (default 7200)]
# e.g.
#   scripts/bench/layer-patch-cost/run.sh tests/corpus/fetched/blargg/cpu_instrs/cpu_instrs.gb
set -euo pipefail
setopt no_bg_nice  # both halves of a pair at the same priority

repo_root=${0:A:h:h:h:h}
rom=${1:?usage: run.sh ROM [pairs] [frames]}
rom=${rom:A}
pairs=${2:-8}
frames=${3:-7200}
sys=$repo_root/crates/sameboy-sys
bench=$repo_root/scripts/bench/sameboy-throughput/sb_bench.c

work=$(mktemp -d "${TMPDIR:-/tmp}/gbcli-layer-cost.XXXXXX")
trap 'rm -rf "$work"' EXIT

# The unpatched tree: the vendored Core with the patches undone, last patch first.
mkdir -p "$work/unpatched/crates/sameboy-sys/SameBoy"
cp -R "$sys/SameBoy/Core" "$work/unpatched/crates/sameboy-sys/SameBoy/Core"
patches=("$sys"/patches/*.patch)
for patch in ${(Oa)patches}; do
    (cd "$work/unpatched" && patch -s -R -p1 < "$patch")
done
if grep -q layer_buffer "$work/unpatched/crates/sameboy-sys/SameBoy/Core/gb.h"; then
    print "reverse-applying the patches left the layer buffer in place" >&2
    exit 1
fi

# build.rs's flags and source list (crates/sameboy-sys/build.rs).
flags=(-O3 -std=gnu11 -D_GNU_SOURCE -DGB_INTERNAL -DNDEBUG -DGB_VERSION='"bench"'
       -DGB_COPYRIGHT_YEAR='"2026"' -DGB_DISABLE_DEBUGGER -DGB_DISABLE_REWIND
       -DGB_DISABLE_CHEAT_SEARCH -DSB_BENCH_CPU_TIME)
build() {
    local tree=$1 out=$2
    local sources=(${(f)"$(print -l "$tree"/Core/*.c | grep -vE '/(debugger|sm83_disassembler|symbol_hash|rewind|cheat_search)\.c$')"})
    cc $flags -Wno-everything -I"$tree" -o "$out" "$bench" $sources -lm
}
build "$work/unpatched/crates/sameboy-sys/SameBoy" "$work/sb_unpatched"
build "$sys/SameBoy" "$work/sb_patched"
print "built both; $pairs pairs of $frames frames on ${rom:t} (load: $(sysctl -n vm.loadavg 2>/dev/null || uptime))"

cpu_seconds() { sed -nE 's/.* cpu=([0-9.]+)s .*/\1/p' "$1"; }
costs=()
for i in {1..$pairs}; do
    "$work/sb_unpatched" "$rom" "$frames" 1 > "$work/u.txt" &
    local u=$!
    "$work/sb_patched" "$rom" "$frames" 1 > "$work/p.txt" &
    local p=$!
    wait $u $p
    local tu=$(cpu_seconds "$work/u.txt") tp=$(cpu_seconds "$work/p.txt")
    local cost=$(( (tp / tu - 1) * 100 ))
    costs+=($cost)
    printf 'pair %d: unpatched %.3fs, patched %.3fs, cost %+.2f%%\n' $i $tu $tp $cost
done
sorted=(${(f)"$(print -l $costs | sort -g)"})
n=${#sorted}
if (( n % 2 )); then
    median=${sorted[$(( (n + 1) / 2 ))]}
else
    median=$(( (sorted[n / 2] + sorted[n / 2 + 1]) / 2 ))
fi
printf 'median cost %+.2f%% (range %+.2f%% to %+.2f%%) over %d pairs\n' $median $sorted[1] $sorted[-1] $n
