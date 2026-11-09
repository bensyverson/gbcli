; The parallax sky: four bands of the background, each scrolled at its own
; speed by changing SCX between them, plus the window as a fixed status
; bar at the bottom.
;
; Band  lines     speed       split written in the HBlank of line
;  0    0-39      1/8 px/f    (VBlank sets it)
;  1    40-87     1/4 px/f    39
;  2    88-119    1/2 px/f    87
;  3    120-135   1 px/f      119
;  bar  136-143   window, no scroll
;
; An LY=LYC STAT interrupt fires at the start of the line before each band;
; the handler waits for that line's HBlank (STAT mode 0) and only then
; writes SCX, so no write lands in mode 3.

include "hardware.inc"

DEF BAND1_LINE EQU 40
DEF BAND2_LINE EQU 88
DEF BAND3_LINE EQU 120
DEF BAR_LINE EQU 136
DEF NO_LINE EQU $FF                   ; LYC no line reaches
EXPORT BAND1_LINE, BAND2_LINE, BAND3_LINE, BAR_LINE

SECTION "STAT vector", ROM0[INT_HANDLER_STAT]
StatVector:
	jp StatSplit

SECTION "Parallax state", HRAM

hSplit: db                            ; which split comes next, 0-2
hBandScx:: ds 4                       ; SCX for bands 0-3
hSplitLyc: ds 4                       ; the line before bands 1-3, then none

SECTION "Parallax", ROM0

; With the LCD off: the window's place, the split table and the STAT
; source. The sky's tiles and map are gfx.asm's (SkyLoad).
ParallaxInit::
	; The window: the status bar's place (font.asm draws it).
	ld a, BAR_LINE
	ldh [rWY], a
	ld a, WX_OFS
	ldh [rWX], a

	ld hl, hSplitLyc
	ld a, BAND1_LINE - 1
	ld [hli], a
	ld a, BAND2_LINE - 1
	ld [hli], a
	ld a, BAND3_LINE - 1
	ld [hli], a
	ld [hl], NO_LINE
	xor a
	ld hl, hBandScx
	ld [hli], a
	ld [hli], a
	ld [hli], a
	ld [hl], a
	ldh [rSCX], a
	ldh [rSCY], a
	ldh [hSplit], a
	ld a, STAT_LYC
	ldh [rSTAT], a
	ld a, NO_LINE
	ldh [rLYC], a
	ret

; In VBlank: the top band's SCX, and the first split armed.
ParallaxVBlank::
	ldh a, [hBandScx]
	ldh [rSCX], a
	xor a
	ldh [hSplit], a
	ldh a, [hSplitLyc]
	ldh [rLYC], a
	ret

; Once a frame: each band's SCX from the frame count.
ParallaxUpdate::
	ld a, [wFrame]
	ld b, a
	ldh [hBandScx + 3], a             ; 1 px a frame
	srl a
	ldh [hBandScx + 2], a             ; 1/2
	srl a
	ldh [hBandScx + 1], a             ; 1/4
	srl a
	ld c, a
	ld a, [wFrame + 1]
	swap a                            ; frame bits 8-11 into bits 5-7 of SCX
	add a
	and %11100000
	or c
	ldh [hBandScx], a                 ; 1/8
	ret

; The split: SCX for the band that starts on the next line, written in this
; line's HBlank, then the next split armed.
StatSplit:
	push af
	push bc
	ldh a, [hSplit]
	add LOW(hBandScx + 1)
	ld c, a
	ldh a, [c]
	ld b, a
.waitHBlank
	ldh a, [rSTAT]
	and STAT_MODE
	jr nz, .waitHBlank
	ld a, b
.write
	ldh [rSCX], a
	ldh a, [hSplit]
	inc a
	ldh [hSplit], a
	add LOW(hSplitLyc)
	ld c, a
	ldh a, [c]
	ldh [rLYC], a
	pop bc
	pop af
	reti
