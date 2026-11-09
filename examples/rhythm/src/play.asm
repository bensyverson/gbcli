; Playing the chart: judging presses against the driver's tick, scoring,
; and drawing the falling notes and the HUD.
;
; Timing: every decision reads wNow, the music driver's own tick count
; (src/music.asm), never a separate timer. A note sounds on the tick its
; chart entry names; a press is judged by how many ticks it is from it.

include "hardware.inc"
include "game.inc"

; Loads now (wNow) into bc.
MACRO load_now
	ld a, [wNow]
	ld c, a
	ld a, [wNow + 1]
	ld b, a
ENDM

; hl = table \1 + a. Clobbers a.
MACRO index_hl
	ld hl, \1
	add l
	ld l, a
	adc h
	sub l
	ld h, a
ENDM

SECTION "Play", ROM0

; Resets the play's state: no note judged, score 0, life full.
PlayInit::
	ld hl, STARTOF("Play state")
	ld bc, SIZEOF("Play state")
	call ClearMem
	ld hl, wJudged
	ld bc, 256
	call ClearMem
	ld a, LIFE_MAX
	ld [wLife], a
	jp UpdateHud

; One frame of play, after the VBlank handler.
PlayFrame::
	call SweepMisses
	ld a, [wPressed]
	ld b, a
	ld c, 0
.lanes
	ld a, c
	index_hl LaneMasks
	ld a, [hl]
	and b
	jr z, .nextLane
	push bc
	ld a, c
	call JudgeLane
	pop bc
.nextLane
	inc c
	ld a, c
	cp 4
	jr nz, .lanes
	call DrawNotes
	jp UpdateHud

; The button for each lane, left to right.
LaneMasks:
	db PAD_LEFT, PAD_RIGHT, PAD_B, PAD_A

; de = ChartTicks[a] (clock time). Clobbers hl.
GetTick:
	ld l, a
	ld h, 0
	add hl, hl
	ld de, ChartTicks
	add hl, de
	ld a, [hli]
	ld e, a
	ld d, [hl]
	ret

; Moves wHead past every note already judged, and judges a miss on each
; unjudged note more than GOOD_WIN ticks in the past.
SweepMisses:
	load_now
.loop
	ld a, [wHead]
	cp CHART_LEN
	ret nc
	ld l, a
	ld h, HIGH(wJudged)
	ld a, [hl]
	and a
	jr nz, .advance
	ld a, [wHead]
	call GetTick
	ld a, c                     ; hl = now - tick
	sub e
	ld l, a
	ld a, b
	sbc d
	ret c                       ; the note is still ahead
	and a
	jr nz, .miss
	ld a, l
	cp GOOD_WIN + 1
	ret c                       ; still within reach
.miss
	ld a, [wHead]
	ld l, a
	ld h, HIGH(wJudged)
	ld [hl], JUDGE_MISS
	ld a, JUDGE_MISS
	call ApplyJudge
.advance
	ld hl, wHead
	inc [hl]
	jr .loop

; Judges a press in lane a against the earliest unjudged note of that lane
; within GOOD_WIN ticks of now; a press with no note in reach does nothing.
JudgeLane:
	ld [wLane], a
	ld a, [wHead]
	ld [wScan], a
	load_now
.loop
	ld a, [wScan]
	cp CHART_LEN
	ret nc
	call GetTick
	ld a, e                     ; hl = tick - now
	sub c
	ld l, a
	ld a, d
	sbc b
	ld h, a
	bit 7, h
	jr nz, .inReach             ; past, and the sweep left it within reach
	and a
	ret nz                      ; far ahead: nothing later is in reach
	ld a, l
	cp GOOD_WIN + 1
	ret nc
.inReach
	ld a, l
	ld [wDiff], a
	ld a, [wScan]
	ld l, a
	ld h, HIGH(wJudged)
	ld a, [hl]
	and a
	jr nz, .next
	ld a, [wScan]
	index_hl ChartLanes
	ld a, [hl]
	ld hl, wLane
	cp [hl]
	jr nz, .next
	; delta = now - tick = -diff
	ld a, [wDiff]
	cpl
	inc a
	ld [wLastDelta], a
	bit 7, a
	jr z, .abs
	cpl
	inc a
.abs
	cp PERFECT_WIN + 1
	ld d, JUDGE_PERFECT
	jr c, .mark
	ld d, JUDGE_GOOD
