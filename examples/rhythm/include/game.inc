; Lantern Drum's shared constants and text encoding.

IF !DEF(GAME_INC)
DEF GAME_INC EQU 1

include "chart.inc"             ; PREROLL, CHART_LEN, SONG_END_TICK (tools/chart.py)

; Judgment windows, in driver ticks (one tick is one frame: `timer off`).
; Wide enough for a first-time player: 3 ticks is 50 ms either side, 8 is
; 133 ms. The closest two notes of one lane are 14 ticks apart, so a good
; window never reaches the next note's tick.
DEF PERFECT_WIN EQU 3           ; |press - note| <= 3 ticks: perfect
DEF GOOD_WIN EQU 8              ; <= 8: good; a note 9 ticks past is a miss
DEF LOOKAHEAD EQU 56            ; ticks before its tick that a note shows
DEF SPEED_SHIFT EQU 1           ; notes fall 2 px a tick
DEF HIT_Y EQU 120               ; screen y of the hit line's top
DEF LABEL_ROW EQU HIT_Y / 8 + 1 ; map row of the lanes' button names

; What wJudged holds for a note, and wLastJudge for the latest one.
DEF JUDGE_NONE EQU 0
DEF JUDGE_PERFECT EQU 1
DEF JUDGE_GOOD EQU 2
DEF JUDGE_MISS EQU 3

; The life meter: full at LIFE_MAX, a miss costs LIFE_MISS, a perfect
; gives back LIFE_PERFECT and a good LIFE_GOOD; empty fails the song.
; 24 misses in a row from full fail it, and two goods (or one perfect) pay
; back a miss, so a player who hits two notes in three, however late or
; early, never fails. The opening lesson's LESSON_NOTES (include/chart.inc)
; cost no life at all.
DEF LIFE_MAX EQU 48
DEF LIFE_MISS EQU 2
DEF LIFE_PERFECT EQU 2
DEF LIFE_GOOD EQU 1
DEF LIFE_CELLS EQU 6            ; meter cells; each holds LIFE_MAX / 6
DEF JUDGE_SHOW EQU 30           ; frames a judgment word stays up

; wState.
DEF STATE_PLAY EQU 0
DEF STATE_RESULTS EQU 1
DEF STATE_TITLE EQU 2

; wMusicMode: the title loop runs free; the play's song runs the clock.
DEF MUSIC_PLAY EQU 0
DEF MUSIC_TITLE EQU 1

; wGrade: an index into GradeLetters, best first.
DEF GRADE_S EQU 0
DEF GRADE_A EQU 1
DEF GRADE_B EQU 2
DEF GRADE_C EQU 3
DEF GRADE_D EQU 4
DEF GRADE_F EQU 5

; Tiles: assets/tiles/play.pixels from 0 (in its order), the font's from
; FONT_BASE, the title screen's from TITLE_BASE (block 1, $8800).
DEF TILE_BLANK EQU 0
DEF TILE_LANE EQU 1
DEF TILE_DRUM EQU 2             ; the hit marker
DEF TILE_LANTERN EQU 3          ; a note
DEF TILE_BURST EQU 4            ; a hit, over the drum
DEF TILE_LATTICE EQU 5          ; the results screen's border
DEF FONT_BASE EQU 32
DEF TITLE_BASE EQU 128
DEF BURST_FRAMES EQU 8          ; frames a hit's burst shows
DEF BURST_PALETTE EQU 4         ; GBC OBJ palette of the bursts

; Text: tile = FONT_BASE + its place in tools/font.py's CHARS.
DEF FONT_CHARS EQUS " 0123456789ABCDEFGHIJKLMNOPQRSTUVWXYZ!-:[]<>"
FOR I, STRLEN("{FONT_CHARS}")
	CHARMAP STRSLICE("{FONT_CHARS}", I, I + 1), FONT_BASE + I
ENDR
DEF TILE_DIGIT0 EQU FONT_BASE + 1
DEF TILE_METER_EMPTY EQU FONT_BASE + 40
DEF TILE_METER_FULL EQU FONT_BASE + 41

; HUD places on the map (row, column).
DEF HUD_SCORE EQU TILEMAP0 + 1 * 32 + 0
DEF HUD_COMBO EQU TILEMAP0 + 4 * 32 + 0
DEF HUD_JUDGE EQU TILEMAP0 + 6 * 32 + 0
DEF HUD_LIFE EQU TILEMAP0 + 9 * 32 + 0
DEF HUD_SCORE_LEN EQU 6
DEF HUD_COMBO_LEN EQU 3
DEF HUD_JUDGE_LEN EQU 7

ENDC
