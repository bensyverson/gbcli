; The supply: what each slot of the parts belt brings as it comes out from
; under the key column. Every number here is in tuning.inc (The supply).
;
; A slot brings a piece with SUPPLY_CHANCE in 256, and after each piece
; SUPPLY_MIN_GAP slots stay empty. A piece is any of SupplyPieces (gen/toys.asm) at
; random, except while Elle holds one half (wWantedPiece completes it):
; then it is wWantedPiece with SUPPLY_MATCH_CHANCE in 256, and always
; once SUPPLY_MATCH_MAX_GAP pieces in a row haven't been it.

include "hardware.inc"
include "tuning.inc"
include "gen/toys.inc"

	ASSERT SUPPLY_CHANCE <= 255 && SUPPLY_MATCH_CHANCE <= 255, "a chance is out of 256: 0-255"

SECTION "Supply", ROM0

; Clears the supply's memory for a round (the random seed carries on).
SupplyInit::
	xor a
	ld [wSupplyGap], a
	ld [wSinceMatch], a
	ld [wWantedPiece], a
	ret

; Out: a = the next slot's contents, 0 or a piece 1-12. Keeps bc.
SupplyNext::
	ld a, [wSupplyGap]
	and a
	jr z, .roll
	dec a
	ld [wSupplyGap], a
	xor a
	ret
.roll
	call Random
	cp SUPPLY_CHANCE
	jr c, .piece
	xor a
	ret
.piece
	ld a, SUPPLY_MIN_GAP
	ld [wSupplyGap], a
	ld a, [wWantedPiece]
	and a
	jr z, .any
	ld a, [wSinceMatch]
	cp SUPPLY_MATCH_MAX_GAP
	jr nc, .match
	call Random
	cp SUPPLY_MATCH_CHANCE
	jr c, .match
.any
	call Random
	; a * SUPPLY_PIECES / 256: one of SupplyPieces, evenly.
	ld e, a
	ld d, 0
	ld hl, 0
	ld a, SUPPLY_PIECES
.mul
	srl a
	jr nc, .noAdd
	add hl, de
.noAdd
	sla e
	rl d
	and a
	jr nz, .mul
	ld a, h
	add LOW(SupplyPieces)
	ld l, a
	adc HIGH(SupplyPieces)
	sub l
	ld h, a
	ld e, [hl]
	ld a, [wWantedPiece]
	cp e
	jr z, .counted
	ld a, [wSinceMatch]
	inc a
	jr z, .capped               ; stays at 255
	ld [wSinceMatch], a
.capped
	ld a, e
	ret
.match
	ld a, [wWantedPiece]
	ld e, a
.counted
	xor a
	ld [wSinceMatch], a
	ld a, e
	ret

; Out: a = a random byte (and hl = the new 16-bit state). Keeps bc, de.
; John Metcalf's xorshift 7,9,8.
Random::
	ld hl, wSeed
	ld a, [hli]
	ld h, [hl]
	ld l, a
	ld a, h
	rra
	ld a, l
	rra
	xor h
	ld h, a
	ld a, l
	rra
	ld a, h
	rra
	xor l
	ld l, a
	xor h
	ld h, a
	ld a, l
	ld [wSeed], a
	ld a, h
	ld [wSeed + 1], a
	ret

; Mixes a into the seed (never leaving it 0). Keeps bc, de.
RandomMix::
	ld hl, wSeed
	xor [hl]
	ld [hli], a
	or [hl]
	ret nz
	ld [hl], 1
	ret

SECTION "Supply variables", WRAM0

wSeed:: dw                         ; the random state: never 0
wSupplyGap: db                     ; empty slots still owed after the last piece
wSinceMatch:: db                   ; new pieces since one matched wWantedPiece
wWantedPiece:: db                  ; the half that completes Elle's one half, or 0
