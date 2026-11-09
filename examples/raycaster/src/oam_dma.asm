; OAM DMA: during the 160 M-cycles a DMA takes, the CPU can only read HRAM,
; so the routine that starts it and waits must run from HRAM. CopyOAMDMA puts
; it there once at startup; afterwards, `ld a, HIGH(wShadowOAM)` then
; `call hOAMDMA` copies shadow OAM to OAM. Call it only during VBlank.

include "hardware.inc"

SECTION "OAM DMA routine", ROM0

; Copies the DMA routine into HRAM.
CopyOAMDMA::
	ld hl, OAMDMARoutine
	ld b, OAMDMARoutine.end - OAMDMARoutine
	ld c, LOW(hOAMDMA)
.copy
	ld a, [hli]
	ldh [c], a
	inc c
	dec b
	jr nz, .copy
	ret

; Never called here: this is the code CopyOAMDMA copies to hOAMDMA.
OAMDMARoutine:
	ldh [rDMA], a
	ld a, 40                    ; 40 loops of 4 M-cycles: the 160 the DMA takes
.wait
	dec a
	jr nz, .wait
	ret
.end

SECTION "OAM DMA", HRAM

hOAMDMA::
	ds OAMDMARoutine.end - OAMDMARoutine
