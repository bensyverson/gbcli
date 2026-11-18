; A busy frame like busy_wait.inc's, then a next frame that halts first (until
; LY=LYC=10, IME off) and only then turns the LCD off. The busy frame stays lag:
; the next frame halted before it turned the LCD off. Written for this
; project; same license as gbcli.

SECTION "VBlank vector", ROM0[$40]
VBlankVector:
    jp VBlank

SECTION "Header", ROM0[$100]
    nop
    jp Main
    ds $150 - @, 0

SECTION "Main", ROM0[$150]
Main:
    di
    ld sp, $DFFF
    xor a
    ld [wFrame], a
    ld a, $91
    ldh [$FF40], a
    xor a
    ldh [$FF0F], a
    inc a
    ldh [$FFFF], a
    ei
HaltLoop:
    halt
    nop
    ld a, [wFrame]
    cp 4
    jr nz, HaltLoop
    di
.leave:
    ldh a, [$FF44]
    cp 144
    jr nc, .leave
.wait:
    ldh a, [$FF44]
    cp 144
    jr c, .wait
    ; Next frame: halt first, until LY = 10 (STAT LYC interrupt, IME off).
    ld a, 10
    ldh [$FF45], a
    ld a, $40
    ldh [$FF41], a
    xor a
    ldh [$FF0F], a
    ld a, $02
    ldh [$FFFF], a
    halt
    nop
.woke:
    ; Then turn the LCD off.
    xor a
    ldh [$FF40], a
    ld bc, 5000
.busy:
    dec bc
    ld a, b
    or c
    jr nz, .busy
    ld a, $91
    ldh [$FF40], a
    xor a
    ldh [$FF41], a
    ld a, 1
    ldh [$FFFF], a
    ld a, 5
    ld [wFrame], a
    xor a
    ldh [$FF0F], a
    ei
    jr HaltLoop

VBlank:
    push af
    ld a, [wFrame]
    inc a
    ld [wFrame], a
    pop af
    reti

SECTION "WRAM", WRAM0[$C000]
wFrame:: db
