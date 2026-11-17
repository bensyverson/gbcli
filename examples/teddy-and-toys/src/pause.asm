; Pause: START in a round stops everything and opens a small menu,
; Continue or Quit, in a box over the HUD's score box.
;
; While paused the main loop runs only PauseUpdate: the belts, the clock,
; Elle and a turn under way all wait. The VBlank handler skips the music's
; tick, so the song holds its place, and NR51 lets only channel 1 (the
; effects', where the pause sound plays) reach the speakers, so the notes
; the music left sounding are silent. Continue puts both back and the song
; goes on where it stopped.
;
;   Up, Down           move the cursor
;   A or START         pick what the cursor is on (it opens on Continue,
;                      so START, START pauses and continues)
;   B                  continue
;
; The menu's text is set in tiles when a round starts (PauseTilesInit), so
; opening and closing it only rewrites map entries in VBlank (PauseVBlank).

include "hardware.inc"
include "layout.inc"
include "images.inc"
include "bank.inc"

	MAIN_FONT_CHARMAP text, 0
	PUSHC text
DEF GLYPH_CURSOR EQU '>'
	POPC

DEF PAUSE_ROW EQU SCORE_ROW         ; the box: map rows 1-4
DEF PAUSE_COL EQU SCORE_COL         ; and columns 6-14, over the score box
DEF PAUSE_W   EQU 9
DEF PAUSE_H   EQU 4
	ASSERT PAUSE_COL + PAUSE_W <= KEY_X / 8, "the pause box must stay left of the key column"
	ASSERT PAUSE_ROW + PAUSE_H < BACK_BELT_ROW, "the pause box must stay in the HUD band"

DEF CURSOR_CONTINUE EQU 0
DEF CURSOR_QUIT     EQU 1

; wPauseDraw: what the VBlank handler draws next.
DEF DRAW_NONE    EQU 0
DEF DRAW_MENU    EQU 1              ; the box, its text and the cursor
DEF DRAW_CURSOR  EQU 2              ; the cursor only
DEF DRAW_RESTORE EQU 3              ; the HUD as it was (the score's digits: ScoreVBlank)

DEF PAPER_PLANE0 EQU $FF            ; COLOR_WHITE (1): plane 0 set, plane 1 clear
DEF PAPER_PLANE1 EQU $00

SECTION "Pause", ROM0

; Sets the menu's text in tiles T_PAUSE_CONTINUE onward. LCD off. Each
; tile starts as the box's white inside, because TextDraw leaves the
; tiles past a line's end as they were.
PauseTilesInit::
	ld hl, TILEBLOCK0 + T_PAUSE_CONTINUE * 16
	ld b, PAUSE_TEXT_TILES * 8
.paper
	ld a, PAPER_PLANE0
	ld [hli], a
	ld a, PAPER_PLANE1
	ld [hli], a
	dec b
	jr nz, .paper
	SWITCH_BANK MainFontGlyphs
	ld hl, ContinueText
	ld de, TILEBLOCK0 + T_PAUSE_CONTINUE * 16
	ld b, COLOR_INK
	ld c, COLOR_WHITE
	call TextDraw
	ld hl, QuitText
	ld de, TILEBLOCK0 + T_PAUSE_QUIT * 16
	ld b, COLOR_INK
	ld c, COLOR_WHITE
	call TextDraw
	ld de, MainFontGlyphs + 8 * GLYPH_CURSOR
	ld hl, TILEBLOCK0 + T_PAUSE_CURSOR * 16
	ld a, 1
	ld b, COLOR_INK
	ld c, COLOR_WHITE
	jp TextLoadTiles

; START in a round: pause.
PauseStart::
	ld a, STATE_PAUSED
	ld [wGameState], a
	ld a, CURSOR_CONTINUE
	ld [wPauseCursor], a
	ld a, DRAW_MENU
	ld [wPauseDraw], a
	ld a, AUDTERM_1_LEFT | AUDTERM_1_RIGHT   ; only the effects' channel
	ldh [rNR51], a
	ld a, SFX_PAUSE
	jp SfxPlay

; Once a frame while paused, from the main loop.
PauseUpdate::
	ldh a, [hJoyPressed]
	ld b, a
	bit B_PAD_B, b
	jr nz, PauseContinue
	and PAD_A | PAD_START
	jr nz, .pick
	ld c, CURSOR_CONTINUE
	bit B_PAD_UP, b
	jr nz, .move
	ld c, CURSOR_QUIT
	bit B_PAD_DOWN, b
	ret z
.move
	ld a, [wPauseCursor]
	cp c
	ret z
	ld a, c
	ld [wPauseCursor], a
	ld a, DRAW_CURSOR
	ld [wPauseDraw], a
	ld a, SFX_MENU
	jp SfxPlay
.pick
	ld a, [wPauseCursor]
	cp CURSOR_QUIT
	jp z, QuitRound
	; fall through

; Back to the round, where it stopped.
PauseContinue:
	ld a, STATE_PLAY
	ld [wGameState], a
	ld a, DRAW_RESTORE
	ld [wPauseDraw], a
	ld a, $FF                   ; every channel to both speakers (MusicInit)
	ldh [rNR51], a
	ret

; From the VBlank handler, in a round or paused: draws what wPauseDraw
; asks for. VRAM must be open.
PauseVBlank::
	ld a, [wPauseDraw]
	and a
	ret z
	ld b, a
	xor a
	ld [wPauseDraw], a
	ld a, b
	cp DRAW_RESTORE
	jr z, .restore
	cp DRAW_CURSOR
	jr z, .cursor
	ld de, PauseMenuMap
	call .box
.cursor
	; The cursor on its row, the box's inside on the other.
	ld hl, TILEMAP0 + (PAUSE_ROW + 1) * 32 + PAUSE_COL + 1
	ld a, [wPauseCursor]
	and a
	ld a, T_PAUSE_CURSOR
	ld b, T_BOX + 4
	jr z, .put
	ld a, b
	ld b, T_PAUSE_CURSOR
.put
	ld [hl], a
	ld a, b
	ld hl, TILEMAP0 + (PAUSE_ROW + 2) * 32 + PAUSE_COL + 1
	ld [hl], a
	ret
.restore
	ld de, PauseRestoreMap
	call .box
	ld a, 1
	ld [wScoreDirty], a         ; ScoreVBlank draws the digits next
	ret
	; The box's rows from de.
.box
	ld hl, TILEMAP0 + PAUSE_ROW * 32 + PAUSE_COL
	ld b, PAUSE_H
.row
	ld c, PAUSE_W
.tile
	ld a, [de]
	ld [hli], a
	inc de
	dec c
	jr nz, .tile
	ld a, l
	add 32 - PAUSE_W
	ld l, a
	adc h
	sub l
	ld h, a
	dec b
	jr nz, .row
	ret

; The menu: the score box's frame tiles around "Continue" and "Quit".
PauseMenuMap:
	db T_BOX, T_BOX + 1, T_BOX + 1, T_BOX + 1, T_BOX + 1, T_BOX + 1, T_BOX + 1, T_BOX + 1, T_BOX + 2
	db T_BOX + 3, T_BOX + 4
	FOR N, 5
		db T_PAUSE_CONTINUE + N
	ENDR
	db T_BOX + 4, T_BOX + 5
	db T_BOX + 3, T_BOX + 4
	FOR N, 4
		db T_PAUSE_QUIT + N
	ENDR
	db T_BOX + 4, T_BOX + 4, T_BOX + 5
	db T_BOX + 6, T_BOX + 7, T_BOX + 7, T_BOX + 7, T_BOX + 7, T_BOX + 7, T_BOX + 7, T_BOX + 7, T_BOX + 8
	ASSERT @ - PauseMenuMap == PAUSE_W * PAUSE_H

; The HUD under the box, as ScreenInit draws it: the 5x3 score box at the
; box's top left (its digits are ScoreVBlank's), the wall elsewhere.
PauseRestoreMap:
	db T_BOX, T_BOX + 1, T_BOX + 1, T_BOX + 1, T_BOX + 2, T_WALL, T_WALL, T_WALL, T_WALL
	db T_BOX + 3, T_BOX + 4, T_BOX + 4, T_BOX + 4, T_BOX + 5, T_WALL, T_WALL, T_WALL, T_WALL
	db T_BOX + 6, T_BOX + 7, T_BOX + 7, T_BOX + 7, T_BOX + 8, T_WALL, T_WALL, T_WALL, T_WALL
	db T_WALL, T_WALL, T_WALL, T_WALL, T_WALL, T_WALL, T_WALL, T_WALL, T_WALL
	ASSERT @ - PauseRestoreMap == PAUSE_W * PAUSE_H
	ASSERT PAUSE_ROW == SCORE_ROW && PAUSE_COL == SCORE_COL, "PauseRestoreMap puts the score box at the box's top left"

	PUSHC text
ContinueText: db "Continue", MAIN_FONT_END
QuitText:     db "Quit", MAIN_FONT_END
	POPC

SECTION "Pause variables", WRAM0
wPauseCursor:: db                  ; CURSOR_CONTINUE or CURSOR_QUIT
wPauseDraw:: db                    ; DRAW_*: what the VBlank handler draws next
