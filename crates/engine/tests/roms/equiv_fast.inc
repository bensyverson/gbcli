; Equivalence-check test program, the optimized rewrite; see equiv.inc.
; Written for this project; same license as gbcli.
;
; equiv_fast.asm and equiv_broken.asm set RIGHT_EDGE, the column the block may
; not move right from: 19 is correct, 20 is the off-by-one. Against equiv_a:
; JOYP read twice per row, `inc [hl]` for the frame count, the map row by
; shifts, the routines in another order, and the variables in another order
; at another address, so every routine and variable moves.

INCLUDE "equiv.inc"

SECTION "Code", ROM0
VBlank:
    push af
    push hl
    ld hl, wOldAddr
    ld a, [hl+]
    ld h, [hl]
    ld l, a
    ld [hl], 0
    ld a, [wY]      ; MAP + wY * 32 + wX
    ld l, a
    ld h, 0
    add hl, hl
    add hl, hl
    add hl, hl
    add hl, hl
    add hl, hl
    ld a, [wX]
    add a, l
    ld l, a
    ld a, h
    adc a, HIGH(MAP)
    ld h, a
    ld [hl], 1
    ld a, l
    ld [wOldAddr], a
    ld a, h
    ld [wOldAddr + 1], a
    pop hl
    pop af
    reti

NoteTable:
    db $06, $21, $39, $44, $59, $6B, $7B, $83, $90, $9D
    db $A2, $AC, $B6, $BE, $C1, $C8, $CE, $D1, $D6, $DB
    db $DB          ; the broken build's column 20 reads this

Beep:
    ld a, [wPressed]
    bit 4, a
    ret z
    ld a, $80
    ldh [rNR21], a
    ld a, $F3
    ldh [rNR22], a
    ld a, [wX]
    add a, LOW(NoteTable)
    ld l, a
    adc a, HIGH(NoteTable)
    sub l
    ld h, a
    ld a, [hl]
    ldh [rNR23], a
    ld a, $87
    ldh [rNR24], a
    ret

MainLoop:
    halt
    nop
    ld a, $20
    ldh [rJOYP], a
    ldh a, [rJOYP]
    ldh a, [rJOYP]
    cpl
    and $0F
    ld b, a
    ld a, $10
    ldh [rJOYP], a
    ldh a, [rJOYP]
    ldh a, [rJOYP]
    cpl
    and $0F
    swap a
    or b
    ld b, a
    ld a, $30
    ldh [rJOYP], a
    ld hl, wHeld
    ld a, [hl]
    cpl
    and b
    ld [wPressed], a
    ld [hl], b
    ld hl, wFrame
    inc [hl]
    jr nz, .move
    inc hl
    inc [hl]
.move:
    ld hl, wX
    bit 0, b        ; RIGHT
    jr z, .left
    ld a, [hl]
    cp RIGHT_EDGE
    jr nc, .left
    inc [hl]
.left:
    bit 1, b        ; LEFT
    jr z, .down
    ld a, [hl]
    and a
    jr z, .down
    dec [hl]
.down:
    ld hl, wY
    bit 3, b        ; DOWN
    jr z, .up
    ld a, [hl]
    cp 17
    jr nc, .up
    inc [hl]
.up:
    bit 2, b        ; UP
    jr z, .beep
    ld a, [hl]
    and a
    jr z, .beep
    dec [hl]
.beep:
    call Beep
    jr MainLoop

SECTION "Variables", WRAM0[$C010]
wFrame:   ds 2
wOldAddr: ds 2
wHeld:    ds 1
wPressed: ds 1
wY:       ds 1
wX:       ds 1
