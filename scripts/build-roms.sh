#!/bin/zsh
# Builds the ROM binaries the repository commits, so a normal `cargo build` needs no RGBDS:
#   - SameBoy's open-source boot ROMs (Expat) from crates/sameboy-sys/SameBoy/BootROMs/*.asm
#     into crates/sameboy-sys/bootroms/, with their license alongside;
#   - gbcli's own test ROMs from crates/sameboy-sys/tests/roms/*.asm, crates/sameboy/tests/roms/*.asm
#     and crates/engine/tests/roms/*.asm, each with its .sym (an .asm may INCLUDE an .inc beside it)
#     (a name ending in _boot assembles to an unpadded .bin boot ROM, the rest to fixed-up .gb carts).
# Never put Nintendo boot ROM dumps in the repository.
#
# Requires RGBDS (tested with v1.0.4) and a C compiler (for SameBoy's pb12 logo compressor).
# Run from anywhere:   scripts/build-roms.sh
set -euo pipefail

repo_root=${0:A:h:h}
sys=$repo_root/crates/sameboy-sys
src=$sys/SameBoy/BootROMs
out=$sys/bootroms
rom_dirs=($sys/tests/roms $repo_root/crates/sameboy/tests/roms $repo_root/crates/engine/tests/roms)

work=$(mktemp -d "${TMPDIR:-/tmp}/gbcli-roms.XXXXXX")
trap 'rm -rf "$work"' EXIT

print "rgbasm: $(rgbasm --version)"

# The boot ROMs incbin a compressed logo: PNG -> 2bpp (rgbgfx) -> pb12 (SameBoy's compressor).
# Flags follow SameBoy's Makefile for RGBDS 0.6+.
cc -std=c99 -Wall -Werror "$src/pb12.c" -o "$work/pb12"
rgbgfx -Z -u -c embedded -o "$work/SameBoyLogo.2bpp" "$src/SameBoyLogo.png"
"$work/pb12" < "$work/SameBoyLogo.2bpp" > "$work/SameBoyLogo.pb12"

# The same list as SameBoy's `make bootroms`, plus the fast CGB boot.
boot_roms=(dmg_boot mgb_boot cgb_boot cgb0_boot cgb_boot_fast agb_boot sgb_boot sgb2_boot)
mkdir -p "$out"
for name in $boot_roms; do
    rgbasm --include "$work/" --include "$src/" -o "$work/$name.o" "$src/$name.asm"
    rgblink -x -o "$out/$name.bin" "$work/$name.o"
    print "built bootroms/$name.bin ($(wc -c < "$out/$name.bin" | tr -d ' ') bytes)"
done
cp "$sys/SameBoy/LICENSE" "$out/LICENSE"

for roms in $rom_dirs; do
    for asm in $roms/*.asm; do
        name=${asm:t:r}
        rgbasm --include "$roms/" -o "$work/$name.o" "$asm"
        if [[ $name == *_boot ]]; then
            rgblink -x -o "$roms/$name.bin" "$work/$name.o"
            print "built ${roms#$repo_root/}/$name.bin"
        else
            rgblink -n "$roms/$name.sym" -o "$roms/$name.gb" "$work/$name.o"
            rgbfix -v -p 0 "$roms/$name.gb"
            print "built ${roms#$repo_root/}/$name.gb"
        fi
    done
done
