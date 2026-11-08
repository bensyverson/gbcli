; A known tune for gbcli's audio views. Written for this project; same license as gbcli.
;
; Plays a few notes on every channel, frame-timed by the vblank interrupt, then
; turns the APU off and writes a register while it is off. The write log test
; (crates/audio/tests/it/log.rs) checks every write this ROM makes, decoded.
;
;   frame 0   square 1 A4 (period 1750, 50% duty, volume 15 fading, step 3)
;             square 2 E4 (period 1650, 25% duty, volume 10 steady, length 32)
;             wave     G3 (period 1714, full level, a triangle in wave RAM)
;   frame 15  square 1 C5 (period 1798) with an upward sweep (step 1, shift 7)
;             noise    (volume 10 fading, step 1; shift 5, 15-bit, divisor code 2)
;   frame 30  wave DAC off, APU off, then NR12 written while off (ignored)
;             and NR11 written while off (a DMG takes its length half)
;
; Every register is written with LDH from A, so each write is one instruction.

SECTION "VBlank", ROM0[$40]
    reti

SECTION "Header", ROM0[$100]
    nop
    jp Main
    ds $150 - @, 0  ; header; rgbfix fills in the logo and checksums

SECTION "Main", ROM0[$150]
Main:
    di
    ld sp, $FFFE
    ld a, $01
    ldh [$FFFF], a    ; IE: vblank
    ld a, $91
    ldh [$FF40], a    ; LCDC: on, BG on
    xor a
    ldh [$FF0F], a    ; IF: clear
    ei

    ld a, $80
    ldh [$FF26], a    ; NR52: APU on
    ld a, $77
    ldh [$FF24], a    ; NR50: both sides at volume 7
    ld a, $FF
    ldh [$FF25], a    ; NR51: every channel to both sides

    ld hl, $FF30      ; wave RAM: a triangle, 0 up to F and back down
    ld de, Wave
    ld c, 16
.wave:
    ld a, [de]
    ld [hl+], a
    inc de
    dec c
    jr nz, .wave

    ; frame 0
    ld a, $00
    ldh [$FF10], a    ; NR10: no sweep
    ld a, $80
    ldh [$FF11], a    ; NR11: duty 50%, length 0
    ld a, $F3
    ldh [$FF12], a    ; NR12: volume 15, down, step 3
    ld a, $D6
    ldh [$FF13], a    ; NR13: period low
    ld a, $86
    ldh [$FF14], a    ; NR14: trigger, period $6D6 = 1750 (A4)

    ld a, $60
    ldh [$FF16], a    ; NR21: duty 25%, length 32
    ld a, $A0
    ldh [$FF17], a    ; NR22: volume 10, no envelope
    ld a, $72
    ldh [$FF18], a    ; NR23: period low
    ld a, $C6
    ldh [$FF19], a    ; NR24: trigger, length on, period $672 = 1650 (E4)

    ld a, $80
    ldh [$FF1A], a    ; NR30: wave DAC on
    ld a, $00
    ldh [$FF1B], a    ; NR31: length 0
    ld a, $20
    ldh [$FF1C], a    ; NR32: full level
    ld a, $B2
    ldh [$FF1D], a    ; NR33: period low
    ld a, $86
    ldh [$FF1E], a    ; NR34: trigger, period $6B2 = 1714 (G3)

    ld b, 15
    call WaitFrames

    ; frame 15
    ld a, $17
    ldh [$FF10], a    ; NR10: sweep step 1, up, shift 7
    ld a, $06
    ldh [$FF13], a    ; NR13: period low
    ld a, $87
    ldh [$FF14], a    ; NR14: trigger, period $706 = 1798 (C5)

    ld a, $A1
    ldh [$FF21], a    ; NR42: volume 10, down, step 1
    ld a, $52
    ldh [$FF22], a    ; NR43: shift 5, 15-bit, divisor code 2
    ld a, $80
    ldh [$FF23], a    ; NR44: trigger

    ld b, 15
    call WaitFrames

    ; frame 30
    xor a
    ldh [$FF1A], a    ; NR30: wave DAC off
    ldh [$FF26], a    ; NR52: APU off
    ld a, $F0
    ldh [$FF12], a    ; NR12 while off: ignored
    ld a, $BF
    ldh [$FF11], a    ; NR11 while off: a DMG takes the length (63), not the duty

.end:
    halt
    jr .end

; Waits for B vblanks.
WaitFrames:
    halt
    dec b
    jr nz, WaitFrames
    ret

Wave:
    db $01, $23, $45, $67, $89, $AB, $CD, $EF
    db $FE, $DC, $BA, $98, $76, $54, $32, $10
