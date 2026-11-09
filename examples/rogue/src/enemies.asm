; Enemies: placed with the floor, they act once each time the player does.
; One next to the player bites; one that can sense the player (x + y
; distance up to CHASE_RANGE) steps toward it, along the longer axis first;
; the rest wait.

include "hardware.inc"
include "game.inc"

SECTION "Enemies", ROM0

; HP, attack and sprite tile for each kind.
EnemyKinds:
	db 3, 1, S_RAT              ; KIND_RAT
	db 6, 2, S_GHOUL            ; KIND_GHOUL
DEF KIND_SIZE EQU 3

; GBC object palette 1; the DMG ignores these bits and uses OBP0.
DEF OAM_FOES_ATTR EQU 1

; Places 2 + depth enemies (at most MAX_ENEMIES) on floor cells of rooms
; other than the first, never two on one cell. Call after the rooms exist.
SpawnEnemies::
	ld hl, wEnemyHP
	ld b, MAX_ENEMIES
	xor a
.clear
	ld [hli], a
	dec b
	jr nz, .clear
	ld a, [wDepth]
	add 2
	cp MAX_ENEMIES + 1
	jr c, .countOk
	ld a, MAX_ENEMIES
.countOk
	ld [wSpawnLeft], a
	xor a
	ld [wSpawnIndex], a
.next
	call MaybeYield
	call RandomRoomCell         ; bc = a cell, not in the first room
	call CellAddr
	ld a, [hl]
	cp T_FLOOR
	jr nz, .next
	call EnemyAt
	jr c, .next
	; Kind: ghouls from floor 3 on, half the time.
	ld d, KIND_RAT
	ld a, [wDepth]
	cp 2
	jr c, .kindOk
	call Rand
	and 1
	ld d, a
.kindOk
	ld a, [wSpawnIndex]
	ld e, a
	push de
	ld d, 0
	ld hl, wEnemyX
	add hl, de
	ld [hl], b
	ld hl, wEnemyY
	add hl, de
	ld [hl], c
	pop de
	push de
	ld a, d
	ld d, 0
	ld hl, wEnemyKind
	add hl, de
	ld [hl], a
	; HP from the kind's table.
	ld e, a
	add a
	add e
	ld e, a
	ld hl, EnemyKinds
	add hl, de
	ld a, [hl]
	pop de
	ld d, 0
	ld hl, wEnemyHP
	add hl, de
	ld [hl], a
	ld a, e
	inc a
	ld [wSpawnIndex], a
	ld a, [wSpawnLeft]
	dec a
	ld [wSpawnLeft], a
	jr nz, .next
	ret

; Returns in B, C a random cell inside a random room other than the first.
RandomRoomCell::
	ld a, [wRoomCount]
	dec a
	call RandBelow
	inc a
	add a
	add a
	ld e, a
	ld d, 0
	ld hl, wRooms
	add hl, de
	ld a, [hli]
	ld b, a                     ; x
	ld a, [hli]
	ld c, a                     ; y
	ld a, [hli]                 ; w
	call RandBelow
	add b
	ld b, a
	ld a, [hl]                  ; h
	call RandBelow
	add c
	ld c, a
	ret

; Sets carry if a live enemy stands on (B, C), with its index in E. Keeps BC.
EnemyAt::
	ld e, 0
	ld hl, wEnemyHP
.loop
	ld a, [hli]
	and a
	jr z, .skip
	push hl
	ld d, 0
	ld hl, wEnemyX
	add hl, de
	ld a, [hl]
	cp b
	jr nz, .no
	ld hl, wEnemyY
	add hl, de
	ld a, [hl]
	cp c
	jr nz, .no
	pop hl
	scf
	ret
.no
	pop hl
.skip
	inc e
	ld a, e
	cp MAX_ENEMIES
	jr nz, .loop
	and a
	ret

; The player hits enemy E for wAtk; at 0 HP it's gone.
HitEnemy::
	ld d, 0
	ld hl, wEnemyHP
	add hl, de
	ld a, [wAtk]
	ld b, a
	ld a, [hl]
	sub b
	jr nc, .alive
	xor a
.alive
	ld [hl], a
	ret

; Every live enemy acts once.
EnemiesTurn::
	xor a
	ld [wTurnIndex], a
.enemy
	ld a, [wTurnIndex]
	ld e, a
	ld d, 0
	ld hl, wEnemyHP
	add hl, de
	ld a, [hl]
	and a
	jr z, .done
	ld hl, wEnemyX
	add hl, de
	ld a, [hl]
	ld [wEX], a
	ld hl, wEnemyY
	add hl, de
	ld a, [hl]
	ld [wEY], a
	call EnemyAct
	ld a, [wGameState]
	cp STATE_PLAY
	ret nz
.done
	ld a, [wTurnIndex]
	inc a
	ld [wTurnIndex], a
	cp MAX_ENEMIES
	jr nz, .enemy
	ret

