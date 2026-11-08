#!/bin/zsh
# Prints one SHA-256 over every file under a directory: the SHA-256 and relative path of each
# file, in byte order of the paths. scripts/vendor-sameboy.sh records it for upstream's Core/
# (crates/sameboy-sys/SameBoy/CORE_SHA256), and crates/sameboy-sys/tests/it/patch_set.rs checks
# that the vendored Core/ with every patch reverse-applied still gives it.
#   scripts/sameboy-core-hash.sh DIR
set -euo pipefail

cd -- "$1"
find . -type f -print0 | LC_ALL=C sort -z | xargs -0 shasum -a 256 | shasum -a 256 | cut -d ' ' -f 1
