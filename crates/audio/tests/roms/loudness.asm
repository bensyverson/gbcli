; A loud channel and a quiet one whose DAC keeps switching, for the loudness
; figures in `run --audio-summary`. Written for this project; same license as
; gbcli.
;
; A DMG DAC outputs its full positive level for a digital 0, so switching a
; DAC on or off steps the output by nearly full scale whatever the volume: a
; pop. Square 2 here is quiet (volume 1) but its DAC is on 2 frames in 3,
; so its WAV's peaks and its DC-inclusive RMS outdo square 1's, which is loud
; (volume 6, steady) and never switches. Most of the windows square 2 sounds
; in hold a pop, so a loudness figure that ranks them right has left the pops
; out, not outvoted them.
;
;   frame 0   square 1 A4 (period 1750, 50% duty, volume 6, no envelope),
;             to the end
;   every 3   square 2 E5 (period 1849, 50% duty, volume 1, no envelope):
;   frames    DAC on and trigger, then 2 frames later DAC off (NR22 = 0)
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

    ; square 1: loud and steady
    xor a
    ldh [$FF10], a    ; NR10: no sweep
    ld a, $80
    ldh [$FF11], a    ; NR11: duty 50%, length 0
    ld a, $60
    ldh [$FF12], a    ; NR12: volume 6, no envelope
    ld a, $D6
    ldh [$FF13], a    ; NR13: period low
    ld a, $86
    ldh [$FF14], a    ; NR14: trigger, period $6D6 = 1750 (A4)

    ld a, $80
    ldh [$FF16], a    ; NR21: duty 50%, length 0
    ld a, $39
    ldh [$FF18], a    ; NR23: period low ($739 = 1849, E5)

.cycle:
    ld a, $10
    ldh [$FF17], a    ; NR22: volume 1, no envelope: DAC on
    ld a, $87
    ldh [$FF19], a    ; NR24: trigger, period high
    ld b, 2
    call WaitFrames
    xor a
    ldh [$FF17], a    ; NR22: DAC off
    ld b, 1
    call WaitFrames
    jr .cycle

; Waits for B vblanks.
WaitFrames:
    halt
    dec b
    jr nz, WaitFrames
    ret
