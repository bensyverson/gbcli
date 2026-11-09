; Loads the art the build converted from assets/ (build/images/images.inc).

include "hardware.inc"
include "game.inc"
include "images.inc"

SECTION "Tiles", ROM0

; Copies the dungeon tiles to tile 0 on, the actors to S_PLAYER on and the
; font to FONT_BASE on, all in block 0. LCD off.
LoadTiles::
	ld hl, TILEBLOCK0
	ld de, DungeonTiles
	ld bc, DungeonTilesEnd - DungeonTiles
	call CopyMem
	ld hl, TILEBLOCK0 + S_PLAYER * 16
	ld de, HeroTiles
	ld bc, HeroTilesEnd - HeroTiles
	call CopyMem
	ld hl, TILEBLOCK0 + S_RAT * 16
	ld de, FoesTiles
	ld bc, FoesTilesEnd - FoesTiles
	call CopyMem
	ld hl, TILEBLOCK0 + FONT_BASE * 16
	ld de, FontTiles
	ld bc, FontTilesEnd - FontTiles
	jp CopyMem

; Draws the title screen: its tiles from tile 0, its map at TILEMAP0, and on
; a GBC its palette. LCD off.
LoadTitle::
	ld hl, TILEBLOCK0
	ld de, TitleTiles
	ld bc, TitleTilesEnd - TitleTiles
	call CopyMem
	ld hl, TILEMAP0
	ld de, TitleTilemap
	ld c, TITLE_HEIGHT
.row
	ld b, TITLE_WIDTH
.cell
	ld a, [de]
	ld [hli], a
	inc de
	dec b
	jr nz, .cell
	push de
	ld de, 32 - TITLE_WIDTH
	add hl, de
	pop de
	dec c
	jr nz, .row
IF DEF(GBCLI_GBC)
	ldh a, [hGbc]
	and a
	ret z
	ld a, BGPI_AUTOINC
	ldh [rBCPS], a
	ld hl, PaletteEmber
	ld b, PaletteEmberEnd - PaletteEmber
.pal
	ld a, [hli]
	ldh [rBCPD], a
	dec b
	jr nz, .pal
	ld a, 1
	ldh [rVBK], a
	ld hl, ATTRMAP0
	ld bc, $800
	xor a
	call FillMem
	xor a
	ldh [rVBK], a
ENDC
	ret
