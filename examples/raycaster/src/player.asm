; Input and movement, run from the VBlank handler once a frame.

include "hardware.inc"
include "defs.inc"

SECTION "Player", ROM0

; Reads the joypad into wPad: the D-pad in bits 4-7, the buttons in 0-3.
ReadJoypad::
	ld a, JOYP_GET_CTRL_PAD
	ldh [rP1], a
	ldh a, [rP1]
	ldh a, [rP1]
	cpl
	and $0F
	swap a
	ld b, a
	ld a, JOYP_GET_BUTTONS
	ldh [rP1], a
	ldh a, [rP1]
	ldh a, [rP1]
	ldh a, [rP1]
	ldh a, [rP1]
	cpl
	and $0F
	or b
	ld [wPad], a
	ld b, a
	ld a, JOYP_GET_NONE
	ldh [rP1], a
	; wPressed: the buttons down now and not last frame.
	ld a, [wPadPrev]
	cpl
	and b
	ld [wPressed], a
	ld a, b
	ld [wPadPrev], a
	ret

; Left and right turn; up and down walk, sliding along walls. A wall walked
; into is left in wBumpCell and wBumpIdx (wBumpCell 0: none).
MovePlayer::
	xor a
	ld [wBumpCell], a
	ld a, [wPad]
	ld b, a
	ld hl, wAngle
	bit B_PAD_LEFT, b
	jr z, .noLeft
	ld a, [hl]
	sub TURN_SPEED
	ld [hli], a
	ld a, [hl]
	sbc 0
	and ANG_MASK_HI
	ld [hld], a
.noLeft
	bit B_PAD_RIGHT, b
	jr z, .noRight
	ld a, [hl]
	add TURN_SPEED
	ld [hli], a
	ld a, [hl]
	adc 0
	and ANG_MASK_HI
	ld [hld], a
.noRight
	ld c, 0                     ; forward
	bit B_PAD_UP, b
	jr nz, .walk
	bit B_PAD_DOWN, b
	ret z
	ld c, $FF                   ; backward: negate the step
.walk
	; The step: cos and sin of the view angle, from MoveTab.
	ld a, [wAngle]
	ld l, a
	ld a, [wAngle + 1]
	add HIGH(MoveTab)
	ld h, a
	ld a, [hl]
	xor c
	sub c
	ld [wStepDX], a
	ld a, [wAngle + 1]
	dec a
	and ANG_MASK_HI
	add HIGH(MoveTab)
	ld h, a
	ld a, [hl]
	xor c
	sub c
	ld [wStepDY], a

	; x first, then y from the new x; each moves only if the cell MARGIN
	; ahead of the new position is open.
	ld a, [wStepDX]
	and a
	jr z, .doneX
	ld hl, wPosX
	call NewCoord               ; DE = new x, HL = probe x
	ld a, [wPosY + 1]
	swap a
	or h
	ld l, a
	ld h, HIGH(wMap)
	ld a, [hl]
	and a
	jr z, .moveX
	call Bump
	jr .doneX
.moveX
	ld a, e
	ld [wPosX], a
	ld a, d
	ld [wPosX + 1], a
.doneX
	ld a, [wStepDY]
	and a
	ret z
	ld hl, wPosY
	call NewCoord
	ld a, h
	swap a
	ld h, a
	ld a, [wPosX + 1]
	or h
	ld l, a
	ld h, HIGH(wMap)
	ld a, [hl]
	and a
	jr nz, Bump
	ld a, e
	ld [wPosY], a
	ld a, d
	ld [wPosY + 1], a
	ret

; Records the cell the player walked into: A its value, L its index.
Bump:
	ld [wBumpCell], a
	ld a, l
	ld [wBumpIdx], a
	ret

; A = signed step, HL = a coordinate. Returns DE = coordinate + step, and
; H = the cell MARGIN beyond DE in the step's direction (L its fraction).
NewCoord:
	ld c, a
	ld e, a
	ld d, 0
	bit 7, a
	jr z, .pos
	dec d
.pos
	ld a, [hli]
	ld h, [hl]
	ld l, a
	add hl, de
	ld d, h
	ld e, l
	bit 7, c
	ld bc, MARGIN
	jr z, .ahead
	ld bc, -MARGIN
.ahead
	add hl, bc
	ret

SECTION "Player variables", WRAM0
wStepDX:: db
wStepDY:: db
wBumpCell:: db
wBumpIdx:: db
