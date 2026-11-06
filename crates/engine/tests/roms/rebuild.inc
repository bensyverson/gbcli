; Rebuilt-ROM test program for gbcli. Written for this project; same license as gbcli.
;
; One program in four builds (rebuild_v1.asm to rebuild_v4.asm set VARIANT), for
; the checkpoint rebuilt-ROM check (design §6):
;   1  the original;
;   2  inserts `sla b` before Main.loop, so Main.loop and Work move (the DX task's v2);
;   3  appends a routine after Work, so no live symbol moves;
;   4  inserts a WRAM byte before wFrame, so every RAM symbol moves.
; Main calls Work forever; Work reads wSpeed and wNever (never written) and
; counts frames in wFrame (16-bit) on each pass. The stack lives in WRAM.

SECTION "Header", ROM0[$100]
    nop
    jp Main
    ds $150 - @, 0  ; rgbfix fills in the logo and checksums

SECTION "Main", ROM0[$150]
Main:
    di
    ld sp, wStackTop
    xor a
    ld [wFrame], a
    ld [wFrame + 1], a
    ld [wSpeed], a
IF VARIANT == 2
    sla b
ENDC
.loop:
    call Work
    jr .loop

Work:
    ld a, [wSpeed]
    add a, b
    ld b, a
    ld hl, wFrame
    inc [hl]
    jr nz, .done
    inc hl
    inc [hl]
.done:
    ret

IF VARIANT == 3
Extra:
    ret
ENDC

SECTION "Variables", WRAM0[$C000]
IF VARIANT == 4
wPad: ds 1
ENDC
wFrame: ds 2
wSpeed: ds 1
wNever: ds 1
wStack: ds $20
wStackTop:
