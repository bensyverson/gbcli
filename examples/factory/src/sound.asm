; Sound effects, on channel 1, which the music never plays (its orders
; give channel 1 an empty pattern, and MusicInit mutes it in the driver).
; Effects play at volume 15 against the music's 7 and below, so they sit
; clearly on top.
;
; An effect is a list of steps; each step sets channel 1's sweep, duty,
; envelope and pitch, retriggers it, and lasts some frames. SfxPlay only
; asks (one byte, so it is safe from any code); the VBlank handler starts
; the effect and steps it (SfxVBlank). A new effect replaces one playing.

include "hardware.inc"

; Effect numbers for SfxPlay.
DEF SFX_GRAB     EQU 1
DEF SFX_ASSEMBLE EQU 2
DEF SFX_SCORE    EQU 3
DEF SFX_BUZZ     EQU 4
DEF SFX_TURN     EQU 5
DEF SFX_WARN     EQU 6              ; the clock turns red: time is running out
DEF SFX_TICK     EQU 7              ; each second after that
DEF SFX_TIMEUP   EQU 8              ; the round is over
EXPORT SFX_GRAB, SFX_ASSEMBLE, SFX_SCORE, SFX_BUZZ, SFX_TURN, SFX_WARN, SFX_TICK, SFX_TIMEUP

; Channel 1 pitches: 2048 - 131072 / Hz.
DEF A3 EQU 1452
DEF C5 EQU 1798
DEF E5 EQU 1849
DEF G5 EQU 1881
DEF A5 EQU 1899
DEF C6 EQU 1923
DEF E6 EQU 1949
DEF G6 EQU 1964
DEF C7 EQU 1985

; NR11's duty bits.
DEF DUTY_12 EQU %00 << 6
DEF DUTY_25 EQU %01 << 6
DEF DUTY_50 EQU %10 << 6

; One step: frames, NR10 (sweep), NR11 (duty), NR12 (envelope), pitch.
MACRO step
	db \1, \2, \3, \4
	dw \5
ENDM
MACRO end_effect
	db 0
ENDM

SECTION "Sound", ROM0

; Sound on, both speakers, full volume. Call once at start-up, before
; MusicInit.
SoundInit::
	ld a, AUDENA_ON
	ldh [rNR52], a
	ld a, $FF
	ldh [rNR51], a
	ld a, $77
	ldh [rNR50], a
	xor a
	ld [wSfxRequest], a
	ld [wSfxLeft], a
	ret

; Asks for effect a (SFX_*); it starts at the next VBlank.
SfxPlay::
	ld [wSfxRequest], a
	ret

; The buzz for a refused press.
SoundBuzz::
	ld a, SFX_BUZZ
	jr SfxPlay

; Once a frame, from the VBlank handler. Keeps bc and de.
SfxVBlank::
	ld a, [wSfxRequest]
	and a
	jr z, .playing
	; A new effect: point at its first step.
	ld l, a
	xor a
	ld [wSfxRequest], a
	ld a, l
	add a
	add LOW(SfxTable - 2)
	ld l, a
	adc HIGH(SfxTable - 2)
	sub l
	ld h, a
	ld a, [hli]
	ld h, [hl]
	ld l, a
	jr .step
.playing
	ld a, [wSfxLeft]
	and a
	ret z
	dec a
	ld [wSfxLeft], a
	ret nz
	ld a, [wSfxStep]
	ld l, a
	ld a, [wSfxStep + 1]
	ld h, a
.step
	ld a, [hli]
	ld [wSfxLeft], a
	and a
	jr z, .silence
	ld a, [hli]
	ldh [rNR10], a
	ld a, [hli]
	ldh [rNR11], a
	ld a, [hli]
	ldh [rNR12], a
	ld a, [hli]
	ldh [rNR13], a
	ld a, [hli]
	or AUD1HIGH_RESTART
	ldh [rNR14], a
	ld a, l
	ld [wSfxStep], a
	ld a, h
	ld [wSfxStep + 1], a
	ret
.silence
	xor a
	ldh [rNR10], a
	ldh [rNR12], a              ; DAC off: channel 1 stops
	ret

SfxTable:
	dw SfxGrab, SfxAssemble, SfxScore, SfxBuzz, SfxTurn, SfxWarn, SfxTick, SfxTimeUp

; A bright pluck that slides up.
SfxGrab:
	step 6, $15, DUTY_50, $F1, C6
	end_effect
; Three notes up: the halves click together.
SfxAssemble:
	step 3, $00, DUTY_50, $F1, C5
	step 3, $00, DUTY_50, $F1, E5
	step 6, $00, DUTY_50, $F2, G5
	end_effect
; A little fanfare: the toy goes off down the belt.
SfxScore:
	step 4, $00, DUTY_25, $F1, G5
	step 4, $00, DUTY_25, $F1, C6
	step 4, $00, DUTY_25, $F1, E6
	step 12, $00, DUTY_25, $F3, G6
	end_effect
; A low, rough note that sags.
SfxBuzz:
	step 10, $1F, DUTY_12, $F2, A3
	end_effect
; A soft swish as Elle turns.
SfxTurn:
	step 4, $27, DUTY_12, $C1, E5
	end_effect
; The clock goes red: two quick high-low pairs.
SfxWarn:
	step 5, $00, DUTY_50, $F1, C7
	step 5, $00, DUTY_50, $F1, G6
	step 5, $00, DUTY_50, $F1, C7
	step 8, $00, DUTY_50, $F2, G6
	end_effect
; Each second in the red: a short tick.
SfxTick:
	step 3, $00, DUTY_50, $C1, E6
	end_effect
; Time's up: a bell that rings down.
SfxTimeUp:
	step 6, $00, DUTY_50, $F2, C7
	step 6, $00, DUTY_50, $F2, G6
	step 6, $00, DUTY_50, $F2, E6
	step 24, $00, DUTY_50, $F4, C6
	end_effect

SECTION "Sound variables", WRAM0
wSfxRequest: db                    ; SFX_* to start at the next VBlank, or 0
wSfxLeft: db                       ; frames left in the step playing, or 0
wSfxStep: dw                       ; the next step
