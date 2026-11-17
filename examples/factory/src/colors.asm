; GBC colors. Every palette keeps the DMG's color numbers in order
; (include/layout.inc): 0 the belts' dark gray (or clear, for objects),
; 1 the lightest, 2 the middle, 3 the ink. So one tile set draws the same
; picture on both machines, and a GBC only swaps in colors: the wall pink,
; the clock face white (red in the last seconds), Elle blonde in denim.
; A DMG build assembles none of this.

IF DEF(GBCLI_GBC)

include "hardware.inc"
include "layout.inc"

; The palette numbers (PAL_*, OBJ_PAL_*) are in include/layout.inc.

MACRO rgb
	dw (\1) | (\2) << 5 | (\3) << 10
ENDM

SECTION "Colors", ROM0

; A round's colors, LCD off: every palette, and both attribute maps.
ColorGbc::
	call LoadPalettes
	ld a, 1
	ldh [rVBK], a
	ld hl, ATTRMAP0
	ld bc, 2 * TILEMAP_AREA     ; both maps: the scene's palette
	call ClearMem
	ld hl, ATTRMAP0
	ld bc, BACK_BELT_ROW * 32   ; the HUD band
	ld a, PAL_WALL
	call FillMem
	ld hl, ATTRMAP0 + CLOCK_ROW * 32 + CLOCK_COL
	ld de, 32 - 4
	ld c, 4
.clockRow
	ld a, PAL_CLOCK
	REPT 4
	ld [hli], a
	ENDR
	add hl, de
	dec c
	jr nz, .clockRow
	xor a
	ldh [rVBK], a
	ret

; The title and results screens' colors, LCD off: the scene's palette
; everywhere.
ColorGbcPlain::
	call LoadPalettes
	ld a, 1
	ldh [rVBK], a
	ld hl, ATTRMAP0
	ld bc, TILEMAP_AREA
	call ClearMem
	xor a
	ldh [rVBK], a
	ret

LoadPalettes:
	ld a, BGPI_AUTOINC
	ldh [rBCPS], a
	ld hl, BgColors
	ld b, BgColorsEnd - BgColors
.bg
	ld a, [hli]
	ldh [rBCPD], a
	dec b
	jr nz, .bg
	ld a, OBPI_AUTOINC
	ldh [rOCPS], a
	ld hl, ObjColors
	ld b, ObjColorsEnd - ObjColors
.obj
	ld a, [hli]
	ldh [rOCPD], a
	dec b
	jr nz, .obj
	ret

BgColors:
	; PAL_SCENE: the belt, white, light gray, ink
	rgb 11, 11, 13
	rgb 31, 31, 31
	rgb 22, 22, 23
	rgb 2, 2, 4
ClockNormalColors::                 ; PAL_CLOCK: ticks, the face, the wall, ink
	rgb 10, 10, 12
	rgb 31, 31, 31
	rgb 31, 21, 24
	rgb 2, 2, 4
	; PAL_WALL: the rail's shadow, white, the pink wall, ink
	rgb 21, 12, 15
	rgb 31, 31, 31
	rgb 31, 21, 24
	rgb 2, 2, 4
BgColorsEnd:
ClockRedColors::                    ; PAL_CLOCK in the last seconds: the face red
	rgb 20, 2, 2
	rgb 31, 13, 11
	rgb 31, 21, 24
	rgb 8, 0, 0

ObjColors:
	; OBJ_PAL_HEAD: (clear), skin, blonde hair, plum ink (eyes, choker)
	rgb 0, 0, 0
	rgb 31, 25, 20
	rgb 29, 22, 9
	rgb 13, 4, 15
	; OBJ_PAL_TOP: skin, denim, navy
	rgb 0, 0, 0
	rgb 31, 25, 20
	rgb 18, 24, 30
	rgb 4, 5, 12
	; OBJ_PAL_HAIR: skin, blonde hair, brown ink
	rgb 0, 0, 0
	rgb 31, 25, 20
	rgb 29, 22, 9
	rgb 9, 5, 3
	; OBJ_PAL_LEGS: skin, denim, the navy skirt and shoes
	rgb 0, 0, 0
	rgb 31, 25, 20
	rgb 18, 24, 30
	rgb 5, 5, 11
	; OBJ_PAL_PIECES: white, light gray, ink
	rgb 0, 0, 0
	rgb 31, 31, 31
	rgb 22, 22, 23
	rgb 2, 2, 4
ObjColorsEnd:

; Elle's OBJ palette per object: per frame (back, front, side), three rows
; (head, top, legs) of two columns (left, right). The side view's left
; column is the hair down her back, its right her denim front.
ElleObjPalettes::
	db OBJ_PAL_HEAD, OBJ_PAL_HEAD, OBJ_PAL_HAIR, OBJ_PAL_HAIR, OBJ_PAL_LEGS, OBJ_PAL_LEGS
	db OBJ_PAL_HEAD, OBJ_PAL_HEAD, OBJ_PAL_TOP,  OBJ_PAL_TOP,  OBJ_PAL_LEGS, OBJ_PAL_LEGS
	db OBJ_PAL_HEAD, OBJ_PAL_HEAD, OBJ_PAL_HAIR, OBJ_PAL_TOP,  OBJ_PAL_LEGS, OBJ_PAL_LEGS

ENDC
