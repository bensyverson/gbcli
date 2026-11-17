; GBC colors. Every palette keeps the DMG's color numbers in order
; (include/layout.inc): 0 the belts' dark gray (or clear, for objects),
; 1 the lightest, 2 the middle, 3 the ink. So one tile set draws the same
; picture on both machines, and a GBC only swaps in colors: the wall pink,
; the clock face white (red in the last seconds), Elle blonde in denim,
; each toy in its own colors, and the title's rows pink and cream.
; A DMG build assembles none of this.
;
; The toys keep color 0 (the belt) and color 3 (the ink, which also draws
; the belt's lines behind a piece) the scene's, so a piece sits on its
; belt; colors 1 and 2 are the toy's. There are 5 palettes for 6 toys:
; the teddy bear and the gag glasses share the warm one (cream and tan:
; the glasses' frames and nose). The same palette numbers serve BG (the
; belts, the key column) and OBJ (Elle's hands).

IF DEF(GBCLI_GBC)

include "hardware.inc"
include "layout.inc"
include "gen/toys.inc"

; The palette numbers (PAL_*, OBJ_PAL_*) are in include/layout.inc.

MACRO rgb
	dw (\1) | (\2) << 5 | (\3) << 10
ENDM

DEF PAL_BYTES EQU 8
DEF BASE_PALETTES EQU 3             ; palettes 0-2 differ by screen; 3-7 are the toys'

SECTION "Colors", ROM0

; A round's colors, LCD off: every palette, both attribute maps, and the
; key column's toys. Belt slots color themselves as they are drawn
; (SlotColors), so call this before BeltsInit.
ColorGbc::
	ld hl, BgRoundColors
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
	; The key column's icons (src/screen.asm draws them at these places).
	ld hl, TILEMAP1 + 1
	ld c, 0
.key
	ld a, c
	inc a
	call SlotColors
	ld de, KEY_ROWS_PER_TOY * 32
	add hl, de
	inc c
	ld a, c
	cp TOY_COUNT
	jr nz, .key
	ret

; The title and results screens' colors, LCD off: their palettes, the
; text's palette everywhere, and on the title (a = 0) the name's rows.
; DrawToyBelt colors its belt.
ColorGbcScreen::
	push af
	ld hl, BgScreenColors
	call LoadPalettes
	ld a, 1
	ldh [rVBK], a
	ld hl, ATTRMAP0
	ld bc, TILEMAP_AREA
	ld a, PAL_TEXT
	call FillMem
	pop af
	and a
	jr nz, .done
	ld hl, ATTRMAP0
	ld bc, TITLE_BELT_ROW * 32      ; the rows above the title's belt
	ld a, PAL_NAME
	call FillMem
.done
	xor a
	ldh [rVBK], a
	ret

; Colors the 2x2 cells at map address hl (either map) with toy a - 1's
; palette, or the scene's for a = 0: an empty belt slot. Only in GBC
; mode; VRAM must be open. Keeps bc, hl.
SlotColors::
	ld d, a
	ldh a, [hGbc]
	and a
	ret z
	ld a, d
	and a
	jr z, .set                  ; PAL_SCENE is 0
	dec a
	call ToyPalette
.set
	ld d, a
	ld a, 1
	ldh [rVBK], a
	push hl
	ld a, d
	ld [hli], a
	ld [hl], a
	ld de, 32 - 1
	add hl, de
	ld [hli], a
	ld [hl], a
	pop hl
	xor a
	ldh [rVBK], a
	ret

; a = toy a's palette number. Keeps bc, de, hl.
ToyPalette::
	push hl
	add LOW(ToyPalettes)
	ld l, a
	adc HIGH(ToyPalettes)
	sub l
	ld h, a
	ld a, [hl]
	pop hl
	ret

; In data/toys.txt's order.
ToyPalettes:
	db PAL_TOY_WARM             ; teddy bear
	db PAL_TOY_TRAIN
	db PAL_TOY_SQUISHY
	db PAL_TOY_SLINKY
	db PAL_TOY_WARM             ; gag glasses
	db PAL_TOY_CRAYONS
	ASSERT @ - ToyPalettes == TOY_COUNT, "ToyPalettes needs one palette per toy in data/toys.txt"

; BG palettes 0-2 from hl, then the toys' (BG 3-7); OBJ: Elle's (0-2) and
; the toys' again (3-7).
LoadPalettes:
	ld a, BGPI_AUTOINC
	ldh [rBCPS], a
	ld b, BASE_PALETTES * PAL_BYTES
	call .bg
	ld hl, ToyColors
	ld b, ToyColorsEnd - ToyColors
	call .bg
	ld a, OBPI_AUTOINC
	ldh [rOCPS], a
	ld hl, ElleColors
	ld b, ElleColorsEnd - ElleColors
	call .obj
	ld hl, ToyColors
	ld b, ToyColorsEnd - ToyColors
.obj
	ld a, [hli]
	ldh [rOCPD], a
	dec b
	jr nz, .obj
	ret
.bg
	ld a, [hli]
	ldh [rBCPD], a
	dec b
	jr nz, .bg
	ret

BgRoundColors:
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
	ASSERT @ - BgRoundColors == BASE_PALETTES * PAL_BYTES
ClockRedColors::                    ; PAL_CLOCK in the last seconds: the face red
	rgb 20, 2, 2
	rgb 31, 13, 11
	rgb 31, 21, 24
	rgb 8, 0, 0

BgScreenColors:
	; PAL_SCENE
	rgb 11, 11, 13
	rgb 31, 31, 31
	rgb 22, 22, 23
	rgb 2, 2, 4
	; PAL_NAME: the title's top rows, the wall's pink, the name in plum
	rgb 11, 11, 13
	rgb 31, 21, 24
	rgb 21, 12, 15
	rgb 13, 2, 12
	; PAL_TEXT: cream paper, navy ink
	rgb 11, 11, 13
	rgb 31, 30, 23
	rgb 22, 22, 23
	rgb 3, 5, 16
	ASSERT @ - BgScreenColors == BASE_PALETTES * PAL_BYTES

ToyColors:
	; PAL_TOY_WARM: cream, tan (the teddy bear; the glasses' frames and nose)
	rgb 11, 11, 13
	rgb 31, 28, 20
	rgb 24, 15, 8
	rgb 2, 2, 4
	; PAL_TOY_TRAIN: light blue, red
	rgb 11, 11, 13
	rgb 21, 27, 31
	rgb 28, 7, 6
	rgb 2, 2, 4
	; PAL_TOY_SQUISHY: the jar's pale glass, the lime squishy
	rgb 11, 11, 13
	rgb 26, 31, 31
	rgb 15, 28, 8
	rgb 2, 2, 4
	; PAL_TOY_SLINKY: pink, violet
	rgb 11, 11, 13
	rgb 31, 18, 26
	rgb 20, 10, 28
	rgb 2, 2, 4
	; PAL_TOY_CRAYONS: yellow, green
	rgb 11, 11, 13
	rgb 31, 27, 5
	rgb 6, 21, 8
	rgb 2, 2, 4
ToyColorsEnd:
	ASSERT ToyColorsEnd - ToyColors == (8 - BASE_PALETTES) * PAL_BYTES
	ASSERT PAL_TOY_WARM == BASE_PALETTES && PAL_TOY_CRAYONS == 7

ElleColors:
	; OBJ_PAL_HEAD: (clear), skin, blonde hair, plum ink (eyes, choker)
	rgb 0, 0, 0
	rgb 31, 25, 20
	rgb 29, 22, 9
	rgb 13, 4, 15
	; OBJ_PAL_TOP (and OBJ_PAL_LEGS): skin, denim, navy
	rgb 0, 0, 0
	rgb 31, 25, 20
	rgb 18, 24, 30
	rgb 4, 5, 12
	; OBJ_PAL_HAIR: skin, blonde hair, brown ink
	rgb 0, 0, 0
	rgb 31, 25, 20
	rgb 29, 22, 9
	rgb 9, 5, 3
ElleColorsEnd:
	ASSERT ElleColorsEnd - ElleColors == BASE_PALETTES * PAL_BYTES

; Elle's OBJ palette per object: per frame (back, front, side), three rows
; (head, top, legs) of two columns (left, right). The side view's left
; column is the hair down her back, its right her denim front.
ElleObjPalettes::
	db OBJ_PAL_HEAD, OBJ_PAL_HEAD, OBJ_PAL_HAIR, OBJ_PAL_HAIR, OBJ_PAL_LEGS, OBJ_PAL_LEGS
	db OBJ_PAL_HEAD, OBJ_PAL_HEAD, OBJ_PAL_TOP,  OBJ_PAL_TOP,  OBJ_PAL_LEGS, OBJ_PAL_LEGS
	db OBJ_PAL_HEAD, OBJ_PAL_HEAD, OBJ_PAL_HAIR, OBJ_PAL_TOP,  OBJ_PAL_LEGS, OBJ_PAL_LEGS

ENDC
