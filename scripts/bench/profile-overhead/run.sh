#!/bin/zsh
# What the cycle profiler costs (leaf 6eLWSu): crates/engine/examples/profile_bench.rs
# in modes plain (a run with no sink), profile (execution events and the shadow
# stack, Machine::profile) and frames (the per-frame budget, Machine::frame_budget),
# run in interleaved rounds so load on a shared machine hits every mode alike.
# Prints every run, then each mode's best and median speed and its cost against
# plain's best.
#
# Usage (from anywhere):
#   scripts/bench/profile-overhead/run.sh ROM [rounds (default 5)] [frames (default 3600)]
# e.g.
#   scripts/bench/profile-overhead/run.sh tests/corpus/fetched/blargg/cpu_instrs/cpu_instrs.gb 5 3600
set -euo pipefail

repo_root=${0:A:h:h:h:h}
rom=${1:?usage: run.sh ROM [rounds] [frames]}
rom=${rom:A}
rounds=${2:-5}
frames=${3:-3600}
modes=(plain profile frames)

cd "$repo_root"
cargo build --release -q -p gbcli-engine --example profile_bench
bench=$repo_root/target/release/examples/profile_bench
print "$rounds rounds of $frames frames on ${rom:t} (load: $(sysctl -n vm.loadavg 2>/dev/null || uptime))"

typeset -A speeds
for round in {1..$rounds}; do
    for mode in $modes; do
        line=$("$bench" "$rom" "$frames" "$mode")
        print "round $round: $line"
        speeds[$mode]+="${line##* } "
    done
done

print
best_of() { print -l ${(z)speeds[$1]} | sort -g | tail -1 }
median_of() {
    local -a sorted
    sorted=($(print -l ${(z)speeds[$1]} | sort -g))
    print ${sorted[$(( (${#sorted} + 1) / 2 ))]}
}
plain=$(best_of plain)
for mode in $modes; do
    best=$(best_of $mode)
    cost=$(python3 -I -c "print(f'{($plain / $best - 1) * 100:+.1f}%')")
    print "$mode: best ${best}x, median $(median_of $mode)x realtime; time against plain's best $cost"
done
