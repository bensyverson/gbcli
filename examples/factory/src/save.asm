; The best score, kept in the cartridge's battery-backed RAM.
;
; The save is a 4-byte mark, then the best score (BCD, low byte first). A
; save without the mark (a new cartridge, or garbage) reads as best 0.

include "hardware.inc"

SECTION "Save code", ROM0

; wBest from the save, writing a fresh save if there is none.
SaveLoad::
	ld a, RAMG_SRAM_ENABLE
	ld [rRAMG], a
	xor a
	ld [rRAMB], a
	ld hl, sMark
	ld de, SaveMark
	ld b, SaveMark.end - SaveMark
.compare
	ld a, [de]
	cp [hl]
	jr nz, .fresh
	inc de
	inc hl
	dec b
	jr nz, .compare
	ld a, [sBest]
	ld [wBest], a
	ld a, [sBest + 1]
	ld [wBest + 1], a
	jr SaveClose
.fresh
	ld hl, sMark
	ld de, SaveMark
	ld b, SaveMark.end - SaveMark
.mark
	ld a, [de]
	ld [hli], a
	inc de
	dec b
	jr nz, .mark
	xor a
	ld [wBest], a
	ld [wBest + 1], a
	; fall through

; Writes wBest to the save.
SaveBest::
	ld a, RAMG_SRAM_ENABLE
	ld [rRAMG], a
	xor a
	ld [rRAMB], a
	ld a, [wBest]
	ld [sBest], a
	ld a, [wBest + 1]
	ld [sBest + 1], a
	; fall through

SaveClose:
	ld a, RAMG_SRAM_DISABLE     ; closed, so a power cut can't scribble on it
	ld [rRAMG], a
	ret

SaveMark:
	db "TOYS"
.end

SECTION "Save", SRAM[$A000]
sMark: ds 4
sBest: dw

SECTION "Best", WRAM0
wBest:: dw                         ; the personal best, BCD (low byte: tens and ones)
