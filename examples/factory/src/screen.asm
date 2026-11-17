; The screen: placeholder tiles, the background map (HUD, belts, floor)
; and the window map (the key column). ScreenInit runs with the LCD off.

include "hardware.inc"
include "layout.inc"
include "gen/toys.inc"
include "images.inc"
include "bank.inc"

	MAIN_FONT_CHARMAP text, 0
	PUSHC text
DEF GLYPH_ZERO EQU '0'              ; the font's glyph number for 0
	POPC

SECTION "Screen", ROM0

; Loads every tile and draws both maps. LCD off, interrupts off.
ScreenInit::
	; Placeholder tiles 0-14.
	ld hl, TILEBLOCK0 + T_WALL * 16
	ld de, PlaceholderTiles
	ld bc, PlaceholderTiles.end - PlaceholderTiles
	call CopyMem

	; Score digits 0-9, from the font, one glyph a tile.
	SWITCH_BANK MainFontGlyphs
	ld de, MainFontGlyphs + 8 * GLYPH_ZERO
	ld hl, TILEBLOCK0 + T_DIGITS * 16
	ld a, 10
	ld b, 3                     ; ink
	ld c, 0                     ; paper
	call TextLoadTiles

	; The toys: halves and finished toys, from the toy data.
	ld hl, TILEBLOCK0 + T_TOYS * 16
	ld de, ToyTiles
	ld bc, ToyTilesEnd - ToyTiles
	call CopyMem

	; Elle.
	SWITCH_BANK ElleTiles
	ld hl, TILEBLOCK0 + T_ELLE * 16
	ld de, ElleTiles
	ld bc, ElleTilesEnd - ElleTiles
	call CopyMem

	; Background: the wall everywhere, then the bands over it.
	ld hl, TILEMAP0
	ld bc, TILEMAP_AREA
	call ClearMem               ; T_WALL is 0
	ld hl, TILEMAP0 + BACK_BELT_ROW * 32
	call DrawBeltRows
	ld hl, TILEMAP0 + FLOOR_ROW * 32
	ld bc, 3 * 32
	ld a, T_FLOOR
	call FillMem
	ld hl, TILEMAP0 + FRONT_BELT_ROW * 32
	call DrawBeltRows

	; The score box: 5x3 tiles from the 9 box tiles.
	ld hl, TILEMAP0 + SCORE_ROW * 32 + SCORE_COL
	ld a, T_BOX
	call DrawBoxRow
	ld hl, TILEMAP0 + (SCORE_ROW + 1) * 32 + SCORE_COL
	ld a, T_BOX + 3
	call DrawBoxRow
	ld hl, TILEMAP0 + (SCORE_ROW + 2) * 32 + SCORE_COL
	ld a, T_BOX + 6
	call DrawBoxRow

	; The key column: divider and white, then each toy's finished icon.
	ld hl, TILEMAP1
	ld b, SCREEN_HEIGHT
.keyRow
	ld a, T_KEY_DIVIDER
	ld [hli], a
	ld a, T_KEY_BG
	ld [hli], a
	ld [hli], a
	ld de, 32 - 3
	add hl, de
	dec b
	jr nz, .keyRow
	ld hl, TILEMAP1 + 1
	ld c, 0                     ; the toy
.keyIcon
	ld a, c
	call ToyTile                ; a = the toy's first tile
	add 8                       ; its finished frame
	call Draw2x2
	ld de, KEY_ROWS_PER_TOY * 32
	add hl, de
	inc c
	ld a, c
	cp TOY_COUNT
	jr nz, .keyIcon
	ret

; a = the first tile of toy a's 12 (half A, half B, finished).
ToyTile::
	ld b, a
	add a                       ; *2
	add b                       ; *3
	add a                       ; *6
	add a                       ; *12
	add T_TOYS
	ret

; Writes a 2x2 piece at map address hl: tiles a, a+1 down the left column,
; a+2, a+3 down the right (the toy tiles' order). Keeps hl. Clobbers a, de.
Draw2x2::
	push hl
	ld [hli], a
	add 2
	ld [hl], a
	ld de, 32 - 1
	add hl, de
	dec a
	ld [hli], a
	add 2
	ld [hl], a
	pop hl
	ret

; Four rows from hl: stripe, belt, belt, stripe.
DrawBeltRows:
	ld bc, 32
	ld a, T_STRIPE
	call FillMem
	ld bc, 2 * 32
	ld a, T_BELT
	call FillMem
	ld bc, 32
	ld a, T_STRIPE
	jp FillMem

; One row of the score box at hl: a, then a+1 three times, then a+2.
DrawBoxRow:
	ld [hli], a
	inc a
	ld [hli], a
	ld [hli], a
	ld [hli], a
	inc a
	ld [hl], a
	ret

PlaceholderTiles:
	; T_WALL
	REPT 8
	dw `00000000
	ENDR
	; T_STRIPE
	dw `33111111
	dw `13311111
	dw `11331111
	dw `11133111
	dw `11113311
	dw `11111331
	dw `11111133
	dw `31111113
	; T_BELT
	REPT 8
	dw `00000000
	ENDR
	; T_FLOOR
	dw `11111111
	dw `11121111
	dw `11111111
	dw `11111111
	dw `11111112
	dw `11111111
	dw `12111111
	dw `11111111
	; T_KEY_BG
	REPT 8
	dw `00000000
	ENDR
	; T_KEY_DIVIDER
	REPT 8
	dw `33000000
	ENDR
	; T_BOX: top left, top, top right
	dw `00000000
	dw `03333333
	REPT 6
	dw `03000000
	ENDR
	dw `00000000
	dw `33333333
	REPT 6
	dw `00000000
	ENDR
	dw `00000000
	dw `33333330
	REPT 6
	dw `00000030
	ENDR
	; left, inside, right
	REPT 8
	dw `03000000
	ENDR
	REPT 8
	dw `00000000
	ENDR
	REPT 8
	dw `00000030
	ENDR
	; bottom left, bottom, bottom right
	REPT 6
	dw `03000000
	ENDR
	dw `03333333
	dw `00000000
	REPT 6
	dw `00000000
	ENDR
	dw `33333333
	dw `00000000
	REPT 6
	dw `00000030
	ENDR
	dw `33333330
	dw `00000000
.end
