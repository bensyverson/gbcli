; The results screen: this round's score, the personal best and "New
; best!", then START for another round. Before the first round it shows
; only the title and the best (PLACEHOLDER: Factory 4 makes the title).

include "hardware.inc"
include "layout.inc"
include "images.inc"
include "bank.inc"
include "gen/toys.inc"

	MAIN_FONT_CHARMAP text, 0
	PUSHC text
DEF GLYPH_ZERO EQU '0'
	POPC

DEF LINE_TILES EQU RESULTS_LINE_TILES

; Map rows (a big line takes two).
DEF TITLE_NAME_ROW   EQU 2
DEF TITLE_BEST_ROW   EQU 12
DEF RESULTS_HEAD_ROW EQU 1
DEF RESULTS_MADE_ROW EQU 3
DEF RESULTS_BEST_ROW EQU 6
DEF RESULTS_NEW_ROW  EQU 9
DEF RESULTS_BELT_ROW EQU 11         ; 4 rows
DEF START_ROW        EQU 16
DEF TOY_BELT_COL     EQU 1          ; the toys on the belt: the first one's column
DEF TOY_BELT_STEP    EQU 3          ; columns from one toy to the next

SECTION "Results", ROM0

; Draws the screen with the LCD off and turns it back on: no window, no
; splits, no objects. wResultsAfterRound says which version. Interrupts
; are off while the LCD is.
ShowResults::
	call LcdOff
IF DEF(GBCLI_GBC)
	ldh a, [hGbc]
	and a
	ld a, [wResultsAfterRound]
	call nz, ColorGbcScreen
ENDC
	call LoadSceneTiles
	ld hl, TILEMAP0
	ld bc, TILEMAP_AREA
	call ClearMem               ; T_BLANK: white
	ld hl, wShadowOAM
	ld bc, OAM_SIZE
	call ClearMem
	SWITCH_BANK MainFontGlyphs
	call BigTextInit
	ld a, [wResultsAfterRound]
	and a
	jr nz, .results

	; The title: the music at the song's tempo again, the name big, the
	; six toys riding a belt, the best.
	call MusicNormal
	ld hl, TitleText
	ld c, TITLE_NAME_ROW
	call DrawBig
	ld a, TITLE_BELT_ROW
	call DrawToyBelt
	ld hl, BestText
	ld de, wBest
	call Compose
	ld hl, wLine
	ld bc, 0 << 8 | TITLE_BEST_ROW  ; small line buffer 0
	call DrawLine
	jr .start

	; After a round: the score, the best and "New best!" big, 2 rows each.
.results
	ld hl, TimeUpText
	ld bc, 0 << 8 | RESULTS_HEAD_ROW
	call DrawLine
	ld hl, MadeText
	ld de, wScore
	call Compose
	ld hl, wLine
	ld c, RESULTS_MADE_ROW
	call DrawBig
	ld hl, BestText
	ld de, wBest
	call Compose
	ld hl, wLine
	ld c, RESULTS_BEST_ROW
	call DrawBig
	ld a, [wNewBest]
	and a
	jr z, .belt
	ld hl, NewBestText
	ld c, RESULTS_NEW_ROW
	call DrawBig
.belt
	ld a, RESULTS_BELT_ROW
	call DrawToyBelt
.start
	ld hl, StartText
	ld bc, 1 << 8 | START_ROW   ; small line buffer 1
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

; A belt band (4 rows) from map row a, with the six finished toys on it,
; as in the key column, top toy leftmost. On a GBC, in the belt's colors
; and each toy's.
DrawToyBelt:
	ld l, a
	ld h, 0
	REPT 5
	add hl, hl
	ENDR
	ld de, TILEMAP0
	add hl, de
	push hl
	call DrawBeltRows
	pop hl
IF DEF(GBCLI_GBC)
	ldh a, [hGbc]
	and a
	jr z, .colored
	push hl
	ld a, 1
	ldh [rVBK], a
	ld bc, 4 * 32
	ld a, PAL_SCENE
	call FillMem
	xor a
	ldh [rVBK], a
	pop hl
.colored
ENDC
	ld de, 32 + TOY_BELT_COL     ; the toys' row, the first toy's column
	add hl, de
	ld c, 0
.toy
	ld a, c
	call ToyTile
	add 8                       ; the finished frame
	call Draw2x2                ; keeps hl
IF DEF(GBCLI_GBC)
	ld a, c
	inc a
	call SlotColors             ; keeps bc, hl
ENDC
	ld de, TOY_BELT_STEP
	add hl, de
	inc c
	ld a, c
	cp TOY_COUNT
	jr nz, .toy
	ret

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
	ld b, COLOR_INK
	ld c, COLOR_WHITE           ; paper
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
TimeUpText:  db "Time's up!", MAIN_FONT_END
MadeText:    db "Toys made: ", MAIN_FONT_END
BestText:    db "Best: ", MAIN_FONT_END
NewBestText: db "New best!", MAIN_FONT_END
StartText:   db "Press START", MAIN_FONT_END
	POPC

SECTION "Results variables", WRAM0
wResultsAfterRound:: db            ; 0 before the first round: title and best only
wNewBest:: db                      ; 1 if the round just played beat the best
wLine: ds 24                       ; a composed line of text
