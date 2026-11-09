; PPU-mode test ROM for gbcli. Written for this project; same license as gbcli.
;
; Writes SCX at places the tests know, without reading a PPU register first,
; so SameBoy's display is still batched (has cycles it hasn't run) when each
; write's instruction is reached. HALT wakes on the LYC coincidence at a
; line's start; counted NOPs then place each write:
;   - Line50Oam:     line 50, in mode 2 (OAM scan);
;   - Line50Drawing: line 50, in mode 3, early, on a line with no objects;
;   - Line50HBlank:  line 50, in mode 0;
;   - Line100Late:   line 100, late in a mode 3 made long by ten objects,
;     with the HBlank interrupt off, so SameBoy batches mode 3 by an upper
;     bound rather than its exact length.
; Every write stores 0, so SCX & 7 stays 0. IME stays off.

DEF rLCDC EQU $FF40
DEF rSTAT EQU $FF41
DEF rSCX EQU $FF43
DEF rLY EQU $FF44
DEF rLYC EQU $FF45
DEF rIF EQU $FF0F
DEF rIE EQU $FFFF

SECTION "Header", ROM0[$100]
    nop
    jp Main
    ds $150 - @, 0  ; rgbfix fills in the logo and checksums

SECTION "Main", ROM0[$150]
Main:
    di
    ld sp, $DFFF
    xor a
    ldh [rIE], a
    ; OAM can only be written with the LCD off: wait for vblank if it is on.
    ldh a, [rLCDC]
    bit 7, a
    jr z, .off
.vblank:
    ldh a, [rLY]
    cp 144
    jr c, .vblank
    xor a
    ldh [rLCDC], a
.off:
    ; Objects 0-9 on lines 100-107, 16 pixels apart; 10-39 hidden.
    ld hl, $FE00
    ld b, 10
    ld c, 8
.visible:
    ld a, 100 + 16
    ld [hl+], a
    ld a, c
    ld [hl+], a
    add 16
    ld c, a
    xor a
    ld [hl+], a
    ld [hl+], a
    dec b
    jr nz, .visible
    ld b, 30 * 4
.hidden:
    ld [hl+], a
    dec b
    jr nz, .hidden
    xor a
    ldh [rSCX], a
    ld a, $93
    ldh [rLCDC], a  ; LCD, background and objects on, tiles at $8000
Frame:
    ld a, 50
    call SleepToLine
    xor a
Line50Oam:
    ldh [rSCX], a
    REPT 20
        nop
    ENDR
Line50Drawing:
    ldh [rSCX], a
    REPT 50
        nop
    ENDR
Line50HBlank:
    ldh [rSCX], a
    ld a, 100
    call SleepToLine
    REPT 60
        nop
    ENDR
    xor a
Line100Late:
    ldh [rSCX], a
    jp Frame

; Sleeps in HALT until LY reaches A: the LYC coincidence wakes it with IME
; off, so nothing is dispatched.
SleepToLine:
    ldh [rLYC], a
    ld a, $40
    ldh [rSTAT], a  ; the LYC interrupt source only
    ld a, $02
    ldh [rIE], a
    xor a
    ldh [rIF], a
    halt
    nop
    ret
