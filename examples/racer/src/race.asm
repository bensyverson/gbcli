; The race: a countdown, then the clock runs down while the car drives the
; lap; crossing the finish first wins, the clock reaching zero loses. START
; after either restarts. The HUD is one row of the window, along the bottom.

include "hardware.inc"
include "race.inc"
include "charmap.inc"

DEF TIME_LIMIT EQU $50          ; seconds, in BCD
DEF READY_FRAMES EQU 120
DEF HUD_WIDTH EQU 20

SECTION "Race", ROM0

InitRace::
	xor a
	ld [wState], a
	ld [wTimeSub], a
	ld a, READY_FRAMES
	ld [wReadyTimer], a
	ld a, TIME_LIMIT
	ld [wTimeSec], a
	jp FormatHud

UpdateRace::
	ld a, [wState]
	cp STATE_READY
	jr z, .ready
	cp STATE_RACE
	jr z, .race
	; Won or lost: START races again.
	ldh a, [hPadNew]
	and PAD_START
	jp nz, Title
	jr FormatHud
.ready
	ld hl, wReadyTimer
	dec [hl]
	jr nz, FormatHud
	ld a, STATE_RACE
	ld [wState], a
	jr FormatHud
.race
	ld a, [wFinished]
	and a
	jr z, .clock
	ld a, STATE_WON
	ld [wState], a
	jr FormatHud
.clock
	; One frame off the clock: sixtieths in wTimeSub, BCD seconds.
	ld hl, wTimeSub
	ld a, [hl]
	and a
	jr z, .borrow
	dec [hl]
	jr FormatHud
.borrow
	ld a, [wTimeSec]
	and a
	jr z, .timeUp
	sub 1
	daa
	ld [wTimeSec], a
	ld [hl], 59
	jr FormatHud
.timeUp
	ld a, STATE_LOST
	ld [wState], a
	; fall through

; Writes this frame's HUD row into wHud; VBlank copies it to the window.
FormatHud:
	ld a, [wState]
	add a
	ld e, a
	ld d, 0
	ld hl, HudTemplates
	add hl, de
	ld a, [hli]
	ld h, [hl]
	ld l, a
	ld de, wHud
	ld b, HUD_WIDTH
.copy
	ld a, [hli]
	ld [de], a
	inc de
	dec b
	jr nz, .copy

	ld a, [wState]
	cp STATE_READY
	jr nz, .notReady
	; 3, 2, 1: one per 40 frames left.
	ld a, [wReadyTimer]
	ld b, FONT_DIGIT0
.count
	inc b
	sub 40
	jr nc, .count
	ld a, b
	cp FONT_DIGIT0 + 4
	jr c, .digit
	ld a, FONT_DIGIT0 + 3
.digit
	ld [wHud + 15], a
	ret
.notReady
	cp STATE_LOST
	ret z
	; RACE and WON show the seconds left at column 5 (RACE) or 8 (WON).
	ld hl, wHud + 5
	cp STATE_RACE
	jr z, .seconds
	ld hl, wHud + 8
.seconds
	ld a, [wTimeSec]
	call PutBcd
	ld a, [wState]
	cp STATE_RACE
	ret nz
	; Speed, as wSpeed * 2 in three digits at column 17.
	ld a, [wSpeed]
	add a
	ld hl, wHud + 17
	ld b, FONT_DIGIT0 - 1
.hundreds
	inc b
	sub 100
	jr nc, .hundreds
	add 100
	ld [hl], b
	inc hl
	ld b, FONT_DIGIT0 - 1
.tens
	inc b
	sub 10
	jr nc, .tens
	add 10 + FONT_DIGIT0
	ld [hl], b
	inc hl
	ld [hl], a
	ret

; Writes BCD A as two digits at HL.
PutBcd:
	ld b, a
	swap a
	and $0F
	add FONT_DIGIT0
	ld [hli], a
	ld a, b
	and $0F
	add FONT_DIGIT0
	ld [hl], a
	ret

; Copies wHud to the window's first row. VBlank only.
CopyHud::
	ld hl, TILEMAP1
	ld de, wHud
	ld b, HUD_WIDTH
.copy
	ld a, [de]
	ld [hli], a
	inc de
	dec b
	jr nz, .copy
	ret

HudTemplates:
	dw HudReady, HudRace, HudWon, HudLost
HudReady:
	db "    GET READY  3    "
HudRace:
	db "TIME 00      SPD 000"
HudWon:
	db "FINISH! 00 SEC LEFT "
HudLost:
	db "TIME UP! PRESS START"
	ASSERT HudLost + HUD_WIDTH == @ && HudRace + HUD_WIDTH == HudWon

SECTION "Race variables", WRAM0
wState:: db                     ; STATE_READY, STATE_RACE, STATE_WON or STATE_LOST
wReadyTimer:: db
wTimeSec:: db                   ; seconds left, BCD
wTimeSub: db                    ; sixtieths left in this second
wHud: ds HUD_WIDTH
