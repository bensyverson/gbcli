; The results screen: the counts, the best combo, the score and a grade.

include "hardware.inc"
include "game.inc"

; Grade thresholds, from the chart's length. S: no miss and at least 90%
; perfect. Below that, by points (2 a perfect, 1 a good) out of 2 a note:
; A 90%, B 75%, C 60%, else D. A failed song is F.
DEF S_PERFECTS EQU (CHART_LEN * 9 + 9) / 10
DEF A_POINTS EQU (CHART_LEN * 2 * 90 + 99) / 100
DEF B_POINTS EQU (CHART_LEN * 2 * 75 + 99) / 100
DEF C_POINTS EQU (CHART_LEN * 2 * 60 + 99) / 100

SECTION "Results", ROM0

; Grades the play and shows the results screen; a = 1 if the song was
; cleared, 0 if it failed. Leaves the LCD on and wState at STATE_RESULTS.
ShowResults::
	ld [wCleared], a
	call Grade
	ld a, STATE_RESULTS
	ld [wState], a
	call LcdOff
IF DEF(GBCLI_GBC)
	ldh a, [hGbc]                ; the play's lane labels leave colors behind
	and a
	call nz, ClearAttrs
ENDC
	ld hl, TILEMAP0
	ld bc, $400
	call ClearMem
	ld hl, wShadowOAM
	call HideFrom
	call DrawBorder

	ld a, [wCleared]
	and a
	ld de, StrCleared
	jr nz, .title
	ld de, StrFailed
.title
	ld hl, TILEMAP0 + 2 * 32 + 6
	call PrintStr

	ld de, StrPerfect
	ld hl, TILEMAP0 + 5 * 32 + 3
	call PrintStr
	ld a, [wPerfect]
	call PrintByte
	ld de, StrGood
	ld hl, TILEMAP0 + 6 * 32 + 3
	call PrintStr
	ld a, [wGood]
	call PrintByte
	ld de, StrMiss
	ld hl, TILEMAP0 + 7 * 32 + 3
	call PrintStr
	ld a, [wMiss]
	call PrintByte
	ld de, StrCombo
	ld hl, TILEMAP0 + 9 * 32 + 3
	call PrintStr
	ld a, [wMaxCombo]
	call PrintByte
	ld de, StrScore
	ld hl, TILEMAP0 + 11 * 32 + 3
	call PrintStr
	ld de, wScore
	call PrintScore
	ld de, StrGrade
	ld hl, TILEMAP0 + 13 * 32 + 3
	call PrintStr
	push hl
	ld a, [wGrade]
	ld hl, GradeLetters
	add l
	ld l, a
	adc h
	sub l
	ld h, a
	ld a, [hl]
	pop hl
	ld [hl], a
	ld de, StrStart
	ld hl, TILEMAP0 + 16 * 32 + 4
	call PrintStr
	jp LcdOn

; A lattice round the screen's edge. LCD off.
DrawBorder:
	ld hl, TILEMAP0
	call .rowOf20
	ld hl, TILEMAP0 + (SCREEN_HEIGHT - 1) * TILEMAP_WIDTH
	call .rowOf20
	ld hl, TILEMAP0 + TILEMAP_WIDTH
	ld de, TILEMAP_WIDTH - (SCREEN_WIDTH - 1)
	ld b, SCREEN_HEIGHT - 2
.sides
	ld [hl], TILE_LATTICE
	ld a, l
	add SCREEN_WIDTH - 1
	ld l, a
	adc h
	sub l
	ld h, a
	ld [hl], TILE_LATTICE
	add hl, de
	dec b
	jr nz, .sides
	ret
.rowOf20
	ld b, SCREEN_WIDTH
	ld a, TILE_LATTICE
.cell
	ld [hli], a
	dec b
	jr nz, .cell
	ret

; Sets wGrade from the counts (see the thresholds above).
Grade:
	ld a, [wCleared]
	and a
	ld a, GRADE_F
	jr z, .set
	ld a, [wMiss]
	and a
	jr nz, .points
	ld a, [wPerfect]
	cp S_PERFECTS
	ld a, GRADE_S
	jr nc, .set
.points
	; hl = 2 * perfect + good
	ld a, [wPerfect]
	ld l, a
	ld h, 0
	add hl, hl
	ld a, [wGood]
	ld e, a
	ld d, 0
	add hl, de
	ld a, GRADE_A
	ld de, A_POINTS
	call .atLeast
	jr nc, .set
	ld a, GRADE_B
	ld de, B_POINTS
	call .atLeast
	jr nc, .set
	ld a, GRADE_C
	ld de, C_POINTS
	call .atLeast
	jr nc, .set
	ld a, GRADE_D
.set
	ld [wGrade], a
	ret
; Carry clear when hl >= de. Keeps a and hl.
.atLeast
	push af
	ld a, l
	sub e
	ld a, h
	sbc d
	jr c, .below
	pop af
	and a
	ret
.below
	pop af
	scf
	ret

GradeLetters: db "SABCDF"

StrCleared: db "CLEAR!", 0
StrFailed:  db "FAILED", 0
StrPerfect: db "PERFECT   ", 0
StrGood:    db "GOOD      ", 0
StrMiss:    db "MISS      ", 0
StrCombo:   db "MAX COMBO ", 0
StrScore:   db "SCORE  ", 0
StrGrade:   db "GRADE     ", 0
StrStart:   db "PRESS START", 0

SECTION "Results state", WRAM0

wCleared:: db
wGrade:: db
