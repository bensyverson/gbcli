; STOP test ROM for gbcli. Written for this project; same license as gbcli.
;
; Deselects both joypad rows, so no key can wake the CPU, then executes STOP
; with interrupts off. Nothing after it should run.
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
    ld a, $30
    ldh [$FF00], a    ; JOYP: neither row selected
    stop
    ld a, $99
    ld [$C000], a
.spin:
    jr .spin
