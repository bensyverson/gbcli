; The title screen: assets/bg/title.pixels, the title theme, and START.

include "hardware.inc"
include "defs.inc"
include "images.inc"

SECTION "Title", ROM0

; Shows the title until START is pressed, then starts a game. Comes from
; Start (LCD off) or from the main loop after an ending (LCD on).
Title::
	di
	call LcdOff
	xor a
	ld [wMode], a               ; MODE_TITLE: the VBlank handler skips the maze
	ld hl, TILEBLOCK0
	ld de, TitleTiles
	ld bc, TitleTilesEnd - TitleTiles
	call CopyMem
	; The tilemap: TITLE_HEIGHT rows of TITLE_WIDTH tiles, into rows of 32.
	ld hl, TILEMAP0
	ld de, TitleTilemap
	ld c, TITLE_HEIGHT
.row
	ld b, TITLE_WIDTH
.col
	ld a, [de]
	inc de
	ld [hli], a
	dec b
	jr nz, .col
	ld a, l
	add 32 - TITLE_WIDTH
	ld l, a
	adc h
	sub l
	ld h, a
	dec c
	jr nz, .row
IF DEF(GBCLI_GBC)
	ldh a, [hGbc]
	and a
	call nz, ClearAttrs
ENDC
	ld hl, song_title
	call MusicPlay
	ld a, LCDC_ON | LCDC_BG_ON | LCDC_BLOCK01 | LCDC_BG_9800
	ldh [rLCDC], a
	ld a, IE_VBLANK
	ldh [rIE], a
	xor a
	ldh [rIF], a
	ld [wPressed], a
	ei
.wait
	halt
	nop
	ld a, [wPressed]
	and PAD_START
	jr z, .wait
	ld a, SFX_START
	call SfxPlay
	di
	jp StartMaze
