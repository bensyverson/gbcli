; The sprite multiplexer.
;
; Game code never writes shadow OAM. Each frame it calls SprBegin, then
; SprPut once per hardware sprite it wants drawn (up to SPR_MAX, more than
; the 40 OAM holds), and SprFlush packs the list into shadow OAM for the
; next VBlank's DMA, alternating its order frame by frame:
;
; - Even frames copy the list forward from the start S: S, S+1, …
; - Odd frames copy it backward from S-1: S-1, S-2, … and then step S back.
;
; OAM is always filled from slot 0, and slot order is priority twice over:
; only 40 fit, and on a line with more than 10 sprites the DMG draws the
; 10 earliest in OAM. A list of up to 80 sprites is therefore all in OAM
; within two frames, and the sprites that lost OAM or a crowded line one
; frame are near the front the next (fair for up to 20 on a line; the slow
; step of S back rotates lines busier than that). The cost is flicker at 30 Hz
; on whatever the hardware can't show at once.

include "hardware.inc"

DEF SPR_MAX EQU 64                    ; list entries: 4 bytes each, one page
EXPORT SPR_MAX
DEF SPR_STEP EQU 11                   ; how far S moves back every second frame: at most 80 - SPR_MAX

SECTION "Sprite list", WRAM0, ALIGN[8]

; The frame's sprites, in OAM's format: Y+16, X+8, tile, attributes.
wSprList:: ds SPR_MAX * OBJ_SIZE

SECTION "Sprite state", WRAM0

wSprCount:: db                        ; entries in wSprList this frame
wSprStart:: db                        ; S, the list index even frames start at

SECTION "Sprite scratch", HRAM

hSprLeft: db                          ; entries still to copy
hSprDir: db                           ; 1 forward, -1 backward

SECTION "Sprites", ROM0

SprInit::
	xor a
	ld [wSprCount], a
	ld [wSprStart], a
	ret

; Empties the frame's list.
SprBegin::
	xor a
	ld [wSprCount], a
	ret

; Adds one sprite: B = screen Y + 16, C = screen X + 8, D = tile,
; E = attributes. Drops it silently when the list is full. Keeps BC and DE.
SprPut::
	ld a, [wSprCount]
	cp SPR_MAX
	ret nc
	ld l, a
	inc a
	ld [wSprCount], a
	ld a, l
	add a
	add a
	ld l, a
	ld h, HIGH(wSprList)
	ld a, b
	ld [hli], a
	ld a, c
	ld [hli], a
	ld a, d
	ld [hli], a
	ld [hl], e
	ret

; Packs the list into shadow OAM (see the top of this file).
SprFlush::
	; Hide all 40: Y = 0 is above the screen and on no line.
	ld hl, wShadowOAM
	ld b, OAM_COUNT
	xor a
.hide
	ld [hli], a
	inc l
	inc l
	inc l
	dec b
	jr nz, .hide

	ld a, [wSprCount]
	and a
	ret z
	ld c, a                           ; C = list length
	cp OAM_COUNT + 1
	jr c, .fits
	ld a, OAM_COUNT
.fits
	ldh [hSprLeft], a

	ld a, [wSprStart]
	cp c
	jr c, .startOk
	xor a                             ; the list shrank under S
	ld [wSprStart], a
.startOk
	ld b, a                           ; B = list index being copied
	ld a, [wFrame]
	rra
	ld a, 1
	jr nc, .dirKnown
	; Backward, from S-1.
	ld a, b
	and a
	jr nz, .notFirst
	ld b, c
.notFirst
	dec b
	ld a, -1
.dirKnown
	ldh [hSprDir], a
	ld de, wShadowOAM

.copy
	ld a, b
	add a
	add a
	ld l, a
	ld h, HIGH(wSprList)
	ld a, [hli]
	ld [de], a
	inc e
	ld a, [hli]
	ld [de], a
	inc e
	ld a, [hli]
	ld [de], a
	inc e
	ld a, [hl]
	ld [de], a
	inc e
	; The next index, wrapping at either end.
	ldh a, [hSprDir]
	add b
	cp c
	jr c, .indexOk                    ; 0 to C-1
	ldh a, [hSprDir]
	inc a                             ; forward past the end: Z clear
	ld a, 0
	jr nz, .indexOk
	ld a, c                           ; backward past 0
	dec a
.indexOk
	ld b, a
	ldh a, [hSprLeft]
	dec a
	ldh [hSprLeft], a
	jr nz, .copy

	; After a backward frame, step S back.
	ldh a, [hSprDir]
	inc a
	ret nz
	; S moves back, never on: a step on would leave out, two frames
	; running, the sprites this frame left out.
	ld a, [wSprStart]
	sub SPR_STEP
	jr nc, .store
.wrap
	add c
	jr nc, .wrap
.store
	ld [wSprStart], a
	ret
