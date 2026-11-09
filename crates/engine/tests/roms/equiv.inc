; Equivalence-check test program for gbcli. Written for this project; same license as gbcli.
;
; The part every build shares: header, vblank vector and start-up. Three builds
; of one tiny game exercise the A/B equivalence check (design §11):
;   equiv_a.asm       the original, written plainly and slowly;
;   equiv_fast.asm    an optimized rewrite (equiv_fast.inc): other code, other
;                     WRAM layout, other routine order, same behavior;
;   equiv_broken.asm  the same rewrite with an off-by-one in the right-edge
;                     clamp, so the block can walk to column 20, off screen;
;   equiv_slow_start.asm  the original with START_DELAY: a longer start-up,
;                     so its LCD comes on later and every vblank is offset.
;
; The game: a solid block on the BG map at (wX, wY), columns 0-19 and rows
; 0-17. Each frame the main loop wakes from HALT on the vblank interrupt, reads
; the joypad (wHeld, wPressed), counts the frame (wFrame, 16 bits), moves the
; block one cell per frame while a direction is held, and on a new A press
; plays a square-wave note whose pitch is NoteTable[wX] (NR21-NR24, in that
; order). The vblank handler erases the block at wOldAddr and draws it at
; the map address of (wX, wY). Each build defines MainLoop, VBlank, the
; routines and the WRAM variables wX, wY, wFrame, wHeld, wPressed, wOldAddr.

DEF rJOYP EQU $FF00
DEF rIF   EQU $FF0F
DEF rNR21 EQU $FF16
DEF rNR22 EQU $FF17
DEF rNR23 EQU $FF18
DEF rNR24 EQU $FF19
DEF rNR50 EQU $FF24
DEF rNR51 EQU $FF25
DEF rNR52 EQU $FF26
DEF rLCDC EQU $FF40
DEF rLY   EQU $FF44
DEF rBGP  EQU $FF47
DEF rIE   EQU $FFFF

DEF MAP EQU $9800

SECTION "VBlank vector", ROM0[$40]
    jp VBlank

SECTION "Header", ROM0[$100]
    nop
    jp Init
    ds $150 - @, 0  ; rgbfix fills in the logo and checksums

SECTION "Init", ROM0[$150]
Init:
    di
    ld sp, $E000
    ldh a, [rLCDC]
    bit 7, a
    jr z, .off
.wait:              ; the LCD may only be turned off in vblank
    ldh a, [rLY]
    cp 144
    jr nz, .wait
    xor a
    ldh [rLCDC], a
.off:
    ld hl, $8000    ; tile 0 blank, tile 1 solid color 3
    ld b, 16
    xor a
.blank:
    ld [hl+], a
    dec b
    jr nz, .blank
    ld b, 16
    ld a, $FF
.solid:
    ld [hl+], a
    dec b
    jr nz, .solid
    ld hl, MAP
    ld bc, $400
.map:
    xor a
    ld [hl+], a
    dec bc
    ld a, b
    or c
    jr nz, .map
    xor a
    ld [wX], a
    ld [wY], a
    ld [wFrame], a
    ld [wFrame + 1], a
    ld [wHeld], a
    ld [wPressed], a
    ld a, LOW(MAP)
    ld [wOldAddr], a
    ld a, HIGH(MAP)
    ld [wOldAddr + 1], a
    ld a, $E4
    ldh [rBGP], a
    ld a, $80       ; sound on, both sides, full volume
    ldh [rNR52], a
    ld a, $77
    ldh [rNR50], a
    ld a, $FF
    ldh [rNR51], a
IF DEF(START_DELAY)
    ld bc, START_DELAY  ; 28 T a pass: the LCD comes on that much later
.delay:
    dec bc
    ld a, b
    or c
    jr nz, .delay
ENDC
    ld a, $91       ; LCD and BG on, tiles at $8000, map at $9800
    ldh [rLCDC], a
    ld a, 1
    ldh [rIE], a
    xor a
    ldh [rIF], a
    ei
    jp MainLoop
