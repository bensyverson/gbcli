; Trap demo for `gbcli help traps`: old_syntax.asm as RGBDS 1.0 wants it.
; Written for this project; same license as gbcli.
;
; DEF before a constant, and MACRO before a macro's name. `gbcli build`
; builds it.

DEF rLY EQU $FF44

MACRO wait_vblank
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
