; Enemy bullets: up to BULLETS at once, each an 8.8 position and velocity,
; in one of 16 directions at one of two speeds.

include "hardware.inc"
include "game.inc"

DEF HIT_R EQU 3                       ; bullet-center to hitbox distance that hits

SECTION "Bullets", WRAM0, ALIGN[8]

wBullets:: ds BULLETS * B_STRIDE      ; 248 bytes, in one page

SECTION "Bullet state", WRAM0

wPlayerHit:: db                       ; set when a bullet or an enemy hits the ship

SECTION "Bullet scratch", HRAM

hHitX: db                             ; the ship's hitbox center this frame
hHitY: db

SECTION "Bullet code", ROM0

; Frees every bullet.
BulletsClear::
	ld hl, wBullets + B_X
	ld b, BULLETS
	xor a
.clear
	ld [hl], a
	ld a, l
	add B_STRIDE
	ld l, a
	xor a
	dec b
	jr nz, .clear
	ret

; Fires one bullet from B = X, C = Y (screen pixels, the bullet's top-left)
; in direction A (0-15), slow when E = 0, fast otherwise. Drops it when
; every slot is busy. Keeps BC and E.
BulletFire::
	push de
	and 15
	add a
	add a                             ; 4 bytes a direction
	ld d, a
	ld a, e
	and a
	ld a, d
	ld de, DirSlow
	jr z, .table
	ld de, DirFast
.table
	add e
	ld e, a
	adc d
	sub e
	ld d, a                           ; DE: the velocity
	ld hl, wBullets + B_X
.find
	ld a, [hl]
	and a
	jr z, .take
	ld a, l
	add B_STRIDE
	ld l, a
	cp LOW(wBullets + B_X) + BULLETS * B_STRIDE
	jr nz, .find
	pop de
	ret
.take
	dec l                             ; B_XF
	ld a, $80
	ld [hli], a
	ld [hl], b                        ; B_X
	inc l
	ld [hli], a                       ; B_YF
	ld [hl], c                        ; B_Y
	inc l
	ld a, [de]
	inc de
	ld [hli], a                       ; B_DXF
	ld a, [de]
	inc de
	ld [hli], a                       ; B_DX
	ld a, [de]
	inc de
	ld [hli], a                       ; B_DYF
	ld a, [de]
	ld [hl], a                        ; B_DY
	pop de
	ret

; Fires a spread of B bullets around direction A, one direction apart,
; from (D, C) at speed E. Keeps C and E.
BulletFan::
	push bc
	ld h, a
	ld a, b
	srl a
	cpl
	inc a
	add h                             ; the first: A - B/2
	ld h, b
	ld b, d
.each
	push hl
	push af
	call BulletFire
	pop af
	pop hl
	inc a
	dec h
	jr nz, .each
	pop bc
	ret

; Fires half a ring: a bullet in every second direction from A (8 in all),
; from B = X, C = Y at speed E. A full ring is two halves on consecutive
; frames, A even then odd, which keeps either frame's cost down.
BulletRing::
	ld d, 8
.each
	push af
	push de
	call BulletFire
	pop de
	pop af
	add 2
	dec d
	jr nz, .each
	ret

; The direction (0-15) from a bullet at B = X, C = Y to the ship's hitbox.
; Keeps BC and E.
AimDir::
	push bc
	push de
	push hl
	; |dX| / 4 into H, |dY| / 4 into L; D bit 0: left, bit 1: down.
	ld d, 0
	ld a, [wPlayerX]
	add 7 - 4                         ; where the bullet's top-left must go
	sub b
	jr nc, .right
	cpl
	inc a
	set 0, d
.right
	srl a
	srl a
	ld h, a
	ld a, [wPlayerY]
	add 4 - 4
	sub c
	jr c, .up
	set 1, d
	jr .dy
.up
	cpl
	inc a
.dy
	srl a
	srl a
	ld l, a
	; T, the angle from the horizontal in 22.5° steps, by tan's bounds.
	ld e, 0
	ld a, l
	add a
	add a
	add l                             ; 5 dY
	cp h
	jr c, .t                          ; 5 dY < dX
	inc e
	ld a, h
	add a
	ld b, a                           ; 2 dX
	ld a, l
	add a
	add l                             ; 3 dY
	cp b
	jr c, .t
	inc e
	ld a, h
	add a
	add h
	ld b, a                           ; 3 dX
	ld a, l
	add a                             ; 2 dY
	cp b
	jr c, .t
	inc e
	ld a, h
	add a
	add a
	add h
	ld b, a                           ; 5 dX
	ld a, l
	cp b
	jr c, .t
	inc e
.t
	; The quadrant.
	ld a, d
	and a
	ld a, e
	jr z, .done                       ; right, up: T
	dec d
	jr nz, .notLeftUp
	ld a, DIR_LEFT
	sub e                             ; left, up: 8 - T
	jr .done
.notLeftUp
	dec d
	jr nz, .leftDown
	xor a
	sub e                             ; right, down: -T
	and 15
	jr .done
.leftDown
	ld a, DIR_LEFT
	add e                             ; left, down: 8 + T
.done
	pop hl
	pop de
	pop bc
	ret

; Moves every bullet, frees those off the playfield, checks each against
; the ship's hitbox, and adds the sprites.
BulletsUpdate::
	call PlayerHitbox
	ld a, b
	ldh [hHitX], a
	ld a, c
	ldh [hHitY], a
	ld hl, wBullets
.loop
	inc l
	ld a, [hl]                        ; B_X
	dec l
	and a
	jr z, .next
	ld a, l
	add B_DXF
	ld e, a
	ld d, h                           ; DE: the velocity
	ld a, [de]
	add [hl]
	ld [hli], a                       ; B_XF
	inc e
	ld a, [de]
	adc [hl]
	ld [hli], a                       ; B_X
	cp FIELD_W
	jr nc, .kill
	add OAM_X_OFS
	ld c, a
	inc e
	ld a, [de]
	add [hl]
	ld [hli], a                       ; B_YF
	inc e
	ld a, [de]
	adc [hl]
	ld [hl], a                        ; B_Y
	cp FIELD_H
	jr nc, .kill
	add OAM_Y_OFS
	ld b, a
	push hl
	ld d, T_BULLET
	ld e, ATTR_SHOT
	call SprPut
	pop hl
	; Against the hitbox: the bullet's center is (X+4, Y+4).
	ld a, c
	sub OAM_X_OFS - 4
	ld e, a
	ldh a, [hHitX]
	sub e
	jr nc, .absX
	cpl
	inc a
.absX
	cp HIT_R
	jr nc, .next
	ld a, b
	sub OAM_Y_OFS - 4
	ld e, a
	ldh a, [hHitY]
	sub e
	jr nc, .absY
	cpl
	inc a
.absY
	cp HIT_R
	jr nc, .next
	ld a, [wInvuln]
	and a
	jr nz, .next
	ld a, 1
	ld [wPlayerHit], a
.kill
	ld a, l
	and %11111000
	inc a
	ld l, a
	ld [hl], 0                        ; B_X = 0: free
.next
	ld a, l
	and %11111000
	add B_STRIDE
	ld l, a
	cp BULLETS * B_STRIDE
	jr nz, .loop
	ret

; Velocities in 8.8 pixels a frame, 16 directions counterclockwise from
; right; Y grows down the screen, so up is negative.
MACRO directions
	FOR I, 16
		DEF _DX = MUL(\1, COS(I * 1.0 / 16)) >> 8
		DEF _DY = -(MUL(\1, SIN(I * 1.0 / 16)) >> 8)
		dw _DX & $FFFF, _DY & $FFFF
	ENDR
ENDM

DirSlow:
	directions 0.75
DirFast:
	directions 1.25
