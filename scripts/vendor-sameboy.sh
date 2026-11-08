#!/bin/zsh
# Vendors SameBoy's Core/, its boot ROM sources and its license into crates/sameboy-sys/SameBoy
# at a given commit (default: latest master), records the commit in SameBoy/COMMIT and the hash
# of upstream's Core/ in SameBoy/CORE_SHA256,
# then re-applies gbcli's patches from crates/sameboy-sys/patches/.
# Run from the repository root:   scripts/vendor-sameboy.sh [commit]
# Then rebuild the boot ROMs:      scripts/build-roms.sh
# and update project/YYYY-MM-DD-sameboy-vendoring.md with the new commit and why.
set -euo pipefail

repo_root=${0:A:h:h}
dest=$repo_root/crates/sameboy-sys/SameBoy
commit=${1:-master}

work=$(mktemp -d "${TMPDIR:-/tmp}/gbcli-vendor.XXXXXX")
trap 'rm -rf "$work"' EXIT

git clone --quiet https://github.com/LIJI32/SameBoy.git "$work/SameBoy"
git -C "$work/SameBoy" checkout --quiet "$commit"
resolved=$(git -C "$work/SameBoy" rev-parse HEAD)

rm -rf "$dest"
mkdir -p "$dest/BootROMs"
cp -R "$work/SameBoy/Core" "$dest/Core"
cp "$work/SameBoy/LICENSE" "$work/SameBoy/version.mk" "$dest/"
# Boot ROM sources only: assembly, includes, the logo and the logo compressor.
cp "$work/SameBoy/BootROMs/"*.asm "$work/SameBoy/BootROMs/"*.inc \
   "$work/SameBoy/BootROMs/pb12.c" "$work/SameBoy/BootROMs/SameBoyLogo.png" "$dest/BootROMs/"
print -r -- "$resolved" > "$dest/COMMIT"
# Upstream's Core/ before our patches: crates/sameboy-sys/tests/it/patch_set.rs checks that the
# vendored Core/ with every patch undone still hashes to this, so no edit bypasses patches/.
"$repo_root/scripts/sameboy-core-hash.sh" "$dest/Core" > "$dest/CORE_SHA256"

# gbcli's own patches, applied on top of the pristine Core. Each is pinned to
# the commit above: a failed check means the commit moved and the patch needs
# rebasing (see the vendoring doc), not that the step can be skipped.
for patch in "$repo_root"/crates/sameboy-sys/patches/*.patch; do
    git -C "$repo_root" apply --check --whitespace=nowarn "$patch"
    git -C "$repo_root" apply --whitespace=nowarn "$patch"
done

print "Vendored SameBoy $resolved into $dest"
