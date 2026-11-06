#!/bin/zsh
# Measures gbcli's debug build times: clean, then incremental after touching the binary crate
# (crates/gbcli/src/main.rs) and after touching the engine crate (crates/engine/src/lib.rs).
# Uses its own target directory (target/bench-workspace-build) so your normal build cache is untouched.
#
#   scripts/bench/workspace-build/run.sh [runs]     # runs per incremental case, default 5; prints medians
set -euo pipefail
zmodload zsh/datetime

repo_root=${0:A:h:h:h:h}
runs=${1:-5}
export CARGO_TARGET_DIR=$repo_root/target/bench-workspace-build
cd "$repo_root"

# Seconds taken by `cargo build` (debug, whole workspace), to two decimals.
timed_build() {
    local start=$EPOCHREALTIME
    cargo build --quiet --workspace
    printf '%.2f' $(( EPOCHREALTIME - start ))
}

median() {
    local sorted=(${(n)@})
    print -r -- ${sorted[$(( (${#sorted} + 1) / 2 ))]}
}

incremental() {
    local file=$1 samples=() i
    for i in {1..$runs}; do
        touch "$file"
        samples+=$(timed_build)
    done
    print -r -- "$(median $samples) s (runs: ${samples[*]})"
}

print "rustc: $(rustc --version)"
print "cc:    $(cc --version | head -1)"
print "host:  $(uname -sm), $(sysctl -n machdep.cpu.brand_string 2>/dev/null), $(sysctl -n hw.ncpu 2>/dev/null || nproc) CPUs"

cargo clean --quiet
print "clean debug build:            $(timed_build) s"
print "incremental, touch gbcli:     $(incremental crates/gbcli/src/main.rs)"
print "incremental, touch engine:    $(incremental crates/engine/src/lib.rs)"
