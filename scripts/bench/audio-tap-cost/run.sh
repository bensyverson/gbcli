#!/bin/zsh
# What the sound-register write log and sample capture cost the SameBoy run loop
# (leaf aDOL0w): crates/sameboy/examples/bench.rs in modes plain (both off),
# apu-log, audio and apu-log+audio, run in interleaved rounds so load on a shared
# machine hits every mode alike. Prints every run, then each mode's best and median
# speed and its cost against plain's best.
#
# With a base revision (a commit before the log and capture existed), it also builds
# that revision's bench from `git archive` in a temporary directory and runs its
# plain and tracking modes in the same rounds as base:plain and base:tracking, next
# to this tree's tracking mode: the cost of both features when off.
#
# Usage (from anywhere):
#   scripts/bench/audio-tap-cost/run.sh ROM [rounds (default 5)] [frames (default 3600)] [base-rev]
# e.g.
#   scripts/bench/audio-tap-cost/run.sh tests/corpus/fetched/blargg/cpu_instrs/cpu_instrs.gb 5 3600 main
set -euo pipefail

repo_root=${0:A:h:h:h:h}
rom=${1:?usage: run.sh ROM [rounds] [frames]}
rom=${rom:A}
rounds=${2:-5}
frames=${3:-3600}
base=${4:-}
modes=(plain apu-log audio apu-log+audio)

cd "$repo_root"
cargo build --release -q -p gbcli-sameboy --example bench
bench=$repo_root/target/release/examples/bench
if [[ -n $base ]]; then
    work=$(mktemp -d "${TMPDIR:-/tmp}/gbcli-audio-cost.XXXXXX")
    trap 'rm -rf "$work"' EXIT
    mkdir "$work/src"
    git archive "$base" | tar -x -C "$work/src"
    (cd "$work/src" && CARGO_TARGET_DIR="$work/target" cargo build --release -q -p gbcli-sameboy --example bench)
    modes+=(tracking base:plain base:tracking)
fi
print "$rounds rounds of $frames frames on ${rom:t} (load: $(sysctl -n vm.loadavg 2>/dev/null || uptime))"

typeset -A speeds
for round in {1..$rounds}; do
    for mode in $modes; do
        if [[ $mode == base:* ]]; then
            line=$("$work/target/release/examples/bench" "$rom" "$frames" 1 "${mode#base:}" | grep '^throughput:')
        else
            line=$("$bench" "$rom" "$frames" 1 "$mode" | grep '^throughput:')
        fi
        speed=$(print -r -- "$line" | sed -nE 's/.* speed=([0-9.]+)x .*/\1/p')
        speeds[$mode]="${speeds[$mode]:-} $speed"
        print -r -- "round $round: $line"
    done
done

best() { print -l ${=1} | sort -g | tail -1; }
median() {
    local sorted=(${(f)"$(print -l ${=1} | sort -g)"})
    local n=${#sorted}
    if (( n % 2 )); then print ${sorted[$(( (n + 1) / 2 ))]}
    else print $(( (sorted[n / 2] + sorted[n / 2 + 1]) / 2.0 )); fi
}
plain=$(best "${speeds[plain]}")
for mode in $modes; do
    b=$(best "${speeds[$mode]}")
    printf '%-14s best %6.1fx  median %6.1fx  cost vs plain (best) %+6.1f%%\n' \
        $mode $b $(median "${speeds[$mode]}") $(( (plain / b - 1) * 100 ))
done
