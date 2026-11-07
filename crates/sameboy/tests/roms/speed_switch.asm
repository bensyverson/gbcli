; CGB speed-switch test ROM for gbcli. Written for this project; same license
; as gbcli.
;
; Switches to double speed and back, the standard way: interrupts off, IE = 0,
; no joypad row selected, KEY1 bit 0 set, then STOP. Each STOP pauses the CPU
; for about 2^17 cycles and then resumes on its own at the next instruction.
;   $C000  set to $99 once execution continues past the first STOP (to double)
;   $C001  set to $99 once execution continues past the second (to normal)

SECTION "Header", ROM0[$100]
    nop
    jp Main
    ds $143 - @, 0
    db $80              ; CGB flag: CGB features, DMG compatible
    ds $150 - @, 0      ; rgbfix fills in the logo and checksums

SECTION "Main", ROM0[$150]
Main:
    di
    xor a
    ld [$C000], a
    ld [$C001], a
    ldh [$FFFF], a      ; IE = 0
    ld a, $30
    ldh [$FF00], a      ; JOYP: no row selected, so no key ends STOP
    ld a, 1
    ldh [$FF4D], a      ; KEY1: prepare a speed switch
    stop
ToDouble::
    ld a, $99
    ld [$C000], a
    ld a, 1
    ldh [$FF4D], a      ; KEY1: prepare the switch back
    stop
ToNormal::
    ld a, $99
    ld [$C001], a
.spin:
    jr .spin
