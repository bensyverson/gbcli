; Equivalence-check test program whose one variable is set late: wX is first
; written after the fourth vblank, then counts frames. The two builds differ
; only in where wX lives (WX_PAD bytes into WRAM0), as an optimized build moves
; a variable. Power-on WRAM is random, so until the program writes wX each
; side reads its own unwritten byte. Written for this project; same license as
; gbcli.

SECTION "Header", ROM0[$100]
    nop
    jp Main
    ds $150 - @, 0  ; rgbfix fills in the logo and checksums

SECTION "Main", ROM0[$150]
Main:
    di
    ld a, $91       ; LCD on, so vblanks come
    ldh [$FF40], a
    ld a, 1         ; IE = VBlank, so HALT wakes at each vblank
    ldh [$FFFF], a
    ld b, 4
.wait:
    xor a
    ldh [$FF0F], a  ; IF = 0: HALT waits for the next vblank
    halt
    nop
    dec b
    jr nz, .wait
    xor a
    ld [wX], a
.count:
    xor a
    ldh [$FF0F], a
    halt
    nop
    ld a, [wX]
    inc a
    ld [wX], a
    jr .count

SECTION "Variables", WRAM0[$C000]
    ds WX_PAD
wX: ds 1
