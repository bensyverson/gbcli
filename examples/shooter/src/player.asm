; The player's ship and its shots.
;
; The ship is 16x8 at (wPlayerX, wPlayerY), top-left, in screen pixels.
; Its hitbox is small, as the genre expects: 4x4 around the cockpit
; (PlayerHitbox). A held fires a shot every SHOT_COOL frames.

include "hardware.inc"
include "game.inc"

DEF SHIP_SPEED EQU 2
DEF SHOT_SPEED EQU 4
DEF SHOT_COOL EQU 6
DEF SHIP_MAX_X EQU FIELD_W - 16
DEF SHIP_MAX_Y EQU FIELD_H - 8
DEF START_X EQU 16
DEF START_Y EQU 64
DEF INVULN_FRAMES EQU 120
EXPORT INVULN_FRAMES

SECTION "Player", WRAM0

wPlayerX:: db
wPlayerY:: db
wInvuln:: db                          ; frames left of the blink after a hit
wShotCool: db
wShots:: ds SHOTS * 2                 ; X, Y; X = 0 is a free slot

SECTION "Player code", ROM0

PlayerInit::
	ld a, START_X
	ld [wPlayerX], a
	ld a, START_Y
	ld [wPlayerY], a
	ld a, INVULN_FRAMES
	ld [wInvuln], a
	xor a
	ld [wShotCool], a
	ld hl, wShots
	ld b, SHOTS * 2
.clear
	ld [hli], a
	dec b
	jr nz, .clear
	ret

; Moves the ship by the pad, fires, moves the shots, and adds the sprites.
PlayerUpdate::
	ldh a, [hPad]
	ld b, a
	ld hl, wPlayerX
	bit B_PAD_LEFT, b
	jr z, .notLeft
	ld a, [hl]
	sub SHIP_SPEED
	jr nc, .left
	xor a
.left
	ld [hl], a
.notLeft
	bit B_PAD_RIGHT, b
	jr z, .notRight
	ld a, [hl]
	add SHIP_SPEED
	cp SHIP_MAX_X + 1
	jr c, .right
	ld a, SHIP_MAX_X
.right
	ld [hl], a
.notRight
	inc hl                            ; wPlayerY
	bit B_PAD_UP, b
	jr z, .notUp
	ld a, [hl]
	sub SHIP_SPEED
	jr nc, .up
	xor a
.up
	ld [hl], a
.notUp
	bit B_PAD_DOWN, b
	jr z, .notDown
	ld a, [hl]
	add SHIP_SPEED
	cp SHIP_MAX_Y + 1
	jr c, .down
	ld a, SHIP_MAX_Y
.down
	ld [hl], a
.notDown

	; Fire.
	ld hl, wShotCool
	ld a, [hl]
	and a
	jr z, .cool
	dec [hl]
	jr .moveShots
.cool
	bit B_PAD_A, b
	jr z, .moveShots
	ld hl, wShots
	ld c, SHOTS
.findShot
	ld a, [hl]
	and a
	jr z, .newShot
	inc hl
	inc hl
	dec c
	jr nz, .findShot
	jr .moveShots
.newShot
	ld a, [wPlayerX]
	add 12
	ld [hli], a
	ld a, [wPlayerY]
	ld [hl], a
	ld a, SHOT_COOL
	ld [wShotCool], a
	call SfxShot

.moveShots
	ld hl, wShots
	ld c, SHOTS
.shot
	ld a, [hl]
	and a
	jr z, .nextShot
	add SHOT_SPEED
	cp FIELD_W
	jr c, .shotOn
	xor a                             ; off the right edge: free
.shotOn
	ld [hl], a
	and a
	jr z, .nextShot
	; sprite: B = Y + 16, C = X + 8
	push bc
	push hl
	add OAM_X_OFS
	ld c, a
	inc hl
	ld a, [hl]
	add OAM_Y_OFS
	ld b, a
	ld d, T_SHOT
	ld e, 0
	call SprPut
	pop hl
	pop bc
.nextShot
	inc hl
	inc hl
	dec c
	jr nz, .shot

	; The ship, blinking while invulnerable.
	ld hl, wInvuln
	ld a, [hl]
	and a
	jr z, .solid
	dec [hl]
	ld a, [wFrame]
	and %00000100
	ret nz
.solid
	ld a, [wPlayerY]
	add OAM_Y_OFS
	ld b, a
	ld a, [wPlayerX]
	add OAM_X_OFS
	ld c, a
	ld d, T_SHIP
	ld e, 0
	call SprPut
	ld a, c
	add 8
	ld c, a
	inc d
	jp SprPut

; The hitbox's center: B = X, C = Y, in screen pixels.
PlayerHitbox::
	ld a, [wPlayerX]
	add 7
	ld b, a
	ld a, [wPlayerY]
	add 4
	ld c, a
	ret
