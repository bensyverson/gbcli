; Per-line scroll test ROM for gbcli. Written for this project; same license as gbcli.
;
; Turns the LCD off and on once, so the frames that follow start on the line
; SameBoy gives no line callback for, then every frame writes the scroll at
; places the tests know:
;   - line 20, mode 2: SCY = 7, which line 20 is drawn with (a HALT that
;     wakes on the STAT mode 2 interrupt places it, since polling LY and then
;     STAT can run past mode 2's 80 dots);
;   - line 40, mode 3: SCX = 16, after line 40's drawing began, so line 41 is
;     the first drawn with it;
;   - line 60, mode 0: SCX = 32, so line 61 is the first drawn with it;
;   - line 60, mode 0, after the SCX write: LCDC = $90 (the background off),
;     so line 61 is the first drawn with bit 0 clear;
;   - line 100, mode 0: LCDC = $91, so line 101 is the first drawn with it set
;     again;
;   - line 144: both scrolls back to 0.
; IME stays off.

DEF rLCDC EQU $FF40
DEF rSTAT EQU $FF41
DEF rSCY EQU $FF42
DEF rSCX EQU $FF43
DEF rLY EQU $FF44
DEF rIF EQU $FF0F
DEF rIE EQU $FFFF

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
    wait_ly 144
    xor a
    ldh [rSCX], a
    ldh [rSCY], a
    ldh [rLCDC], a  ; LCD off
    ld a, $20
    ldh [rSTAT], a  ; the mode 2 interrupt source, for HALT to wake on
    ld a, $02
    ldh [rIE], a    ; IE = STAT; IME is off, so nothing is dispatched
    ld a, $91
    ldh [rLCDC], a  ; LCD on
Frame:
    wait_ly 19
    xor a
    ldh [rIF], a
    halt
    nop
    ld a, 7
ScyOamScan:
    ldh [rSCY], a
    wait_ly 40
    wait_mode 3
    ld a, 16
ScxDrawing:
    ldh [rSCX], a
    wait_ly 60
    wait_mode 3
    wait_mode 0
    ld a, 32
ScxHBlank:
    ldh [rSCX], a
    ld a, $90
LcdcBgOff:
    ldh [rLCDC], a
    wait_ly 100
    wait_mode 3
    wait_mode 0
    ld a, $91
LcdcBgOn:
    ldh [rLCDC], a
    wait_ly 144
    xor a
    ldh [rSCX], a
    ldh [rSCY], a
    jr Frame
