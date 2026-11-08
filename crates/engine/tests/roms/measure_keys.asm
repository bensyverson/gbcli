; Held-input test ROM for `measure --keys`. Written for this project; same license as gbcli.
;
; Once per frame (at LY 144, polled with interrupts off), Work reads the
; button row of JOYP and, while A is held, spins a 100-pass delay loop, so
; the cycles from Work to Work.done tell a held A from a released one.

SECTION "Header", ROM0[$100]
    nop
    jp Main
    ds $150 - @, 0  ; rgbfix fills in the logo and checksums

SECTION "Main", ROM0[$150]
Main:
    di
    ld sp, $DFFF
    ld a, $80       ; LCD on: a quick boot may leave it off, and LY counts only with it on
    ldh [$FF40], a  ; rLCDC
.frame:
    ldh a, [$FF44]  ; rLY
    cp 144
    jr nz, .frame
    call Work
.leave:
    ldh a, [$FF44]
    cp 144
    jr z, .leave
    jr .frame

Work:
    ld a, $10       ; P15 low: the button row
    ldh [$FF00], a
    ldh a, [$FF00]
    ldh a, [$FF00]  ; read twice to let the lines settle
    ld b, a
    ld a, $30       ; deselect both rows
    ldh [$FF00], a
    bit 0, b        ; A, low while held
    jr nz, .done
    ld c, 100
.spin:
    dec c
    jr nz, .spin
.done:
    ret
