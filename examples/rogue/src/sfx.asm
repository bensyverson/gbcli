; Sound effects on channel 2, which both songs leave empty and the driver
; has muted. An effect is a list of steps, each held for some frames; the
; VBlank handler plays them. A new effect replaces the one playing.

include "hardware.inc"
include "game.inc"

; A channel-2 period for a frequency in Hz.
DEF PERIOD EQUS "2048 - 131072 / "

; One step: frames, NR21 (duty), NR22 (envelope), period. 0 frames ends.
MACRO step
	db \1, \2, \3
	dw \4
ENDM
DEF D12 EQU $00
DEF D25 EQU $40
DEF D50 EQU $80
DEF D75 EQU $C0

SECTION "Sound effects", ROM0

SfxTable:
	dw SfxStep, SfxHit, SfxHurt, SfxPickup, SfxStairs

SfxStep:
	step 3, D12, $31, {PERIOD} 180
	db 0
SfxHit:
	step 2, D50, $A1, {PERIOD} 660
	step 4, D50, $81, {PERIOD} 330
	db 0
SfxHurt:
	step 3, D75, $F1, {PERIOD} 150
	step 5, D75, $C1, {PERIOD} 110
	db 0
SfxPickup:
	step 3, D50, $B1, {PERIOD} 523
	step 3, D50, $B1, {PERIOD} 659
	step 3, D50, $B1, {PERIOD} 784
	step 6, D50, $B2, {PERIOD} 1047
	db 0
SfxStairs:
	step 5, D25, $B2, {PERIOD} 392
	step 5, D25, $A2, {PERIOD} 330
	step 5, D25, $92, {PERIOD} 262
	step 8, D25, $83, {PERIOD} 196
	db 0

; Starts effect A. Keeps BC and DE.
SfxPlay::
	push hl
	add a
	add LOW(SfxTable)
	ld l, a
	adc HIGH(SfxTable)
	sub l
	ld h, a
	ld a, [hli]
	ld [wSfxPtr], a
	ld a, [hl]
	ld [wSfxPtr + 1], a
	ld a, 1
	ld [wSfxWait], a            ; the first step plays at the next VBlank
	pop hl
	ret

; Called by the VBlank handler: plays the next step when the current one ends.
SfxVBlank::
	ld a, [wSfxWait]
	and a
	ret z
	dec a
	ld [wSfxWait], a
	ret nz
	ld a, [wSfxPtr]
	ld l, a
	ld a, [wSfxPtr + 1]
	ld h, a
	ld a, [hli]
	and a
	jr z, .end
	ld [wSfxWait], a
	ld a, [hli]
	ldh [rNR21], a
	ld a, [hli]
	ldh [rNR22], a
	ld a, [hli]
	ldh [rNR23], a
	ld a, [hli]
	or AUD2HIGH_RESTART
	ldh [rNR24], a
	ld a, l
	ld [wSfxPtr], a
	ld a, h
	ld [wSfxPtr + 1], a
	ret
.end
	xor a
	ldh [rNR22], a              ; silence: volume 0 and DAC off
	ret

SECTION "Sound effect variables", WRAM0
wSfxPtr: dw
wSfxWait: db                    ; frames left in this step; 0: idle
