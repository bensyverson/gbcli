; Sound effects on channel 1, which the music leaves silent.
;
; An effect is a list of steps, each a note held for some frames: the frame
; count, then the five channel 1 registers (NR10 sweep, NR11 duty, NR12
; envelope, NR13 and NR14 the period, NR14 with the trigger bit). A zero
; frame count ends it and silences the channel.

include "hardware.inc"
include "defs.inc"

; One step: frames, sweep, duty (0-3), envelope, period (0-2047).
MACRO STEP
	db \1, \2, (\3) << 6, \4, LOW(\5), HIGH(\5) | $80
ENDM

; Periods: 2048 - 131072 / Hz.
DEF P_A3  EQU 1155              ; 220 Hz
DEF P_D4  EQU 1379
DEF P_C5  EQU 1797
DEF P_E5  EQU 1849
DEF P_G5  EQU 1881
DEF P_A5  EQU 1899
DEF P_C6  EQU 1923
DEF P_E6  EQU 1949
DEF P_G6  EQU 1964
DEF P_C7  EQU 1985

SECTION "Sound effects", ROM0

; Starts effect A (SFX_*), over whatever channel 1 was playing.
SfxPlay::
	add a
	ld e, a
	ld d, 0
	ld hl, SfxTable
	add hl, de
	ld a, [hli]
	ld [wSfxPtr], a
	ld a, [hl]
	ld [wSfxPtr + 1], a
	ld a, 1
	ld [wSfxLeft], a            ; the first step starts on the next tick
	ret

; One frame of the effect, from the VBlank handler.
SfxTick::
	ld a, [wSfxLeft]
	and a
	ret z
	dec a
	ld [wSfxLeft], a
	ret nz
	; The next step.
	ld hl, wSfxPtr
	ld a, [hli]
	ld h, [hl]
	ld l, a
	ld a, [hli]
	and a
	jr z, .end
	ld [wSfxLeft], a
	ld a, [hli]
	ldh [rNR10], a
	ld a, [hli]
	ldh [rNR11], a
	ld a, [hli]
	ldh [rNR12], a
	ld a, [hli]
	ldh [rNR13], a
	ld a, [hli]
	ldh [rNR14], a
	ld a, l
	ld [wSfxPtr], a
	ld a, h
	ld [wSfxPtr + 1], a
	ret
.end
	xor a
	ldh [rNR12], a              ; DAC off: the channel stops
	ret

SfxTable:
	dw SfxKey, SfxLocked, SfxWin, SfxLose, SfxStart

SfxKey:                         ; a bright rising chime
	STEP 4, $00, 2, $F2, P_C6
	STEP 4, $00, 2, $F2, P_E6
	STEP 4, $00, 2, $F2, P_G6
	STEP 10, $00, 2, $F3, P_C7
	db 0
SfxLocked:                      ; a low double knock
	STEP 6, $00, 3, $C1, P_A3
	STEP 4, $00, 3, $00, P_A3
	STEP 8, $00, 3, $C1, P_A3
	db 0
SfxWin:                         ; a short fanfare
	STEP 8, $00, 2, $F3, P_C5
	STEP 8, $00, 2, $F3, P_E5
	STEP 8, $00, 2, $F3, P_G5
	STEP 24, $00, 2, $F4, P_C6
	db 0
SfxLose:                        ; a long fall: the sweep lowers the pitch
	STEP 60, $7F, 1, $F7, P_A5
	db 0
SfxStart:                       ; a blip
	STEP 3, $00, 2, $C1, P_G6
	STEP 6, $00, 2, $C2, P_C7
	db 0

SECTION "Sound effect variables", WRAM0
wSfxPtr: dw                     ; the next step
wSfxLeft: db                    ; frames left in this step (0: none playing)