.mark
	ld a, [wScan]
	ld l, a
	ld h, HIGH(wJudged)
	ld [hl], d
	ld a, [wLane]
	index_hl wBurst
	ld [hl], BURST_FRAMES
	ld a, d
	jp ApplyJudge
.next
	ld hl, wScan
	inc [hl]
	jr .loop

; Scores judgment a: counts it, moves the combo, the score and the life,
; and shows its word. Keeps BC.
ApplyJudge:
	ld [wLastJudge], a
	push af
	call PlaySfx
	pop af
	ld e, a
	ld a, JUDGE_SHOW
	ld [wJudgeTimer], a
	ld a, e
	cp JUDGE_MISS
	jr z, .miss
	; A hit: one more in the combo, and the best combo so far.
	ld hl, wCombo
	inc [hl]
	ld a, [wMaxCombo]
	cp [hl]
	jr nc, .comboKept
	ld a, [hl]
	ld [wMaxCombo], a
.comboKept
	ld a, e
	cp JUDGE_PERFECT
	jr nz, .good
	ld hl, wPerfect
	inc [hl]
	ld a, LIFE_PERFECT
	call GainLife
	ld a, $03                   ; 300, in BCD hundreds
	jr AddScoreHundreds
.good
	ld hl, wGood
	inc [hl]
	ld a, LIFE_GOOD
	call GainLife
	ld a, $01                   ; 100
	jr AddScoreHundreds
.miss
	ld hl, wMiss
	inc [hl]
	xor a
	ld [wCombo], a
	ld a, [wHead]               ; the note missed: the lesson's cost no life
	cp LESSON_NOTES
	ret c
	ld a, [wLife]
	sub LIFE_MISS
	jr nc, .alive
	xor a
.alive
	ld [wLife], a
	ret

; Adds a to the life, up to LIFE_MAX. Keeps BC and DE.
GainLife:
	ld hl, wLife
	add [hl]
	cp LIFE_MAX
	jr c, .store
	ld a, LIFE_MAX
.store
	ld [hl], a
	ret

; Adds a (BCD, in hundreds) to the score.
AddScoreHundreds:
	ld hl, wScore + 1
	add [hl]
	daa
	ld [hli], a
	ld a, [hl]
	adc 0
	daa
	ld [hl], a
	ret

; Writes a sprite to shadow OAM for every unjudged note from wHead that is
; due within LOOKAHEAD ticks, at HIT_Y less 2 px per tick still to go, and
; hides the rest of OAM.
DrawNotes:
	call DrawBursts
	ld a, [wHead]
	ld [wScan], a
	load_now
.loop
	ld a, [wOamPtr]
	cp OAM_SIZE
	ret nc                      ; OAM is full
	ld a, [wScan]
	cp CHART_LEN
	jr nc, .done
	call GetTick
	ld a, e                     ; hl = tick - now
	sub c
	ld l, a
	ld a, d
	sbc b
	ld h, a
	and a
	jr z, .ahead
	inc a
	jr nz, .done                ; far ahead
	jr .visible                 ; just past, not yet swept
.ahead
	ld a, l
	cp LOOKAHEAD + 1
	jr nc, .done
.visible
	ld a, l
	ld [wDiff], a
	ld a, [wScan]
	ld l, a
	ld h, HIGH(wJudged)
	ld a, [hl]
	and a
	jr nz, .next
	ld a, [wScan]
	index_hl ChartLanes
	ld a, [hl]
	ld [wNoteLane], a
	index_hl LaneX
	ld d, [hl]                  ; d = x
	ld a, [wDiff]
	REPT SPEED_SHIFT
	add a
	ENDR
	ld e, a
	ld a, HIT_Y + OAM_Y_OFS
	sub e
	ld e, a                     ; e = y
	ld a, [wOamPtr]
	ld l, a
	ld h, HIGH(wShadowOAM)
	ld a, e
	ld [hli], a
	ld a, d
	ld [hli], a
	ld a, TILE_LANTERN
	ld [hli], a
	ld a, [wNoteLane]           ; GBC: OBJ palette = lane; the DMG ignores it
	ld [hli], a
	ld a, l
	ld [wOamPtr], a
.next
	ld hl, wScan
	inc [hl]
	jr .loop
.done
	ld a, [wOamPtr]
	ld l, a
	ld h, HIGH(wShadowOAM)
	jp HideFrom

