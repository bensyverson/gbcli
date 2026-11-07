; Trap demo for `gbcli help traps`: the HALT bug.
; Written for this project; same license as gbcli.
;
; IME is off and VBlank is both enabled (IE) and requested (IF), so each HALT
; ends at once and the CPU reads the byte after it twice.
;   - Bugged: HALT, then `inc b`: it runs twice, so wBugged = 2;
;   - Fixed:  HALT, then a nop to be read twice, then `inc c`: wFixed = 1.
; Then idles. `gbcli run --frames 1 --read '[wBugged]' '[wFixed]'` reads 2, 1.

SECTION "Header", ROM0[$100]
    nop
    jp Main
    ds $150 - @, 0  ; rgbfix fills in the logo and checksums

SECTION "Main", ROM0[$150]
Main:
    di
    ld sp, $DFFF
    ld bc, 0
    ld a, 1
    ldh [$FFFF], a  ; IE = VBlank
    ldh [$FF0F], a  ; IF = VBlank: pending, and IME = 0 never clears it
Bugged:
    halt
    inc b
    ld a, b
    ld [wBugged], a
Fixed:
    halt
    nop
    inc c
    ld a, c
    ld [wFixed], a
Idle:
    jr Idle

SECTION "Results", WRAM0
wBugged: db
wFixed: db
