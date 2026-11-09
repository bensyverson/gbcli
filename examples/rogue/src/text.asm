; Text: the status line in the window, and the end screens.

include "hardware.inc"
include "game.inc"

; The font's characters, in tools/font.py's CHARS order.
DEF FONT_CHARS EQUS " 0123456789ABCDEFGHIJKLMNOPQRSTUVWXYZ/!:-.'"
FOR I, STRLEN("{FONT_CHARS}")
	CHARMAP STRSLICE("{FONT_CHARS}", I, I + 1), FONT_BASE + I
ENDR
DEF FONT_SPACE EQU FONT_BASE
DEF FONT_DIGITS EQU FONT_BASE + 1

DEF STATUS_LEN EQU 20

SECTION "Text", ROM0

; Fills the window map with spaces. LCD off.
ClearWindow::
	ld hl, TILEMAP1
	ld bc, $400
	ld a, FONT_SPACE
	jp FillMem

; Rebuilds the status line: "HP 20/20 ATK 2 FL 1", and marks it for the
; VBlank handler.
UpdateStatus::
	ld hl, wStatusBuf
	ld de, StatusTemplate
	ld b, STATUS_LEN
.copy
	ld a, [de]
	ld [hli], a
	inc de
	dec b
	jr nz, .copy
	ld a, [wHP]
	ld hl, wStatusBuf + 3
	call PutNumber
	ld a, [wMaxHP]
	ld hl, wStatusBuf + 6
	call PutNumber
	ld a, [wAtk]
	ld hl, wStatusBuf + 12
	call PutNumber
	ld a, [wDepth]
	inc a
	ld hl, wStatusBuf + 17
	call PutNumber
	ld a, 1
	ld [wStatusDirty], a
	ret

StatusTemplate:
	db "HP   /   ATK   FL   "

; Writes A (0-99) at HL as two characters, a space for a leading zero.
PutNumber:
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
	and a
	ld a, FONT_SPACE
	jr z, .space
	ld a, b
	add FONT_DIGITS
.space
	ld [hli], a
	ld a, c
	add FONT_DIGITS
	ld [hl], a
	ret

; Called by the VBlank handler: copies the status line if it changed.
FlushStatus::
	ld a, [wStatusDirty]
	and a
	ret z
	xor a
	ld [wStatusDirty], a
	ld hl, wStatusBuf
	ld de, TILEMAP1
	REPT STATUS_LEN
	ld a, [hli]
	ld [de], a
	inc e
	ENDR
	ret

; Shows the end screen for wGameState over the whole window, then waits for
; START and restarts. The LCD goes off for the redraw, in VBlank.
ShowEnd::
	call WaitFrame
	di
.waitVBlank
	ldh a, [rLY]
	cp LY_VBLANK
	jr c, .waitVBlank
	xor a
	ldh [rLCDC], a
	call ClearWindow
	ld a, [wGameState]
	cp STATE_WON
	ld de, WonText
	jr z, .write
	ld de, DeadText
.write
	ld hl, TILEMAP1 + 6 * 32
	call WriteLines
	; The depth reached.
	ld a, [wDepth]
	inc a
	ld hl, TILEMAP1 + 12 * 32 + 12
	call PutNumber
	xor a
	ldh [rWY], a
	ld a, WX_OFS
	ldh [rWX], a
	ld a, LCDC_ON | LCDC_BG_ON | LCDC_OBJ_OFF | LCDC_BLOCK01 | LCDC_BG_9800 | LCDC_WIN_ON | LCDC_WIN_9C00
	ldh [rLCDC], a
	xor a
	ldh [rIF], a
	ei
.wait
	call WaitFrame
	call ReadJoypad
	ld a, [wPadNew]
	and PAD_START
	jr z, .wait
	jp Start

; Writes lines from DE at HL, a row (32 cells) apart: each line ends in 1, the
; text in 0. LCD off.
WriteLines:
	push hl
.char
	ld a, [de]
	inc de
	and a
	jr z, .end
	cp 1
	jr z, .line
	ld [hli], a
	jr .char
.line
	pop hl
	ld bc, 32
	add hl, bc
	push hl
	jr .char
.end
	pop hl
	ret

WonText:
	db "   YOU HOLD THE", 1
	db "      EMBER!", 1, 1
	db "  THE DEEP IS LIT", 1
	db "   AT LAST.", 1, 1
	db "      FLOOR", 1, 1, 1
	db "   PRESS START", 0

DeadText:
	db "  YOUR LANTERN", 1
	db "   GUTTERS OUT.", 1, 1
	db "  YOU FELL ON", 1, 1, 1
	db "      FLOOR", 1, 1, 1
	db "   PRESS START", 0

SECTION "Text variables", WRAM0
wStatusBuf: ds STATUS_LEN
wStatusDirty: db