; Starts shadow OAM with a burst over each drum hit in the last
; BURST_FRAMES frames, and sets wOamPtr past them.
DrawBursts:
	ld hl, wShadowOAM
	ld de, wBurst
	ld c, 0                     ; lane
.lane
	ld a, [de]
	and a
	jr z, .next
	dec a
	ld [de], a
	ld a, HIT_Y + OAM_Y_OFS
	ld [hli], a
	push hl
	ld a, c
	index_hl LaneX
	ld a, [hl]
	pop hl
	ld [hli], a
	ld a, TILE_BURST
	ld [hli], a
	ld a, BURST_PALETTE
	ld [hli], a
.next
	inc de
	inc c
	ld a, c
	cp 4
	jr nz, .lane
	ld a, l
	ld [wOamPtr], a
	ret


; OAM x of each lane's notes.
LaneX::
	db 64 + OAM_X_OFS, 88 + OAM_X_OFS, 112 + OAM_X_OFS, 136 + OAM_X_OFS

; Clears shadow OAM from HL (in its page) to its end.
HideFrom::
	ld a, l
	cp OAM_SIZE
	ret nc
	xor a
	ld [hli], a
	jr HideFrom

; Fills the HUD buffer from the score, combo, judgment and life; the
; VBlank handler copies it to the map (CopyHud).
UpdateHud:
	ld hl, wHudScore
	ld de, wScore
	call PrintScore
	ld a, [wCombo]
	ld hl, wHudCombo
	call PrintByte
	; The judgment word, while its timer runs.
	ld hl, wJudgeTimer
	ld a, [hl]
	and a
	jr z, .noWord
	dec [hl]
	ld a, [wLastJudge]
	add a
	index_hl JudgeWords
	ld a, [hli]
	ld d, [hl]
	ld e, a
	jr .word
.noWord
	ld de, JudgeBlank
.word
	ld hl, wHudJudge
	ld b, HUD_JUDGE_LEN
.copyWord
	ld a, [de]
	ld [hli], a
	inc de
	dec b
	jr nz, .copyWord
	; The meter: cell k is full while life > k * LIFE_MAX / LIFE_CELLS.
	ld hl, wHudLife
	ld a, [wLife]
	ld c, a
	ld b, 0
.cell
	ld a, b
	cp c
	ld a, TILE_METER_FULL
	jr c, .put
	ld a, TILE_METER_EMPTY
.put
	ld [hli], a
	ld a, b
	add LIFE_MAX / LIFE_CELLS
	ld b, a
	cp LIFE_MAX
	jr c, .cell
	ret

JudgeWords:
	dw JudgeBlank, JudgePerfect, JudgeGood, JudgeMiss
JudgeBlank:   db "       "
JudgePerfect: db "PERFECT"
JudgeGood:    db "GOOD!  "
JudgeMiss:    db "MISS   "

; Copies the HUD buffer to the map. VBlank only.
CopyHud::
	ld hl, wHudScore
	ld de, HUD_SCORE
	ld b, HUD_SCORE_LEN
	call .copy
	ld de, HUD_COMBO
	ld b, HUD_COMBO_LEN
	call .copy
	ld de, HUD_JUDGE
	ld b, HUD_JUDGE_LEN
	call .copy
	ld de, HUD_LIFE
	ld b, LIFE_CELLS
.copy
	ld a, [hli]
	ld [de], a
	inc de
	dec b
	jr nz, .copy
	ret

SECTION "Play state", WRAM0

wHead:: db                      ; the first note not yet judged
wScan:: db
wLane:: db
wDiff:: db
wOamPtr:: db
wPerfect:: db
wGood:: db
wMiss:: db
wCombo:: db
wMaxCombo:: db
wLife:: db
wScore:: ds 3                   ; BCD, lowest byte first
wLastJudge:: db
wLastDelta:: db                 ; press tick - note tick, signed
wJudgeTimer:: db
wNoteLane:: db
wBurst:: ds 4                   ; frames left of each lane's burst
; The HUD as tiles, in CopyHud's order.
wHudScore:: ds HUD_SCORE_LEN
wHudCombo:: ds HUD_COMBO_LEN
wHudJudge:: ds HUD_JUDGE_LEN
wHudLife:: ds LIFE_CELLS

SECTION "Judged", WRAMX, ALIGN[8]

; One byte per chart note: JUDGE_NONE until it is judged.
wJudged:: ds 256
