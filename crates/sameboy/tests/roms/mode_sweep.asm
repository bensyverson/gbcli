; Mode-sweep test ROM for gbcli. Written for this project; same license as gbcli.
;
; Sweeps a VRAM write and an SCX write across line 50, one M-cycle later each
; frame, so the tests can compare STAT at the write instruction's start with
; the mode the write lands in (crates/sameboy/tests/it/mode_sweep.rs).
; Frame k (k = 0..127): wake at line 50's start, run k NOPs from a sled, then
;   WriteVram: ld [de], a   ; de = $8000 + k, a = $FF
;   WriteScx:  ldh [rSCX], a ; a = 0
; VRAM $8000-$80FF is cleared with the LCD off, so a dropped write reads 0.
; No objects, so SameBoy batches mode 3 by its exact length.

DEF rLCDC EQU $FF40
DEF rSTAT EQU $FF41
DEF rSCX EQU $FF43
DEF rLY EQU $FF44
DEF rLYC EQU $FF45
DEF rIF EQU $FF0F
DEF rIE EQU $FFFF
DEF hK EQU $FF80

SECTION "Header", ROM0[$100]
    nop
    jp Main
    ds $150 - @, 0

SECTION "Main", ROM0[$150]
Main:
    di
    ld sp, $DFFF
    xor a
    ldh [rIE], a
    ldh [hK], a
    ldh a, [rLCDC]
    bit 7, a
    jr z, .off
.vblank:
    ldh a, [rLY]
    cp 144
    jr c, .vblank
    xor a
    ldh [rLCDC], a
.off:
    ld hl, $8000
    ld bc, $100
.clear:
    xor a
    ld [hl+], a
    dec bc
    ld a, b
    or c
    jr nz, .clear
    ; Hide every object.
    ld hl, $FE00
    ld b, 160
    xor a
.oam:
    ld [hl+], a
    dec b
    jr nz, .oam
    xor a
    ldh [rSCX], a
    ld a, $91
    ldh [rLCDC], a
Frame:
    ld a, 50
    call SleepToLine
    ldh a, [hK]
    ld e, a
    ld d, $80
    ld c, a
    ld b, 0
    ld hl, SledEnd
    ; hl = SledEnd - k
    ld a, l
    sub c
    ld l, a
    ld a, h
    sbc b
    ld h, a
    ld a, $FF
    jp hl
Sled:
    REPT 128
        nop
    ENDR
SledEnd:
WriteVram:
    ld [de], a
    xor a
WriteScx:
    ldh [rSCX], a
    ldh a, [hK]
    inc a
    and $7F
    ldh [hK], a
    jp nz, Frame
Done:
    jr Done

SleepToLine:
    ldh [rLYC], a
    ld a, $40
    ldh [rSTAT], a
    ld a, $02
    ldh [rIE], a
    xor a
    ldh [rIF], a
    halt
    nop
    ret
