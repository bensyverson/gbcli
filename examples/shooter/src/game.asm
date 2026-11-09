; The game's states: title, play, and the two results (stage clear, game
; over), plus the joypad, the score and the lives.
;
; Screens change with the LCD off (LcdOff waits for VBlank first), so
; their maps can be drawn freely; play then runs at one frame per VBlank.

include "hardware.inc"
include "charmap.inc"

DEF START_LIVES EQU 3
DEF END_DELAY EQU 90                  ; frames from the last blow to the result
DEF RESULT_HOLD EQU 60                ; frames before a result takes START

SECTION "Game state", WRAM0

wState:: db
wStateT:: db                          ; frames in this state, stopping at 255
wScore:: ds 3                         ; BCD, most significant byte first
wLives:: db
wEndT: db                             ; counts down to the result screen; 0 = off
wDead:: db                            ; nonzero once the last life is gone

SECTION "Pad", HRAM

hPad:: db                             ; held: PAD_* bits
hPadNew:: db                          ; pressed this frame

SECTION "Game code", ROM0

; From start-up, with the LCD off.
GameInit::
	xor a
	ld [wScore], a
	ld [wScore + 1], a
	ld [wScore + 2], a
	ldh [hPad], a
	ldh [hPadNew], a
	ld [wHudDirty], a
	call GfxInit
	jp TitleDraw

; Once a frame, from the main loop.
GameUpdate::
	ld hl, wStateT
	inc [hl]
	jr nz, .counted
	dec [hl]
.counted
	ld a, [wState]
	cp STATE_PLAY
	jr z, PlayUpdate
	and a
	jr z, TitleUpdate
	cp STATE_TEST
	jr z, TestUpdate
	; A result screen.
	ld a, [wStateT]
	cp RESULT_HOLD
	ret c
	ldh a, [hPadNew]
	and PAD_START
	ret z
	call LcdOff
	call TitleDraw
	jp LcdOn

TitleUpdate:
	ldh a, [hPadNew]
	bit B_PAD_SELECT, a
	jr nz, .test
	and PAD_START
	ret z
	call LcdOff
	call PlayDraw
	jp LcdOn
.test
	call LcdOff
	call SkyLoad
	call HudScreen
	call SceneInit
	ld a, STATE_TEST
	call SetState
	jp LcdOn

; The sprite test runs until START.
TestUpdate:
	call SceneUpdate
	ldh a, [hPadNew]
	and PAD_START
	ret z
	call LcdOff
	call TitleDraw
	jp LcdOn

PlayUpdate:
	call SfxUpdate
	ld a, [wDead]
	and a
	call z, PlayerUpdate
	call EnemiesUpdate
	call BulletsUpdate
	call EnemiesCollide
	ld a, [wPlayerHit]
	and a
	call nz, PlayerHurt
	; The stage clears once the boss is gone.
	ld a, [wEndT]
	and a
	jr nz, .ending
	ld a, [wBossState]
	cp BOSS_DEAD
	ret nz
	ld a, END_DELAY
	ld [wEndT], a
	ret
.ending
	dec a
	ld [wEndT], a
	ret nz
	call LcdOff
	ld a, [wDead]
	and a
	ld a, STATE_OVER
	jr nz, .result
	ld a, STATE_WIN
.result
	call ResultDraw
	jp LcdOn

; A bullet or an enemy reached the ship: a life goes, and every bullet on
; screen with it, as a mercy.
PlayerHurt:
	xor a
	ld [wPlayerHit], a
	call BulletsClear
	call SfxBoom
	ld hl, wLives
	dec [hl]
	call HudLives
	ld a, [wLives]
	and a
	jr z, .last
	ld a, INVULN_FRAMES
	ld [wInvuln], a
	ret
.last
	ld a, 1
	ld [wDead], a
	ld a, 255
	ld [wInvuln], a                   ; nothing more can hit it
	ld a, END_DELAY
	ld [wEndT], a
	ret

; Adds BCD A (ten-thousands), D (hundreds), E (units) to the score.
AddScore::
	ld b, a
	ld hl, wScore + 2
	ld a, [hl]
	add e
	daa
	ld [hld], a
	ld a, [hl]
	adc d
	daa
	ld [hld], a
	ld a, [hl]
	adc b
	daa
	ld [hl], a
	jp HudScore

; ---------------------------------------------------------------------------
; Screens, each drawn with the LCD off.

TitleDraw:
	call TextScreen
	call TitleLoad
	call MusicTitle
	ld b, 15
	ld c, 4
	ld de, PressStartText
	call TextAt
	ld a, STATE_TITLE
	jr SetState

PlayDraw:
	call SkyLoad
	call HudScreen
	call SfxInit
	call MusicStage
	call PlayerInit
	call EnemiesInit
	call BulletsClear
	xor a
	ld [wScore], a
	ld [wScore + 1], a
	ld [wScore + 2], a
	ld [wPlayerHit], a
	ld [wEndT], a
	ld [wDead], a
	ld a, START_LIVES
	ld [wLives], a
	call HudScore
	call HudLives
	call HudBossOff
	ld a, STATE_PLAY
	jr SetState

; A = STATE_WIN or STATE_OVER.
ResultDraw:
	push af
	call TextScreen
	pop af
	push af
	ld de, ClearText
	ld c, 4
	cp STATE_WIN
	jr z, .headline
	ld de, OverText
	ld c, 5
.headline
	ld b, 6
	call TextAt
	ld b, 9
	ld c, 4
	ld de, ScoreText
	call TextAt                       ; leaves HL after "SCORE "
	call ScoreDigits
	ld b, 13
	ld c, 4
	ld de, PressStartText
	call TextAt
	pop af
SetState:
	ld [wState], a
	xor a
	ld [wStateT], a
	ret

PressStartText: db "PRESS START", TEXT_END
ClearText: db "STAGE CLEAR", TEXT_END
OverText: db "GAME OVER", TEXT_END
ScoreText: db "SCORE ", TEXT_END

; ---------------------------------------------------------------------------

; Turns the LCD off at the start of VBlank, with interrupts off.
LcdOff::
	di
.wait
	ldh a, [rLY]
	cp LY_VBLANK
	jr nz, .wait
	ldh a, [rLCDC]
	res B_LCDC_ENABLE, a
	ldh [rLCDC], a
	ret

; Turns the LCD back on and interrupts with it, dropping requests that
; came while it was off.
LcdOn::
	ldh a, [rLCDC]
	set B_LCDC_ENABLE, a
	ldh [rLCDC], a
	xor a
	ldh [rIF], a
	ei
	ret

; The pad into hPad (held) and hPadNew (pressed since last frame).
ReadPad::
	ld a, JOYP_GET_CTRL_PAD
	ldh [rJOYP], a
	ldh a, [rJOYP]
	ldh a, [rJOYP]
	cpl
	and $0F
	swap a
	ld b, a
	ld a, JOYP_GET_BUTTONS
	ldh [rJOYP], a
	ldh a, [rJOYP]
	ldh a, [rJOYP]
	ldh a, [rJOYP]
	ldh a, [rJOYP]
	cpl
	and $0F
	or b
	ld b, a
	ld a, JOYP_GET_NONE
	ldh [rJOYP], a
	ldh a, [hPad]
	xor b
	and b
	ldh [hPadNew], a
	ld a, b
	ldh [hPad], a
	ret