; One enemy's turn: wTurnIndex, standing at wEX, wEY.
EnemyAct:
	; dx = px - ex into wDX (signed), |dx| into wADX; the same for y.
	ld a, [wEX]
	ld b, a
	ld a, [wPlayerX]
	sub b
	ld [wDX], a
	call Abs
	ld [wADX], a
	ld a, [wEY]
	ld b, a
	ld a, [wPlayerY]
	sub b
	ld [wDY], a
	call Abs
	ld [wADY], a
	ld b, a
	ld a, [wADX]
	add b
	cp 1
	jr z, Bite
	cp CHASE_RANGE + 1
	ret nc
	; Step along the longer axis first, then the other.
	ld a, [wADY]
	ld b, a
	ld a, [wADX]
	cp b
	jr c, .yFirst
	call TryStepX
	ret c
	jp TryStepY
.yFirst
	call TryStepY
	ret c
	jp TryStepX

; The enemy bites: the player loses the kind's attack in HP.
Bite:
	call EnemyKindRow
	inc hl
	ld b, [hl]                  ; attack
	ld a, [wHP]
	sub b
	jr z, .dead
	jr c, .dead
	ld [wHP], a
	ld a, SFX_HURT
	jp SfxPlay
.dead
	xor a
	ld [wHP], a
	ld a, STATE_DEAD
	ld [wGameState], a
	ret

; Returns in HL the EnemyKinds row of enemy wTurnIndex.
EnemyKindRow:
	ld a, [wTurnIndex]
	ld e, a
	ld d, 0
	ld hl, wEnemyKind
	add hl, de
	ld a, [hl]
	ld e, a
	add a
	add e
	ld e, a
	ld hl, EnemyKinds
	add hl, de
	ret

; Tries one step toward the player along x (none if dx is 0). Carry if moved.
TryStepX:
	ld a, [wDX]
	and a
	ret z                       ; and no carry
	ld a, [wEX]
	ld b, a
	ld a, [wDX]
	bit 7, a
	jr nz, .left
	inc b
	jr .go
.left
	dec b
.go
	ld a, [wEY]
	ld c, a
	jr TryStepTo

; Tries one step toward the player along y (none if dy is 0). Carry if moved.
TryStepY:
	ld a, [wDY]
	and a
	ret z
	ld a, [wEY]
	ld c, a
	ld a, [wDY]
	bit 7, a
	jr nz, .up
	inc c
	jr .go
.up
	dec c
.go
	ld a, [wEX]
	ld b, a
	; fall through

; Moves enemy wTurnIndex to (B, C) if that cell is open: walkable, not the
; player's, no other enemy's. Carry if moved.
TryStepTo:
	call CellAddr
	ld a, [hl]
	cp T_ROCK
	jr z, .blocked
	cp T_WALL
	jr z, .blocked
	ld a, [wPlayerX]
	cp b
	jr nz, .notPlayer
	ld a, [wPlayerY]
	cp c
	jr z, .blocked
.notPlayer
	call EnemyAt
	jr c, .blocked
	ld a, [wTurnIndex]
	ld e, a
	ld d, 0
	ld hl, wEnemyX
	add hl, de
	ld [hl], b
	ld hl, wEnemyY
	add hl, de
	ld [hl], c
	scf
	ret
.blocked
	and a
	ret

; Returns |A| for a signed A.
Abs:
	bit 7, a
	ret z
	cpl
	inc a
	ret

; Puts each live enemy on screen in OAM entries 1 on, or hides it.
PlaceEnemySprites::
	ld hl, wShadowOAM + 4
	ld e, 0
.loop
	push hl
	ld d, 0
	ld hl, wEnemyHP
	add hl, de
	ld a, [hl]
	and a
	jr z, .hide
	ld hl, wEnemyX
	add hl, de
	ld a, [wCamX]
	ld b, a
	ld a, [hl]
	sub b
	jr c, .hide
	cp VIEW_W
	jr nc, .hide
	ld b, a                     ; screen column
	ld hl, wEnemyY
	add hl, de
	ld a, [wCamY]
	ld c, a
	ld a, [hl]
	sub c
	jr c, .hide
	cp VIEW_H - 1               ; the last row is under the status line
	jr nc, .hide
	ld c, a                     ; screen row
	ld hl, wEnemyKind
	add hl, de
	ld a, [hl]
	ld d, a
	add a
	add d
	add 2
	push de
	ld e, a
	ld d, 0
	ld hl, EnemyKinds
	add hl, de
	ld a, [hl]                  ; tile
	pop de
	ld d, a
	pop hl
	ld a, c
	add a
	add a
	add a
	add OAM_Y_OFS
	ld [hli], a
	ld a, b
	add a
	add a
	add a
	add OAM_X_OFS
	ld [hli], a
	ld a, d
	ld [hli], a
	ld a, OAM_FOES_ATTR
	ld [hli], a
	jr .next
.hide
	pop hl
	xor a
	ld [hli], a
	ld [hli], a
	ld [hli], a
	ld [hli], a
.next
	inc e
	ld a, e
	cp MAX_ENEMIES
	jr nz, .loop
	ret

SECTION "Enemy variables", WRAM0
wEnemyX:: ds MAX_ENEMIES
wEnemyY:: ds MAX_ENEMIES
wEnemyHP:: ds MAX_ENEMIES      ; 0: gone
wEnemyKind:: ds MAX_ENEMIES
wSpawnLeft: db
wSpawnIndex: db
wTurnIndex: db
wEX: db
wEY: db
wDX: db
wDY: db
wADX: db
wADY: db
