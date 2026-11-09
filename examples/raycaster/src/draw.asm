; Drawing: wHH and wXBits into the draw buffer, one tile column (4 rays) at
; a time, and the view's map and copy list beside it.
;
; A tile column's byte pair for a row depends only on which of its four rays
; are inside their wall on that row. Walls are centered on the horizon, so in
; the top half a ray turns from ceiling to wall on row HALF_H - hh and stays
; wall; sorting the four rays by that row splits the top half into five spans
; of identical rows. The bottom half mirrors the top: the same spans in
; reverse, with floor in place of ceiling.
;
; For a row whose wall rays have pair bits M, with L and H the pair bits of
; the column's rays whose wall shade has bit 0 and bit 1 set:
;   above the horizon  low = M & L,         high = M & H  (ceiling: shade 0)
;   below the horizon  low = (M & L) | ~M,  high = M & H  (floor: shade 1)
;
; Only edge tiles (a wall starts inside them) are written and copied. Tile
; rows above the first edge are all ceiling (floor below the horizon), rows
; past the last are all wall: the view's map points those at shared tiles.
; Shared wall tiles exist for stone only (H = $FF: shades 2 and 3); in a
; column with the door, the wall rows are written and copied too.

include "hardware.inc"
include "defs.inc"

; Sorts registers \1 and \2 so that \1 <= \2 (unsigned).
MACRO SORT2
	ld a, \2
	cp \1
	jr nc, .sorted\@
	ld \2, \1
	ld \1, a
.sorted\@
ENDM

; The key for one ray: (HALF_H - hh) * 4 + its index in the tile column, so
; the keys sort by the row the wall starts on and still name their ray.
MACRO RAYKEY
	ld a, [hli]
	cpl
	add HALF_H + 1
	add a
	add a
	IF \1 != 0
		add \1
	ENDC
ENDM

; hM\2 = hM\1 | the pair bits of the ray keyed in register \3.
MACRO ADDRAY
	ld a, \3
	and 3
	add LOW(PairMask)
	ld l, a
	ldh a, [hM\1]
	or [hl]
	ldh [hM\2], a
ENDM

; Fills span \1's rows (count hC\1, wall bits hM\1) above the horizon;
; C holds L.
MACRO TOPSPAN
	ldh a, [hC\1]
	ld b, a
	ldh a, [hColHi]
	ld e, a
	ldh a, [hM\1]
	and e
	ld e, a                     ; high = M & H
	ldh a, [hM\1]
	and c
	ld d, a                     ; low = M & L
	call FillRows
ENDM

; The same span's mirror below the horizon.
MACRO BOTTOMSPAN
	ldh a, [hC\1]
	ld b, a
	ldh a, [hColHi]
	ld e, a
	ldh a, [hM\1]
	and e
	ld e, a                     ; high = M & H
	ldh a, [hM\1]
	cpl
	ld d, a
	ldh a, [hM\1]
	and c
	or d
	ld d, a                     ; low = (M & L) | ~M
	call FillRows
ENDM

SECTION "Draw", ROM0

DrawView::
	xor a
	ldh [hListLen], a
	ldh [hDirty], a
	ldh [hM0], a
	dec a
	ldh [hM4], a
	xor a
.col
	ld [wCol], a                ; the tile column's first ray (tx * 4)
	ld l, a
	ld h, HIGH(wHH)
	RAYKEY 0
	ld b, a
	RAYKEY 1
	ld c, a
	RAYKEY 2
	ld d, a
	RAYKEY 3
	ld e, a
	dec l
	inc h                       ; wLoBits
	ld a, [hld]
	or [hl]
	dec l
	or [hl]
	dec l
	or [hl]
	ldh [hColLo], a
	inc h                       ; wHiBits
	ld a, [hli]
	or [hl]
	inc l
	or [hl]
	inc l
	or [hl]
	ldh [hColHi], a
	SORT2 b, c
	SORT2 d, e
	SORT2 b, d
	SORT2 c, e
	SORT2 c, d

	; The wall bits of each span: none, then each ray in the order its wall
	; starts, then all four.
	ld h, HIGH(PairMask)
	ADDRAY 0, 1, b
	ADDRAY 1, 2, c
	ADDRAY 2, 3, d
	; The rows: each key's row, and the spans' lengths between them, cut to
	; the edge tiles (from tile row s0 / 8 up to tile row (s3 + 7) / 8).
	srl b
	srl b
	srl c
	srl c
	srl d
	srl d
	srl e
	srl e
	ld a, b
	and 7
	ldh [hC0], a
	ld a, c
	sub b
	ldh [hC1], a
	ld a, d
	sub c
	ldh [hC2], a
	ld a, e
	sub d
	ldh [hC3], a
	ld a, b
	srl a
	srl a
	srl a
	ldh [hT0], a                ; the first edge tile row
	ldh a, [hColHi]
	inc a
	ld a, VIEW_TH / 2           ; not all stone: every row to the horizon
	jr nz, .gotT1
	ld a, e
	add 7
	srl a
	srl a
	srl a
