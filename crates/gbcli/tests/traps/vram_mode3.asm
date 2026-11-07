; Trap demo for `gbcli help traps`: a VRAM write during mode 3 is dropped.
; Written for this project; same license as gbcli.
;
; With the LCD off, clears tiles 0 and 1 ($8000-$801F). Turns the LCD on and,
; on line 10:
;   - Dropped: waits for mode 3 and writes $FF to $8000; the PPU owns VRAM
;     then, so the write is ignored;
;   - Kept:    waits for mode 0 (HBlank) and writes $FF to $8010; it lands.
; Then idles. `gbcli run --frames 2 --read '[$8000]' '[$8010]'` reads 0 and 255.

DEF rLCDC EQU $FF40
DEF rSTAT EQU $FF41
DEF rLY EQU $FF44

SECTION "Header", ROM0[$100]
    nop
    jp Main
    ds $150 - @, 0  ; rgbfix fills in the logo and checksums

SECTION "Main", ROM0[$150]
Main:
    di
    ld sp, $DFFF
.vblank             ; the LCD goes off only in VBlank
    ldh a, [rLY]
    cp 144
    jr c, .vblank
    xor a
    ldh [rLCDC], a
    ld hl, $8000
    ld b, 32
.clear
    ld [hli], a
    dec b
    jr nz, .clear
    ld a, $91
    ldh [rLCDC], a  ; LCD on, background on, tiles at $8000
.line10
    ldh a, [rLY]
    cp 10
    jr nz, .line10
.mode3
    ldh a, [rSTAT]
    and 3
    cp 3
    jr nz, .mode3
    ld a, $FF
Dropped:
    ld [$8000], a
.hblank
    ldh a, [rSTAT]
    and 3
    jr nz, .hblank
    ld a, $FF
Kept:
    ld [$8010], a
Idle:
    jr Idle
