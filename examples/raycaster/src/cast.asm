; Casting one ray: a DDA over the map, with the side distances in registers.
; tools/raymath.py's cast() mirrors every step; keep the two in step.

include "hardware.inc"
include "defs.inc"

; One DDA loop for one quadrant: \1 steps the map index in x (inc l or
; dec l), \2 in y (add or sub MAP_W). BC = side distance x, DE = side
; distance y, HL = the map cell.
MACRO DDA_LOOP
.loop\@
	ld a, c
	sub e
	ld a, b
	sbc d
	jr nc, .stepY\@             ; sideX >= sideY
	ldh a, [hDX]
	add c
	ld c, a
	ldh a, [hDX + 1]
	adc b
	ld b, a
	\1
	ld a, [hl]
	and a
	jr z, .loop\@
	jp .hitX
.stepY\@
	ldh a, [hDY]
	add e
	ld e, a
	ldh a, [hDY + 1]
	adc d
	ld d, a
	ld a, l
	\2 MAP_W
	ld l, a
	ld a, [hl]
	and a
	jr z, .loop\@
	jp .hitY
ENDM

; Casts one ray at angle DE (0-1023) from wRPosX, wRPosY.
; Returns DE = the distance to the wall hit (8.8), wSide (0: x side) and
; wCell (the cell hit).
SECTION "Cast", ROM0

CastRay::
	; Cells crossed per unit of ray: dX = Delta[a], dY = Delta[a - 256].
	ld a, d
	ldh [hQuad], a
	add HIGH(DeltaLo)
	ld h, a
	ld l, e
	ld a, [hl]
	ldh [hDX], a
	ld a, h
	add HIGH(DeltaHi) - HIGH(DeltaLo)
	ld h, a
	ld a, [hl]
	ldh [hDX + 1], a
	ld a, d
	dec a
	and ANG_MASK_HI
	add HIGH(DeltaLo)
	ld h, a
	ld a, [hl]
	ldh [hDY], a
	ld a, h
	add HIGH(DeltaHi) - HIGH(DeltaLo)
	ld h, a
	ld a, [hl]
	ldh [hDY + 1], a

	; Side distance to the first x grid line: x forward when
	; (a + 256) mod 1024 < 512.
	ldh a, [hDX]
	ld e, a
	ldh a, [hDX + 1]
	ld d, a
	ldh a, [hQuad]
	inc a
	and 2
	ld a, [wRPosX]
	jr nz, .backX
	and a
	jr z, .wholeX               ; at a grid line, the next is a whole cell on
	cpl
	inc a                       ; 256 - fraction
.backX
	call Mul16x8
	ld a, l
	ldh [hSideX], a
	ld a, h
	ldh [hSideX + 1], a
	jr .sideY
.wholeX
	ld a, e
	ldh [hSideX], a
	ld a, d
	ldh [hSideX + 1], a
.sideY
	; And to the first y grid line: y forward when a < 512.
	ldh a, [hDY]
	ld e, a
	ldh a, [hDY + 1]
	ld d, a
	ldh a, [hQuad]
	and 2
	ld a, [wRPosY]
	jr nz, .backY
	and a
	jr z, .wholeY
	cpl
	inc a
.backY
	call Mul16x8
	ld d, h
	ld e, l
.wholeY
	ldh a, [hSideX]
	ld c, a
	ldh a, [hSideX + 1]
	ld b, a

	; The map cell the player stands in: y * 16 + x.
	ld a, [wRPosY + 1]
	swap a
	ld l, a
	ld a, [wRPosX + 1]
	or l
	ld l, a
	ld h, HIGH(wMap)

	ldh a, [hQuad]
	and a
	jr z, .quad0
	dec a
	jr z, .quad1
	dec a
	jp z, .quad2
	jp .quad3
.quad0                          ; x forward, y forward
	DDA_LOOP inc l, add
.quad1                          ; x back, y forward
	DDA_LOOP dec l, add
.quad2                          ; x back, y back
	DDA_LOOP dec l, sub
.quad3                          ; x forward, y back
	DDA_LOOP inc l, sub

	; The distance: the side distance before the last step.
.hitX
	ld [wCell], a
	xor a
	ld [wSide], a
	ldh a, [hDX]
	ld e, a
	ld a, c
	sub e
	ld e, a
	ldh a, [hDX + 1]
	ld d, a
	ld a, b
	sbc d
	ld d, a
	ret
.hitY
	ld [wCell], a
	ld a, 1
	ld [wSide], a
	ldh a, [hDY]
	ld b, a
	ld a, e
	sub b
	ld e, a
	ldh a, [hDY + 1]
	ld b, a
	ld a, d
	sbc b
	ld d, a
	ret

SECTION "Cast HRAM", HRAM
hQuad: db                       ; the ray angle's high bits: its quadrant
hDX: dw
hDY: dw
hSideX: dw
