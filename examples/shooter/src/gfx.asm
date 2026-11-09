; Loading the art that tools/draw_art.py draws and `gbcli build` converts
; (build/images/images.inc): tiles, maps and, on a GBC, palettes and
; attributes. Everything here runs with the LCD off.

include "hardware.inc"
include "game.inc"
include "images.inc"

; Copies a converted image's tiles: \1 is its label stem, \2 the tile.
MACRO load_tiles
	ld hl, TILEBLOCK0 + (\2) * 16
	ld de, \1Tiles
	ld bc, \1TilesEnd - \1Tiles
	call CopyMem
ENDM

SECTION "Graphics", ROM0

; The sprites and the font, once at start-up.
GfxInit::
	load_tiles Ship, T_SHIP
	load_tiles Bullet, T_BULLET
	load_tiles Enemies, T_DART
	load_tiles Boss, T_BOSS
	load_tiles Font, T_FONT
	ld a, SKY0_BGP                    ; every image's: each index its shade
	ldh [rBGP], a
	ldh [rOBP0], a
	xor a
	ldh [rOBP1], a                    ; the hit flash: all white
IF DEF(GBCLI_GBC)
	ldh a, [hGbc]
	and a
	ret z
	; Objects: ship, enemy, shot, boss, then the flash's all-white.
	ld a, OBPI_AUTOINC
	ldh [rOCPS], a
	ld hl, PaletteShip
	call Palette
	ld hl, PaletteEnemy
	call Palette
	ld hl, PaletteShot
	call Palette
	ld hl, PaletteBoss
	call Palette
	ld b, 8
	ld a, $FF
.white
	ldh [rOCPD], a
	dec b
	jr nz, .white
	; Background: the bands, the title, then the text in slot 7.
	ld a, BGPI_AUTOINC
	ldh [rBCPS], a
	ld hl, PaletteStars
	call BgPalette
	ld hl, PalettePeaks
	call BgPalette
	ld hl, PaletteForest
	call BgPalette
	ld hl, PaletteGround
	call BgPalette
	ld hl, PaletteTitle
	call BgPalette
	ld a, BGPI_AUTOINC | (PAL_TEXT * 8)
	ldh [rBCPS], a
	ld hl, PaletteText
	jp BgPalette

; 8 bytes from HL to the OBJ palette data port.
Palette:
	ld b, 8
.byte
	ld a, [hli]
	ldh [rOCPD], a
	dec b
	jr nz, .byte
	ret

; 8 bytes from HL to the BG palette data port.
BgPalette:
	ld b, 8
.byte
	ld a, [hli]
	ldh [rBCPD], a
	dec b
	jr nz, .byte
ENDC
	ret

; The play screen's background: the sky's four bands, tiles from T_SCREEN,
; one map row of 32 for each tile row, and on a GBC each band's palette.
SkyLoad::
	load_tiles Sky0, T_SCREEN
	load_tiles Sky1, T_SCREEN + SKY0_TILES
	load_tiles Sky2, T_SCREEN + SKY0_TILES + SKY1_TILES
	load_tiles Sky3, T_SCREEN + SKY0_TILES + SKY1_TILES + SKY2_TILES
	ld hl, TILEMAP0
	ld de, Sky0Tilemap
	ld bc, Sky0TilemapEnd - Sky0Tilemap
	ld a, T_SCREEN
	call MapCopy
	ld de, Sky1Tilemap
	ld bc, Sky1TilemapEnd - Sky1Tilemap
	ld a, T_SCREEN + SKY0_TILES
	call MapCopy
	ld de, Sky2Tilemap
	ld bc, Sky2TilemapEnd - Sky2Tilemap
	ld a, T_SCREEN + SKY0_TILES + SKY1_TILES
	call MapCopy
	ld de, Sky3Tilemap
	ld bc, Sky3TilemapEnd - Sky3Tilemap
	ld a, T_SCREEN + SKY0_TILES + SKY1_TILES + SKY2_TILES
	call MapCopy
IF DEF(GBCLI_GBC)
	ldh a, [hGbc]
	and a
	ret z
	ld a, 1
	ldh [rVBK], a
	ld hl, TILEMAP0
	ld bc, SKY0_HEIGHT * 32
	ld a, PAL_STARS
	call FillMem
	ld bc, SKY1_HEIGHT * 32
	ld a, PAL_PEAKS
	call FillMem
	ld bc, SKY2_HEIGHT * 32
	ld a, PAL_FOREST
	call FillMem
	ld bc, SKY3_HEIGHT * 32
	ld a, PAL_GROUND
	call FillMem
	xor a
	ldh [rVBK], a
ENDC
	ret

; The title screen: its image on the window, tiles from T_SCREEN.
TitleLoad::
	load_tiles Title, T_SCREEN
	ld hl, TILEMAP1
	ld de, TitleTilemap
	ld b, TITLE_HEIGHT
.row
	push bc
	ld bc, TITLE_WIDTH
	ld a, T_SCREEN
	call MapCopy
	ld bc, 32 - TITLE_WIDTH
	add hl, bc
	pop bc
	dec b
	jr nz, .row
	ld a, PAL_TITLE
	jp WindowAttrs

; Copies BC map bytes from DE to HL, adding A to each. Leaves HL and DE
; after them.
MapCopy:
	push af
	ld a, [de]
	inc de
	ld [hl], a
	pop af
	push af
	add [hl]
	ld [hli], a
	pop af
	dec bc
	push af
	ld a, b
	or c
	jr z, .done
	pop af
	jr MapCopy
.done
	pop af
	ret
