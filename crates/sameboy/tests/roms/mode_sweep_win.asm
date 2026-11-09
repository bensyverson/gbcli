; Mode-sweep test ROM for gbcli, window variant. Written for this project; same
; license as gbcli.
;
; As mode_sweep.asm, but every line shows the window: SCX = 7, WY = 0, WX = 7,
; window on, no objects and no HBlank interrupt, so SameBoy batches mode 3 by
; its 300-dot upper bound (mode3_batching_length) on a line with no objects.
; Frame k (k = 0..127): wake at line 50's start, run k NOPs from a sled, then
;   WriteVram: ld [de], a   ; de = $8000 + k, a = $FF
;   WriteScx:  ldh [rSCX], a ; a = 7
; VRAM $8000-$80FF is cleared with the LCD off, so a dropped write reads 0.

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
    ld a, 7
    ldh [rSCX], a
    xor a
    ldh [$FF4A], a
    ld a, 7
    ldh [$FF4B], a
    ld a, $B1
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
    ld a, 7
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
