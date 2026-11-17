; The results screen: this round's score, the personal best and "New
; best!", then START for another round. Before the first round it shows
; only the title and the best (PLACEHOLDER: Factory 4 makes the title).

include "hardware.inc"
include "layout.inc"
include "images.inc"
include "bank.inc"

	MAIN_FONT_CHARMAP text, 0
	PUSHC text
DEF GLYPH_ZERO EQU '0'
	POPC

DEF LINE_TILES EQU 20

SECTION "Results", ROM0

; Draws the screen with the LCD off and turns it back on: no window, no
; splits, no objects. wResultsAfterRound says which version. Interrupts
; are off while the LCD is.
ShowResults::
	call LcdOff
	ld hl, TILEMAP0
	ld bc, TILEMAP_AREA
	call ClearMem               ; tile 0: blank wall
	ld hl, TILEBLOCK0 + T_RESULTS * 16
	ld bc, 5 * LINE_TILES * 16
	call ClearMem
	ld hl, wShadowOAM
	ld bc, OAM_SIZE
	call ClearMem
	SWITCH_BANK MainFontGlyphs

	ld hl, TitleText
	ld bc, 0 << 8 | 3           ; buffer 0, map row 3
	call DrawLine
	ld a, [wResultsAfterRound]
	and a
	jr z, .best
	ld hl, MadeText
	ld de, wScore
	call Compose
	ld hl, wLine
	ld bc, 1 << 8 | 7
	call DrawLine
.best
	ld hl, BestText
	ld de, wBest
	call Compose
	ld hl, wLine
	ld bc, 2 << 8 | 9
	call DrawLine
	ld a, [wNewBest]
	and a
	jr z, .start
	ld hl, NewBestText
	ld bc, 3 << 8 | 11
	call DrawLine
.start
	ld hl, StartText
	ld bc, 4 << 8 | 14
	call DrawLine

	xor a
	ldh [rSCX], a
	ld a, IE_VBLANK
	ldh [rIE], a
	ld a, LCDC_ON | LCDC_BG_ON | LCDC_BLOCK01 | LCDC_BG_9800
	ldh [rLCDC], a
	xor a
	ldh [rIF], a
	reti                        ; returns with interrupts on

; Turns the LCD off in VBlank and leaves interrupts off.
LcdOff::
	di
	ldh a, [rLCDC]
	bit B_LCDC_ENABLE, a
	ret z
.wait
	ldh a, [rLY]
	cp LY_VBLANK
	jr c, .wait
	xor a
	ldh [rLCDC], a
	ret

; Draws string hl into line buffer b, centered on map row c.
DrawLine:
	push bc
	ld a, b
	add a
	add a
	add b                       ; * 5
	add a
	add a                       ; * 20
	add T_RESULTS
	ld e, a
	ld d, 0
	sla e
	rl d
	sla e
	rl d
	sla e
	rl d
	sla e
	rl d                        ; tile * 16
	ld a, d
	add HIGH(TILEBLOCK0)
	ld d, a
	ld b, 3                     ; ink
	ld c, 0                     ; paper
	call TextDraw               ; a: the tiles it touched
	pop bc
	ld e, a                     ; e: tiles
	ld a, LINE_TILES
	sub e
	srl a                       ; first column, centered
	ld l, c
	ld h, 0
	add hl, hl
	add hl, hl
	add hl, hl
	add hl, hl
	add hl, hl                  ; row * 32
	add l
	ld l, a
	ld a, h
	adc HIGH(TILEMAP0)
	ld h, a
	; tiles: buffer b's first tile onward
	ld a, b
	add a
	add a
	add b
	add a
	add a
	add T_RESULTS
.map
	ld [hli], a
	inc a
	dec e
	jr nz, .map
	ret

; wLine = string hl (without its end), then the BCD number at de with no
; leading zeros, then MAIN_FONT_END.
Compose:
	push de
	ld de, wLine
.copy
	ld a, [hli]
	cp MAIN_FONT_END
	jr z, .number
	ld [de], a
	inc de
	jr .copy
.number
	pop hl                      ; the number: low byte, then high
	ld a, [hli]
	ld c, a                     ; tens and ones
	ld a, [hl]
	and $0F                     ; hundreds
	ld b, 0                     ; 0 until a digit is written
	jr z, .tens
	call .digit
	inc b
.tens
	ld a, c
	swap a
	and $0F
	jr nz, .writeTens
	bit 0, b
	jr z, .ones
.writeTens
	call .digit
.ones
	ld a, c
	and $0F
	call .digit
	ld a, MAIN_FONT_END
	ld [de], a
	ret
.digit
	add GLYPH_ZERO
	ld [de], a
	inc de
	ret

	PUSHC text
TitleText:   db "Teddy & Toys", MAIN_FONT_END
MadeText:    db "Toys made: ", MAIN_FONT_END
BestText:    db "Best: ", MAIN_FONT_END
NewBestText: db "New best!", MAIN_FONT_END
StartText:   db "Press START", MAIN_FONT_END
	POPC

SECTION "Results variables", WRAM0
wResultsAfterRound:: db            ; 0 before the first round: title and best only
wNewBest:: db                      ; 1 if the round just played beat the best
wLine: ds 24                       ; a composed line of text
