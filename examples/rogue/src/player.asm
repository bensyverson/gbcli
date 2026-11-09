; The player's turn: a D-pad press steps one cell (held, it repeats), or
; attacks an enemy standing there; B waits a turn. Then every enemy acts.
; Stepping on an item takes it; stairs lead down; the Ember wins.

include "hardware.inc"
include "game.inc"

SECTION "Player", ROM0

; Sets up a new run's player.
PlayerInit::
	ld a, PLAYER_HP
	ld [wHP], a
	ld [wMaxHP], a
	ld a, PLAYER_ATK
	ld [wAtk], a
	xor a
	ld [wGameState], a
	ld [wDepth], a
	ret

PlayerUpdate::
	; What the last turn queued must reach the screen before the next turn.
	ld a, [wStreamKind]
	ld b, a
	ld a, [wPatchPending]
	or b
	ret nz
	ld a, [wPadNew]
	and PAD_B
	jp nz, EndTurn              ; wait a turn
	ld a, [wPadNew]
	and PAD_CTRL_PAD
	jr z, .held
	ld b, a
	ld a, REPEAT_FIRST
	ld [wRepeat], a
	jr .step
.held
	ld a, [wPad]
	and PAD_CTRL_PAD
	ret z
	ld b, a
	ld hl, wRepeat
	dec [hl]
	ret nz
	ld [hl], REPEAT_NEXT
.step
	; One direction per step, in a fixed order.
	ld a, [wPlayerX]
	ld d, a
	ld a, [wPlayerY]
	ld e, a
	bit B_PAD_RIGHT, b
	jr z, .notRight
	inc d
	jr .try
.notRight
	bit B_PAD_LEFT, b
	jr z, .notLeft
	dec d
	jr .try
.notLeft
	bit B_PAD_UP, b
	jr z, .notUp
	dec e
	jr .try
.notUp
	inc e                       ; down
.try
	ld b, d
	ld c, e
	call EnemyAt
	jr nc, .noEnemy
	call HitEnemy
	ld a, SFX_HIT
	call SfxPlay
	jr EndTurn
.noEnemy
	call CellAddr
	ld a, [hl]
	cp T_ROCK
	ret z
	cp T_WALL
	ret z
	ld a, b
	ld [wPlayerX], a
	ld a, c
	ld [wPlayerY], a
	ld a, [hl]
	cp T_STAIRS
	jr z, .descend
	cp T_EMBER
	jr z, .win
	cp T_POTION
	jr z, .potion
	cp T_BLADE
	jr nz, .moved
	ld a, BLADE_ATK
	ld [wAtk], a
	jr .taken
.potion
	ld a, [wHP]
	add POTION_HEAL
	ld d, a
	ld a, [wMaxHP]
	cp d
	jr c, .full
	ld a, d
.full
	ld [wHP], a
.taken
	ld [hl], T_FLOOR
	call QueueCellPatch
	ld a, SFX_PICKUP
	jr .sound
.moved
	ld a, SFX_STEP
.sound
	call SfxPlay
	call CameraFollow
	call PlacePlayerSprite
	jr EndTurn
.descend
	ld a, SFX_STAIRS
	call SfxPlay
	ld hl, wDepth
	inc [hl]
	jp NewFloor
.win
	ld a, STATE_WON
	ld [wGameState], a
	call UpdateStatus
	jp ShowEnd

; The enemies' half of the turn, then the screen.
EndTurn:
	call EnemiesTurn
	call PlaceEnemySprites
	call UpdateStatus
	ld a, [wGameState]
	cp STATE_PLAY
	ret z
	jp ShowEnd

SECTION "Player variables", WRAM0
wRepeat: db
wHP:: db
wMaxHP:: db
wAtk:: db
wGameState:: db
