; Trap demo for `gbcli help traps`: turning the LCD off outside VBlank.
; Written for this project; same license as gbcli.
;
; The boot ROM leaves the LCD on.
;   - OffMidFrame: on line 60 writes LCDC bit 7 = 0, mid-frame (on real DMG
;     hardware this can damage the screen);
;   - turns the LCD back on, then
;   - OffInVBlank: waits for line 144 (VBlank) and turns it off again.
; Then idles with the LCD off. `gbcli run --frames 4 --raster` lists both
; LCDC writes under the line each landed on: 60, then 144.

DEF rLCDC EQU $FF40
DEF rLY EQU $FF44

SECTION "Header", ROM0[$100]
    nop
    jp Main
    ds $150 - @, 0  ; rgbfix fills in the logo and checksums

SECTION "Main", ROM0[$150]
Main:
    di
    ld sp, $DFFF
.line60
    ldh a, [rLY]
    cp 60
    jr nz, .line60
    ld a, $11
OffMidFrame:
    ldh [rLCDC], a  ; bit 7 clear: LCD off on line 60
    ld a, $91
    ldh [rLCDC], a  ; on again
.vblank
    ldh a, [rLY]
    cp 144
    jr nz, .vblank
    ld a, $11
OffInVBlank:
    ldh [rLCDC], a
Idle:
    jr Idle
