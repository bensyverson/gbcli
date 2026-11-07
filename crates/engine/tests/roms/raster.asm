; Raster-timeline test ROM for gbcli. Written for this project; same license as gbcli.
;
; Turns the LCD on, then every frame makes its register writes at places
; the tests know: polling LY finds the line, polling STAT's mode places the
; write within it.
;   - ScxMid:   SCX on line 40, during mode 3 (a mid-scanline effect);
;   - BgpMid:   BGP on line 80, during mode 3;
;   - BgpHBlank: BGP on line 120, during mode 0 (after its mode 3);
;   - ScxVBlank: SCX on line 144, at the start of vblank, then LYC, STAT
;     and IF to sleep in HALT until line 30.
; IME stays off; nothing else writes an IO register in the loop.

DEF rSCX EQU $FF43
DEF rLCDC EQU $FF40
DEF rSTAT EQU $FF41
DEF rLY EQU $FF44
DEF rBGP EQU $FF47
DEF rLYC EQU $FF45
DEF rIF EQU $FF0F

; Waits until LY reads \1.
MACRO wait_ly
.ly\@:
    ldh a, [rLY]
    cp \1
    jr nz, .ly\@
ENDM

; Waits until STAT's mode bits read \1.
MACRO wait_mode
.mode\@:
    ldh a, [rSTAT]
    and 3
    cp \1
    jr nz, .mode\@
ENDM

SECTION "Header", ROM0[$100]
    nop
    jp Main
    ds $150 - @, 0  ; rgbfix fills in the logo and checksums

SECTION "Main", ROM0[$150]
Main:
    di
    ld sp, $DFFF
    xor a
    ldh [$FFFF], a  ; IE = 0
LcdOn:
    ld a, $91
    ldh [rLCDC], a  ; LCD on, background on, tiles at $8000
Frame:
    wait_ly 40
    wait_mode 3
    ld a, 16
ScxMid:
    ldh [rSCX], a
    wait_ly 80
    wait_mode 3
    ld a, $1B
BgpMid:
    ldh [rBGP], a
    wait_ly 120
    wait_mode 3
    wait_mode 0
    ld a, $E4
BgpHBlank:
    ldh [rBGP], a
    wait_ly 144
    xor a
ScxVBlank:
    ldh [rSCX], a
    ; Sleep until line 30 of the next frame: HALT wakes on the LYC
    ; coincidence with IME off, so nothing is dispatched. SameBoy runs HALT
    ; in shorter chunks than instructions, which moves where in a chunk the
    ; line starts fall.
    ld a, 30
LycWrite:
    ldh [rLYC], a
    ld a, $40
StatWrite:
    ldh [rSTAT], a  ; the LYC interrupt source
    ld a, $02
    ldh [$FFFF], a  ; IE = STAT
    xor a
IfWrite:
    ldh [rIF], a
    halt
    nop
    jr Frame