.gotT1
	ldh [hT1], a                ; the first all-wall tile row after the edges
	add a
	add a
	add a
	sub e
	ldh [hC4], a                ; the last span, to that row

	; The view's map for this tile column, and its edge tiles on the list.
	ldh a, [hColLo]
	ld l, a
	ld h, HIGH(XToTile)
	ld a, [hl]
	ldh [hWallTile], a
	ld a, [wCol]
	srl a
	srl a
	ldh [hTX], a
	ld a, 1
	ldh [hRowBitTop], a         ; map row c's bit in hDirty
	ld a, $80
	ldh [hRowBitBottom], a      ; map row 7 - c's
	ld c, 0                     ; the tile row, 0-3
.classify
	; HL = the map entry for row c; E = the low byte of row 7 - c's.
	ldh a, [hTX]
	ld e, a
	ld a, c
	swap a
	add e
	ld l, a
	ldh a, [hDrawMeta]
	ld h, a
	ld a, 7
	sub c
	swap a
	add e
	ld e, a
	ldh a, [hT0]
	ld b, a
	ld a, c
	cp b
	jr c, .ceilingRow           ; c < T0
	ldh a, [hT1]
	ld b, a
	ld a, c
	cp b
	jr nc, .wallRow             ; c >= T1
	; An edge: own tiles tx * 8 + c and tx * 8 + 7 - c, both on the list.
	ldh a, [hTX]
	add a
	add a
	add a
	ld d, a
	add c
	ld b, a
	ld a, d
	add 7
	sub c
	ld d, a
	ldh a, [hListLen]
	push hl
	add LIST_START
	ld l, a
	ld [hl], b
	inc l
	ld [hl], d
	pop hl
	ldh a, [hListLen]
	add 2
	ldh [hListLen], a
	jr .putMap
.ceilingRow
	ld b, TILE_BLANK
	ld d, TILE_SHADE1
	jr .putMap
.wallRow
	ldh a, [hWallTile]
	ld b, a
	ld d, a
.putMap
	; The map entries, B at HL and D at E. The meta page still holds this
	; buffer's previous map, which is what the back map shows: a row with a
	; changed entry is marked for the copy, the rest are not copied.
	ld a, [hl]
	cp b
	jr z, .topSame
	ld [hl], b
	ldh a, [hRowBitTop]
	ld b, a
	ldh a, [hDirty]
	or b
	ldh [hDirty], a
.topSame
	ld l, e
	ld a, [hl]
	cp d
	jr z, .classified
	ld [hl], d
	ldh a, [hRowBitBottom]
	ld b, a
	ldh a, [hDirty]
	or b
	ldh [hDirty], a
.classified
	ldh a, [hRowBitTop]
	add a
	ldh [hRowBitTop], a
	ldh a, [hRowBitBottom]
	srl a
	ldh [hRowBitBottom], a
	inc c
	ld a, c
	cp VIEW_TH / 2
	jr nz, .classify

	; The tile column in the draw buffer: its start + tx * 128.
	ldh a, [hTX]
	rrca                        ; (tx >> 1) | (tx & 1) << 7
	ld e, a
	and $80
	ld l, a
	ldh a, [hDrawBuf]
	ld b, a
	ld a, e
	and $7F
	add b
	ld h, a
	push hl
	; Above the horizon, from the first edge tile row down.
	ldh a, [hT0]
	swap a                      ; 16 bytes a tile row
	add l
	ld l, a
	ldh a, [hColLo]
	ld c, a
	TOPSPAN 0
	TOPSPAN 1
	TOPSPAN 2
	TOPSPAN 3
	TOPSPAN 4
	; Below the horizon, from the mirror of the last edge tile row down.
	pop hl
	ldh a, [hT1]
	cpl
	add VIEW_TH + 1             ; 8 - T1
	swap a
	add l
	ld l, a
	BOTTOMSPAN 4
	BOTTOMSPAN 3
	BOTTOMSPAN 2
	BOTTOMSPAN 1
	BOTTOMSPAN 0

	ld a, [wCol]
	add 4
	cp RAYS
	jp nz, .col
	ret

; Writes B rows (B may be 0) of low byte D and high byte E from HL, within
; one 256-byte page. Leaves HL after them.
FillRows:
	inc b
	jr .test
.row
	ld [hl], d
	inc l
	ld [hl], e
	inc l
.test
	dec b
	jr nz, .row
	ret

SECTION "Draw HRAM", HRAM
hColLo: db                          ; the tile column's rays' shade bit 0, as pair bits
hColHi: db                          ; and shade bit 1
hM0: db                         ; each span's wall pair bits
hM1: db
hM2: db
hM3: db
hM4: db
hC0: db                         ; each span's rows, cut to the edge tiles
hC1: db
hC2: db
hC3: db
hC4: db
hT0: db
hT1: db
hTX: db
hWallTile: db
hDrawMeta:: db                  ; the high byte of the draw buffer's map and list page
hListLen:: db                   ; tiles on its copy list so far
hDirty:: db                     ; bit r: the view's map row r changed
hRowBitTop: db
hRowBitBottom: db

; Per view buffer, one page: the view's map (8 rows of 16 tile numbers) for
; the copy to write into the back map, then the list of tiles to copy.
SECTION "View meta 0", WRAM0, ALIGN[8]
wMeta0:: ds 256
SECTION "View meta 1", WRAMX, ALIGN[8]
wMeta1:: ds 256
