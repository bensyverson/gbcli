; Rendering a view: one ray per 2-pixel column (cast.asm), each column's
; wall half height and shade, then DrawView (draw.asm) into the draw buffer.
; tools/raymath.py mirrors every step here; keep the two in step.

include "hardware.inc"
include "defs.inc"

SECTION "Render", ROM0

; Casts the view from the player's pose into the draw buffer (hDrawBuf).
RenderView::
	; Take the pose in one piece: the VBlank handler moves the player.
	di
	ld hl, wPosX
	ld de, wRPosX
	ld b, 6
.pose
	ld a, [hli]
	ld [de], a
	inc de
	dec b
	jr nz, .pose
	ei

	xor a
.ray
	ld [wCol], a
	; The ray's angle: the view angle plus the column's offset, mod 1024.
	add LOW(ColOffHi)
	ld l, a
	ld h, HIGH(ColOffHi)
	ld b, [hl]                  ; the offset's high byte
	ld a, l
	sub LOW(ColOffHi) - LOW(ColOffLo)
	ld l, a
	ld a, [wRAngle]
	add [hl]
	ld e, a
	ld a, [wRAngle + 1]
	adc b
	and ANG_MASK_HI
	ld d, a
	call CastRay                ; DE = distance, wSide
	; Undo the fisheye: perpendicular distance = distance * cos(offset).
	ld a, [wCol]
	add LOW(CosOff)
	ld l, a
	ld h, HIGH(CosOff)
	ld a, [hl]
	call Mul16x8                ; HL = perpendicular distance
	; Half height from the table, indexed by distance >> 3.
	ld a, l
	srl h
	rra
	srl h
	rra
	srl h
	rra
	ld l, a
	ld a, h
	add HIGH(HalfHeight)
	ld h, a
	ld b, [hl]
	ld a, [wCol]
	ld l, a
	ld h, HIGH(wHH)
	ld [hl], b
	; The wall's shade, from the cell hit and the side, as the ray's pair
	; bits in each plane: wLoBits (shade bit 0), wHiBits (shade bit 1).
	ld a, l
	and 3
	add LOW(PairMask)
	ld e, a
	ld d, HIGH(PairMask)
	ld a, [de]
	ld c, a                     ; the ray's pair bits
	ld a, [wCell]
	add a
	ld e, a
	ld a, [wSide]
	add e
	add LOW(CellShade)
	ld e, a
	ld d, HIGH(CellShade)
	ld a, [de]
	ld b, a                     ; the shade
	inc h                       ; wLoBits
	xor a
	bit 0, b
	jr z, .gotLo
	ld a, c
.gotLo
	ld [hl], a
	inc h                       ; wHiBits
	xor a
	bit 1, b
	jr z, .gotHi
	ld a, c
.gotHi
	ld [hl], a
	ld a, [wCol]
	inc a
	cp RAYS
	jr nz, .ray

	jp DrawView

; HL = (DE * A) >> 8, exactly: D * A + ((E * A) >> 8), each 8x8 product
; from quarter squares (a * b = Sq(a + b) - Sq(|a - b|)). Clobbers BC, DE.
Mul16x8::
	ld b, a
	; C = (E * B) >> 8: the high byte of the low product.
	ld a, e
	sub b
	jr nc, .lowDiff
	cpl
	inc a
.lowDiff
	ld l, a
	ld h, HIGH(SqLo)
	ld c, [hl]
	inc h
	inc h
	ld a, [hl]
	ldh [hMulT], a
	ld a, e
	add b
	ld l, a
	ld a, HIGH(SqLo)
	adc 0
	ld h, a
	ld a, [hl]
	sub c                       ; only the borrow matters
	inc h                       ; (inc leaves the carry alone)
	inc h
	ldh a, [hMulT]
	ld c, a
	ld a, [hl]
	sbc c
	ld c, a
	; DE = D * B.
	ld a, d
	sub b
	jr nc, .highDiff
	cpl
	inc a
.highDiff
	ld l, a
	ld h, HIGH(SqLo)
	ld e, [hl]
	inc h
	inc h
	ld a, [hl]
	ldh [hMulT], a
	ld a, d
	add b
	ld l, a
	ld a, HIGH(SqLo)
	adc 0
	ld h, a
	ld a, [hl]
	sub e
	ld e, a
	inc h
	inc h
	ldh a, [hMulT]
	ld d, a
	ld a, [hl]
	sbc d
	ld d, a
	; HL = DE + C.
	ld a, e
	add c
	ld l, a
	ld a, d
	adc 0
	ld h, a
	ret

SECTION "Multiply scratch", HRAM
hMulT: db

SECTION "Pair masks", ROM0, ALIGN[2]
PairMask::
	db %11000000, %00110000, %00001100, %00000011

SECTION "Render variables", WRAM0

; The pose the view is being cast from (copied to wViewX.. when shown).
wRPosX:: dw
wRPosY:: dw
wRAngle:: dw
wCol:: db
wSide:: db                      ; the last ray's wall: 0 x side, 1 y side
wCell:: db                      ; and the cell it hit

SECTION "Column heights", WRAM0, ALIGN[8]
wHH:: ds 256                    ; half height per column (RAYS used)
wLoBits:: ds 256                ; the column's pair bits if its shade has bit 0
wHiBits:: ds 256                ; and if it has bit 1

SECTION "Map", WRAM0, ALIGN[8]
wMap:: ds MAP_W * MAP_W         ; nonzero cells are walls

; Two view buffers: the main loop casts into one while the VBlank handler
; copies the other. Each is the view as tiles, column-major: tile column tx
; at tx * 128, row y of it at 2 * y (low plane, then high). VRAM tile
; tx * 8 + ty has the same offset.
SECTION "View buffer 0", WRAM0, ALIGN[8]
wBuf0:: ds BUF_SIZE
SECTION "View buffer 1", WRAMX, ALIGN[8]
wBuf1:: ds BUF_SIZE

SECTION "Draw buffer", HRAM
hDrawBuf:: db                   ; the high byte of the buffer being cast into
