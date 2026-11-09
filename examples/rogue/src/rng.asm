; A 16-bit xorshift (7, 9, 8): period 65535, any nonzero state.

include "hardware.inc"

SECTION "RNG", ROM0

; Seeds the generator from HL (0 becomes 1), then discards a few outputs so
; that small seeds (a frame count) spread out.
SeedRand::
	ld a, h
	or l
	jr nz, .nonzero
	inc l
.nonzero
	ld a, l
	ld [wRng], a
	ld a, h
	ld [wRng + 1], a
	ld b, 16
.warm
	call Rand
	dec b
	jr nz, .warm
	ret

; Returns the next random byte in A. Keeps BC and DE; clobbers HL.
Rand::
	ld a, [wRng]
	ld l, a
	ld a, [wRng + 1]
	ld h, a
	; hl ^= hl << 7
	ld a, h
	rra
	ld a, l
	rra
	xor h
	ld h, a
	; hl ^= hl >> 9
	ld a, l
	rra
	ld a, h
	rra
	xor l
	ld l, a
	; hl ^= hl << 8
	xor h
	ld h, a
	ld [wRng + 1], a
	ld a, l
	ld [wRng], a
	ld a, h
	ret

; Returns a random number in [0, A) in A, A >= 1. Keeps BC, DE and HL.
RandBelow::
	push bc
	push hl
	ld b, a                     ; n
	dec a
	; c = the smallest all-ones mask covering n - 1
	ld c, a
	srl a
	or c
	ld c, a
	srl a
	srl a
	or c
	ld c, a
	swap a
	and $0F
	or c
	ld c, a
.retry
	call Rand
	and c
	cp b
	jr nc, .retry
	pop hl
	pop bc
	ret

SECTION "RNG state", WRAM0
wRng:: dw
