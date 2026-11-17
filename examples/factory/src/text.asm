; Text in the font assets/fonts/main.yaff, which `gbcli build` turns into
; MainFontGlyphs (8 bytes a glyph, 1bpp), MainFontWidths (a byte a glyph)
; and the charmap macro MAIN_FONT_CHARMAP in images.inc. `gbcli help fonts`
; states the layout rules TextDraw follows; `gbcli font assets/fonts/main.yaff
; --sample TEXT` sets TEXT by the same rules, so its pixels are TextDraw's.
;
; Both routines write tiles: give them VRAM only with the LCD off, or draw
; into a WRAM buffer and copy it in VBlank. VRAM is closed to the CPU while
; the LCD draws (`gbcli help traps`), and a TextDraw runs far past VBlank's
; 1,140 M-cycles (a glyph takes up to 954). Only TextLoadTiles, 360 M-cycles
; a tile, fits VRAM in VBlank, for two or three tiles.
; The font is in ROMX: on a cartridge with an MBC, switch to
; BANK(MainFontGlyphs) first.

include "hardware.inc"
include "images.inc"

SECTION "Text", ROM0

; Draws a line of text, variable width, into consecutive tiles.
; In: hl a string of glyph numbers (`MAIN_FONT_CHARMAP text, 0`, then
;     PUSHC text / db "Hi", MAIN_FONT_END / POPC), ended by MAIN_FONT_END or
;     MAIN_FONT_NEWLINE; de the line's first tile; b the ink shade and c the
;     paper shade, 0-3.
; Out: a the tiles the line touched, (width + 7) / 8, each wholly written:
;     paper wherever no glyph inks it. hl the byte after the string's end,
;     so after a MAIN_FONT_NEWLINE it is the next line's string.
; A glyph number past the font's draws nothing and takes no room, as a
; character `gbcli font --sample` lacks. Tiles past the line keep what they
; held. Clobbers bc and de.
TextDraw::
	push hl
	call TextShades
	pop hl
	ld a, l
	ld [wTextString], a
	ld a, h
	ld [wTextString + 1], a
	ld hl, wTextTile
	ld a, e
	ld [hli], a                 ; wTextTile: the tile the pen is in
	ld a, d
	ld [hli], a
	ld a, e
	ld [hli], a                 ; wTextNext: the first tile not yet papered
	ld a, d
	ld [hli], a
	ld a, e
	ld [hli], a                 ; wTextFirst
	ld a, d
	ld [hli], a
	xor a
	ld [hl], a                  ; wTextPen: the pen's column in its tile, 0-7

.glyph
	ld hl, wTextString
	ld a, [hli]
	ld h, [hl]
	ld l, a
	ld a, [hli]
	ld b, a
	ld a, l
	ld [wTextString], a
	ld a, h
	ld [wTextString + 1], a
	ld a, b
	cp MAIN_FONT_NEWLINE        ; MAIN_FONT_END is above it
	jp nc, .done
	cp MAIN_FONT_GLYPHS
	jr nc, .glyph

	; The glyph's width and rows.
	ld e, a
	ld d, 0
	ld hl, MainFontWidths
	add hl, de
	ld a, [hl]
	ld [wTextWidth], a
	ld h, d
	ld l, e
	add hl, hl
	add hl, hl
	add hl, hl
	ld de, MainFontGlyphs
	add hl, de

	; Rotate each row right by the pen's column into wTextRows: a row's
	; first-tile pixels are then the bits under M1 below, its next-tile
	; pixels the bits under M2. The jump enters the rrca run so that as
	; many rrca as the column run.
	ld d, h
	ld e, l
	ld a, [wTextPen]
	ld b, a
	ld a, LOW(.rotated)
	sub b
	ld l, a
	ld a, HIGH(.rotated)
	sbc 0
	ld h, a
	ld bc, wTextRows
.row
	ld a, [de]
	inc de
	jp hl
	REPT 7
	rrca
	ENDR
.rotated
	ld [bc], a
	inc c
	ld a, c
	and 7
	jr nz, .row

	; The glyph's columns, m = $FF << (8 - width), rotated the same way:
	; M1 = its columns in the pen's tile, M2 = those in the next one.
	ld a, [wTextWidth]
	dec a
	ld e, a
	ld d, 0
	ld hl, TextWidthMasks
	add hl, de
	ld c, [hl]
	ld a, [wTextPen]
	ld e, a
	ld hl, TextRightOf
	add hl, de
	ld b, [hl]                  ; $FF >> column
	ld a, c
	inc e
	jr .rotateMask
.rotateMaskLoop
	rrca
.rotateMask
	dec e
	jr nz, .rotateMaskLoop
	ld c, a
	and b
	ld [wTextM1], a
	ld a, b
	cpl
	and c
	ld [wTextM2], a

	; Draw into the pen's tile, then into the next when the glyph reaches it.
	ld a, [wTextM1]
	call TextPlaneMasks
	ld hl, wTextTile
	ld a, [hli]
	ld h, [hl]
	ld l, a
	push hl
	call TextDrawTile
	pop hl
	ld a, [wTextM2]
	and a
	jr z, .advance
	call TextPlaneMasks
	ld de, 16
	add hl, de
	call TextDrawTile

	; The pen moves right by the width, into the next tile past column 7.
