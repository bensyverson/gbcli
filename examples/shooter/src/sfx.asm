; Sound effects on channel 4 (noise), which the stage song gives up.
;
; Each effect is four register writes: NR41 (length), NR42 (envelope),
; NR43 (the noise's pitch) and NR44 (trigger). An effect plays unless a
; louder one is still sounding: explosion beats hit beats shot.

include "hardware.inc"

; Effects: priority order, and their row in SfxTable.
DEF SFX_SHOT EQU 0
DEF SFX_HIT EQU 1
DEF SFX_BOOM EQU 2

SECTION "Sfx state", WRAM0

wSfxPlaying: db                       ; the effect sounding now
wSfxLeft: db                          ; frames until it is over; 0 = quiet

SECTION "Sfx", ROM0

SfxInit::
	xor a
	ld [wSfxLeft], a
	ret

SfxShot::
	ld a, SFX_SHOT
	jr SfxPlay
SfxHit::
	ld a, SFX_HIT
	jr SfxPlay
SfxBoom::
	ld a, SFX_BOOM
	; fall through

; Plays effect A, unless a higher one is still sounding. Keeps BC, DE, HL.
SfxPlay:
	push hl
	ld l, a
	ld a, [wSfxLeft]
	and a
	jr z, .play
	ld a, [wSfxPlaying]
	cp l
	jr z, .play
	jr nc, .done                      ; a louder one is on
.play
	ld a, l
	ld [wSfxPlaying], a
	add a
	add a
	add a                             ; 8 bytes a row
	add LOW(SfxTable)
	ld l, a
	ld a, 0
	adc HIGH(SfxTable)
	ld h, a
	ld a, [hli]
	ld [wSfxLeft], a
	ld a, [hli]
	ldh [rNR41], a
	ld a, [hli]
	ldh [rNR42], a
	ld a, [hli]
	ldh [rNR43], a
	ld a, [hl]
	ldh [rNR44], a
.done
	pop hl
	ret

; Once a frame: counts the effect down.
SfxUpdate::
	ld hl, wSfxLeft
	ld a, [hl]
	and a
	ret z
	dec [hl]
	ret

; Frames, NR41, NR42, NR43, NR44, padding.
SfxTable:
	db 6, 0, $A1, $0B, $80, 0, 0, 0   ; shot: a short high hiss, 7-bit
	db 10, 0, $C2, $34, $80, 0, 0, 0  ; hit: a mid crack
	db 45, 0, $F5, $67, $80, 0, 0, 0  ; explosion: a long low rumble
