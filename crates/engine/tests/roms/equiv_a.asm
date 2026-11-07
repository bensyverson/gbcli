; Equivalence-check test program, the original build; see equiv.inc.
; Written for this project; same license as gbcli.
;
; Plain and slow on purpose: JOYP read four times per row, a 16-bit add for
; the frame count, every variable reloaded, the map row found by adding 32
; wY times. So the optimized build writes the sound registers earlier in the
; frame, with the same values in the same order.

INCLUDE "equiv.inc"

SECTION "Code", ROM0
MainLoop:
    halt
    nop
    call ReadKeys
    call Tick
    call Move
    call Beep
    jr MainLoop

ReadKeys:
    ld a, $20       ; P14 low: the direction row
    ldh [rJOYP], a
    ldh a, [rJOYP]
    ldh a, [rJOYP]
    ldh a, [rJOYP]
    ldh a, [rJOYP]
    cpl
    and $0F
    ld b, a
    ld a, $10       ; P15 low: the button row
    ldh [rJOYP], a
    ldh a, [rJOYP]
    ldh a, [rJOYP]
    ldh a, [rJOYP]
    ldh a, [rJOYP]
    cpl
    and $0F
    swap a
    or b
    ld b, a
    ld a, $30
    ldh [rJOYP], a
    ld a, [wHeld]
    cpl
    and b
    ld [wPressed], a
    ld a, b
    ld [wHeld], a
    ret

Tick:
    ld a, [wFrame]
    add a, 1
    ld [wFrame], a
    ld a, [wFrame + 1]
    adc a, 0
    ld [wFrame + 1], a
    ret

Move:
    ld a, [wHeld]
    bit 0, a        ; RIGHT
    jr z, .left
    ld a, [wX]
    cp 19
    jr nc, .left
    inc a
    ld [wX], a
.left:
    ld a, [wHeld]
    bit 1, a        ; LEFT
    jr z, .down
    ld a, [wX]
    and a
    jr z, .down
    dec a
    ld [wX], a
.down:
    ld a, [wHeld]
    bit 3, a        ; DOWN
    jr z, .up
    ld a, [wY]
    cp 17
    jr nc, .up
    inc a
    ld [wY], a
.up:
    ld a, [wHeld]
    bit 2, a        ; UP
    ret z
    ld a, [wY]
    and a
    ret z
    dec a
    ld [wY], a
    ret

Beep:
    ld a, [wPressed]
    bit 4, a        ; A, newly pressed
    ret z
    ld a, $80       ; 50% duty
    ldh [rNR21], a
    ld a, $F3       ; full volume, fading
    ldh [rNR22], a
    ld a, [wX]
    ld e, a
    ld d, 0
    ld hl, NoteTable
    add hl, de
    ld a, [hl]
    ldh [rNR23], a
    ld a, $87       ; trigger, period bits 8-10 = 7
    ldh [rNR24], a
    ret

VBlank:
    push af
    push bc
    push de
    push hl
    ld a, [wOldAddr]
    ld l, a
    ld a, [wOldAddr + 1]
    ld h, a
    ld [hl], 0
    ld hl, MAP
    ld a, [wY]
    ld b, a
    ld de, 32
.row:
    ld a, b
    and a
    jr z, .column
    add hl, de
    dec b
    jr .row
.column:
    ld a, [wX]
    ld e, a
    ld d, 0
    add hl, de
    ld [hl], 1
    ld a, l
    ld [wOldAddr], a
    ld a, h
    ld [wOldAddr + 1], a
    pop hl
    pop de
    pop bc
    pop af
    reti

NoteTable:
    db $06, $21, $39, $44, $59, $6B, $7B, $83, $90, $9D
    db $A2, $AC, $B6, $BE, $C1, $C8, $CE, $D1, $D6, $DB

SECTION "Variables", WRAM0[$C000]
wX:       ds 1
wY:       ds 1
wFrame:   ds 2
wHeld:    ds 1
wPressed: ds 1
wOldAddr: ds 2
