; Joypad recorder test ROM for gbcli. Written for this project; same license as gbcli.
;
; At the start of every vblank (LY reaching 144, polled with interrupts off),
; reads both JOYP rows and stores the held buttons at wLog + wCount, then
; increments wCount (8 bits, so the log wraps after 256 frames). A logged byte
; is 1 per held button, in gbcli_core::Keys order: bits 0-3 RIGHT LEFT UP
; DOWN, bits 4-7 A B SELECT START. The log is cleared before the first frame.

SECTION "Header", ROM0[$100]
    nop
    jp Main
    ds $150 - @, 0  ; rgbfix fills in the logo and checksums

SECTION "Main", ROM0[$150]
Main:
    di
    ld sp, $DFFF
    xor a
    ld [wCount], a
    ld hl, wLog
    ld b, 0         ; 256 bytes
.clear:
    ld [hl+], a
    dec b
    jr nz, .clear
.frame:
    ldh a, [$FF44]  ; rLY
    cp 144
    jr nz, .frame
    ld a, $20       ; P14 low: the direction row
    ldh [$FF00], a
    ldh a, [$FF00]
    ldh a, [$FF00]  ; read twice to let the lines settle
    cpl
    and $0F
    ld c, a
    ld a, $10       ; P15 low: the button row
    ldh [$FF00], a
    ldh a, [$FF00]
    ldh a, [$FF00]
    cpl
    and $0F
    swap a
    or c
    ld c, a
    ld a, $30       ; deselect both rows
    ldh [$FF00], a
    ld a, [wCount]
    ld l, a
    ld h, HIGH(wLog)
    ld [hl], c
    inc a
    ld [wCount], a
.leave:
    ldh a, [$FF44]
    cp 144
    jr z, .leave
    jr .frame

SECTION "Count", WRAM0[$C000]
wCount: ds 1

SECTION "Log", WRAM0[$C100]
wLog: ds 256