.advance
	ld a, [wTextWidth]
	ld b, a
	ld a, [wTextPen]
	add b
	cp 8
	jr c, .samePen
	sub 8
	ld [wTextPen], a
	ld hl, wTextTile
	ld a, [hl]
	add 16
	ld [hli], a
	jp nc, .glyph
	inc [hl]
	jp .glyph
.samePen
	ld [wTextPen], a
	jp .glyph

	; The tiles used are those papered: (wTextNext - wTextFirst) / 16.
.done
	ld hl, wTextFirst
	ld a, [hli]
	ld e, a
	ld d, [hl]
	ld hl, wTextNext
	ld a, [hli]
	ld h, [hl]
	sub e
	ld l, a
	ld a, h
	sbc d
	ld h, a
	add hl, hl
	add hl, hl
	add hl, hl
	add hl, hl
	ld b, h
	ld hl, wTextString
	ld a, [hli]
	ld h, [hl]
	ld l, a
	ld a, b
	ret

; Into one tile at hl, rows from wTextRows: each plane's row ^= row & mask,
; with b the mask for plane 0 and c for plane 1. The glyph's columns are
; still paper (the pen only moves right), so flipping the planes where ink
; and paper differ draws ink. A tile the line hasn't reached is papered
; first. Keeps hl; clobbers a and de.
TextDrawTile:
	ld a, [wTextNext]
	cp l
	jr nz, .papered
	ld a, [wTextNext + 1]
	cp h
	jr nz, .papered
	push bc
	push hl
	ld a, [wTextPaper0]
	ld b, a
	ld a, [wTextPaper1]
	ld c, a
	ld e, 8
.paper
	ld a, b
	ld [hli], a
	ld a, c
	ld [hli], a
	dec e
	jr nz, .paper
	ld a, l
	ld [wTextNext], a
	ld a, h
	ld [wTextNext + 1], a
	pop hl
	pop bc
.papered
	push hl
	ld de, wTextRows
.row
	ld a, [de]
	and b
	xor [hl]
	ld [hli], a
	ld a, [de]
	inc e
	and c
	xor [hl]
	ld [hli], a
	ld a, e
	and 7
	jr nz, .row
	pop hl
	ret

; b = a & the plane-0 flip, c = a & the plane-1 flip.
TextPlaneMasks:
	ld c, a
	ld a, [wTextFlip0]
	and c
	ld b, a
	ld a, [wTextFlip1]
	and c
	ld c, a
	ret

; From b (ink) and c (paper), each plane's flip ($FF where ink and paper
; differ in it) and paper ($FF where paper sets it). Clobbers a and hl.
TextShades:
	ld hl, wTextFlip0
	ld a, b
	xor c
	rrca                        ; bit 0 into carry
	sbc a                       ; $FF if it was set
	ld [hli], a
	ld a, b
	xor c
	rrca
	rrca                        ; bit 1 into carry
	sbc a
	ld [hli], a
	ld a, c
	rrca
	sbc a
	ld [hli], a                 ; wTextPaper0
	ld a, c
	rrca
	rrca
	sbc a
	ld [hl], a                  ; wTextPaper1
	ret

; Expands 1bpp glyphs into 2bpp tiles, ink where a bit is set and paper
; elsewhere: a font loaded as tiles, for text written straight into the
; tilemap (MAIN_FONT_CHARMAP hud, $80 for glyphs loaded at tile $80). Every
; glyph sits at its tile's left edge, so it suits a fixed-width font.
; In: de the first glyph's rows (MainFontGlyphs + 8 * its number), hl the
;     first tile, a how many glyphs (0 loads none), b ink and c paper, 0-3.
; Out: hl past the last tile, de past the last glyph, a 0. Clobbers bc.
TextLoadTiles::
	and a
	ret z
	push af
	push hl
	call TextShades
	pop hl
	pop af
	ld c, a
	ld b, 0
	REPT 3
	sla c
	rl b
	ENDR
.row
	ld a, [de]
	inc de
	push de
	ld d, a
	ld a, [wTextFlip0]
	and d
	ld e, a
	ld a, [wTextPaper0]
	xor e
	ld [hli], a
	ld a, [wTextFlip1]
	and d
	ld e, a
	ld a, [wTextPaper1]
	xor e
	ld [hli], a
	pop de
	dec bc
	ld a, b
	or c
	jr nz, .row
	ret

; m = $FF << (8 - width), by width - 1.
TextWidthMasks:
	db $80, $C0, $E0, $F0, $F8, $FC, $FE, $FF

; $FF >> column, by column.
TextRightOf:
	db $FF, $7F, $3F, $1F, $0F, $07, $03, $01

SECTION "Text variables", WRAM0

wTextFlip0: db
wTextFlip1: db
wTextPaper0: db
wTextPaper1: db
wTextTile: dw
wTextNext: dw
wTextFirst: dw
wTextPen: db
wTextString: dw
wTextWidth: db
wTextM1: db
wTextM2: db

SECTION "Text rows", WRAM0, ALIGN[3]

; The glyph's rows, rotated by the pen's column. Aligned to 8, so the low
; three bits of the address count the rows.
wTextRows: ds 8
