; Hit and miss sounds, on channel 2 alone. Both songs leave channel 2
; silent and MusicPlay mutes it in the driver, so an effect never cuts a
; note of the music and the music never cuts an effect, whenever a press
; lands.

include "hardware.inc"
include "game.inc"

SECTION "Sound effects", ROM0

; Plays the sound for judgment a (JUDGE_PERFECT, _GOOD or _MISS). Keeps BC.
PlaySfx::
	dec a                       ; JUDGE_PERFECT is 1
	cp 3
	ret nc
	ld e, a
	add a
	add a                       ; 4 bytes an effect
	add LOW(SfxTable)
	ld l, a
	adc HIGH(SfxTable)
	sub l
	ld h, a
	ld a, [hli]
	ldh [rNR21], a              ; duty, length
	ld a, [hli]
	ldh [rNR22], a              ; envelope
	ld a, [hli]
	ldh [rNR23], a              ; period, low byte
	ld a, [hl]
	ldh [rNR24], a              ; trigger, length off, period high bits
	ret

; Each: NR21, NR22, NR23, NR24. Period p plays 131072 / (2048 - p) Hz.
MACRO sfx
	db \1, \2, LOW(\3), $80 | HIGH(\3)
ENDM
SfxTable:
	sfx $80, $A1, 1985          ; perfect: a bright tap, 2080 Hz, 50% duty
	sfx $40, $81, 1899          ; good: a softer tap, 880 Hz, 25% duty
	sfx $00, $C3, 1046          ; miss: a dull thud, 131 Hz, 12.5% duty
