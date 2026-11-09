; Text: the font's tiles, strings on the map, numbers as digit tiles.

include "hardware.inc"
include "game.inc"
include "images.inc"

SECTION "Text", ROM0

; Copies the font to tile FONT_BASE. LCD off.
LoadFont::
	ld hl, TILEBLOCK0 + FONT_BASE * 16
	ld de, FontTiles
	ld bc, FontTilesEnd - FontTiles
	jp CopyMem

; Writes the zero-terminated string at DE from HL (the map with the LCD off,
; or a buffer).
; Returns HL past the last tile.
PrintStr::
	ld a, [de]
	and a
	ret z
	ld [hli], a
	inc de
	jr PrintStr

; Writes A (0-255) as three digit tiles from HL; returns HL past them.
; Keeps DE.
PrintByte::
	ld b, 0
.hundreds
	cp 100
	jr c, .tens0
	sub 100
	inc b
	jr .hundreds
.tens0
	ld c, a
	ld a, b
	add TILE_DIGIT0
	ld [hli], a
	ld a, c
	ld b, 0
.tens
	cp 10
	jr c, .ones
	sub 10
	inc b
	jr .tens
.ones
	ld c, a
	ld a, b
	add TILE_DIGIT0
	ld [hli], a
	ld a, c
	add TILE_DIGIT0
	ld [hli], a
	ret

; Writes the 3-byte BCD score at DE (lowest byte first) as six digit tiles
; from HL, highest digit first.
PrintScore::
	inc de
	inc de
	ld b, 3
.byte
	ld a, [de]
	swap a
	and $0F
	add TILE_DIGIT0
	ld [hli], a
	ld a, [de]
	and $0F
	add TILE_DIGIT0
	ld [hli], a
	dec de
	dec b
	jr nz, .byte
	ret
