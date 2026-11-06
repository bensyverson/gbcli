; A minimal boot ROM for gbcli's tests. Written for this project; same license as gbcli.
;
; Hands over to the cartridge in four instructions instead of the ~84 frames a
; real boot ROM takes, so tests start fast. Registers and PPU state at
; handover are NOT those of a real boot; tests that care use a built-in boot ROM.
; Assembled to quick_boot.bin (256 bytes, unpadded) by scripts/build-roms.sh.

SECTION "Boot", ROM0[$0]
    ld sp, $FFFE
    jp Handover
    ds $FC - @, 0
Handover:              ; $00FC: writing FF50 unmaps this ROM, so PC lands on $0100
    ld a, 1
    ldh [$FF50], a
