; STOP-and-wake test ROM for gbcli. Written for this project; same license as gbcli.
;
; Selects the direction row only, then executes STOP with interrupts off, so
; a direction key wakes the CPU and an action key doesn't.
;   $C000  set to $99 only if execution continues past STOP

SECTION "Header", ROM0[$100]
    nop
    jp Main
    ds $150 - @, 0

SECTION "Main", ROM0[$150]
Main:
    di
    xor a
    ld [$C000], a
    ldh [$FFFF], a    ; IE = 0
    ld a, $20
    ldh [$FF00], a    ; JOYP: direction row selected (P14 low)
    stop
    ld a, $99
    ld [$C000], a
.spin:
    jr .spin
