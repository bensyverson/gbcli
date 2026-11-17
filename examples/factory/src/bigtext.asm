; Text at twice the font's size, for the results screen: TextDraw sets
; the line in a scratch buffer, then each of its tiles is drawn doubled
; into 2x2 tiles. VRAM only, so the LCD must be off.

include "hardware.inc"
include "layout.inc"

DEF SCRATCH_TILE EQU T_SCRATCH
DEF BIG_MAX_TILES EQU 10            ; a line's source tiles: 20 columns doubled

SECTION "Big text", ROM0

; Starts a screen's big lines at tile T_BIG.
BigTextInit::
	ld a, T_BIG
	ld [wBigNext], a
	ret

; Draws string hl (MAIN_FONT_END ends it) at twice the size, centered on
; map rows c and c + 1. The font's bank must be in. Clobbers everything.
DrawBig::
	push bc
	ld de, TILEBLOCK0 + SCRATCH_TILE * 16
	ld b, COLOR_INK
	ld c, COLOR_WHITE           ; paper
	call TextDraw               ; a: n, the tiles it touched
	cp BIG_MAX_TILES + 1
	jr c, .fits
	ld a, BIG_MAX_TILES         ; too wide: its right end is cut
.fits
	ld [wBigCount], a
	; Double the n scratch tiles into the 4n tiles from wBigNext.
	ld b, a
	ld a, [wBigNext]
	call TileAddress
	ld hl, TILEBLOCK0 + SCRATCH_TILE * 16
	call Scale2x
	; The map: top halves on row c, bottom halves on row c + 1.
	pop bc
	ld a, [wBigCount]
	add a
	ld b, a                     ; b: 2n columns
	ld a, SCREEN_WIDTH
	sub b
	srl a                       ; the first column, centered
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
	ld a, [wBigNext]
	call .row
	ld d, a                     ; the bottom halves' first tile
	ld a, TILEMAP_WIDTH
	sub b                       ; hl: the row below, the same first column
	add l
	ld l, a
	adc h
	sub l
	ld h, a
	ld a, d
	call .row
	ld [wBigNext], a
	ret
; Writes b tiles a, a + 1 ... at hl; a and hl end past them. Keeps b.
.row
	ld c, b
.tile
	ld [hli], a
	inc a
	dec c
	jr nz, .tile
	ret

; de = the VRAM address of tile a ($8000 addressing). Keeps b.
TileAddress:
	ld e, a
	ld d, 0
	REPT 4
	sla e
	rl d
	ENDR
	ld a, d
	add HIGH(TILEBLOCK0)
	ld d, a
	ret

; Draws b tiles at hl doubled, as 2b x 2 tiles at de: the top halves'
; 2b tiles, then the bottom halves' 2b.
Scale2x:
	ld a, b
	ld [wBigLeft], a
	; The bottom row is b * 32 bytes after the top.
	push hl
	ld l, b
	ld h, 0
	REPT 5
	add hl, hl
	ENDR
	ld a, l
	ld [wBigBottom], a
	ld a, h
	ld [wBigBottom + 1], a
	pop hl
.tile
	push de
	call ScaleHalf              ; rows 0-3 to the top half
	pop de
	push de
	push hl
	ld a, [wBigBottom]
	ld l, a
	ld a, [wBigBottom + 1]
	ld h, a
	add hl, de
	ld d, h
	ld e, l
	pop hl
	call ScaleHalf              ; rows 4-7 to the bottom half
	pop de
	ld a, e
	add 32                      ; the next 2 tiles
	ld e, a
	adc d
	sub e
	ld d, a
	ld a, [wBigLeft]
	dec a
	ld [wBigLeft], a
	jr nz, .tile
	ret

; Four rows of a 2bpp tile at hl, each drawn twice and twice as wide, into
; the tile at de (left halves) and the tile after it (right halves). hl
; advances 8.
ScaleHalf:
	ld c, 4
.row
	ld a, [hli]
	ld b, a                     ; plane 0
	ld a, [hli]
	ld [wBigPlane1], a
	push hl
	push bc
	; The left tile: each plane's high nibble.
	ld a, b
	swap a
	call Double
	ld b, a
	ld a, [wBigPlane1]
	swap a
	call Double
	ld c, a
	ld h, d
	ld l, e
	call .put
	; The right tile, 16 bytes on: the low nibbles.
	pop bc
	push bc
	ld a, b
	call Double
	ld b, a
	ld a, [wBigPlane1]
	call Double
	ld c, a
	ld hl, 16
	add hl, de
	call .put
	pop bc
	pop hl
	ld a, e
	add 4                       ; two rows down
	ld e, a
	adc d
	sub e
	ld d, a
	dec c
	jr nz, .row
	ret
; Two rows of planes b, c at hl.
.put
	ld a, b
	ld [hli], a
	ld a, c
	ld [hli], a
	ld a, b
	ld [hli], a
	ld [hl], c
	ret

; a = the low nibble of a with each bit doubled ($5 -> $33). Keeps bc, de.
Double:
	and $0F
	push hl
	add LOW(DoubleTable)
	ld l, a
	ld h, HIGH(DoubleTable)
	ld a, [hl]
	pop hl
	ret

SECTION "Double table", ROM0, ALIGN[4]
DoubleTable:
	FOR N, 16
		db ((N >> 3) & 1) * $C0 | ((N >> 2) & 1) * $30 | ((N >> 1) & 1) * $0C | (N & 1) * $03
	ENDR

SECTION "Big text variables", WRAM0
wBigNext: db                       ; the next big line's first tile
wBigCount: db                      ; the line being drawn: its source tiles
wBigLeft: db                       ; Scale2x: source tiles still to draw
wBigBottom: dw                     ; Scale2x: bytes from a top tile to the one below
wBigPlane1: db                     ; ScaleHalf: the row's plane 1
