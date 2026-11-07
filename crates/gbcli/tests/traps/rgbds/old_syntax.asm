; Trap demo for `gbcli help traps`: syntax older RGBDS took and 1.0.4 refuses.
; Written for this project; same license as gbcli.
;
; Many tutorials and older hardware.inc copies define constants and macros
; this way. `gbcli build` fails with one diagnostic row per error, each with
; its file and line. new_syntax.asm is the same program as RGBDS 1.0 wants it.

rLY EQU $FF44

wait_vblank: MACRO
.wait\@
    ldh a, [rLY]
    cp 144
    jr c, .wait\@
ENDM

SECTION "Header", ROM0[$100]
    nop
    jp Main
    ds $150 - @, 0

SECTION "Main", ROM0
Main:
    di
    wait_vblank
.idle
    jr .idle
