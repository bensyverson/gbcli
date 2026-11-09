; Enemies, the stage's wave script and the boss.
;
; The stage is a timeline: StageWaves lists (frame, kind, line) and each
; entry spawns one enemy at the right edge when the stage clock reaches its
; frame. At BOSS_TIME the boss flies in; destroying it clears the stage.
;
; Kinds: DART flies straight across fast and fires one aimed shot; WAVE
; weaves on a sine and fires once; POD flies in, stops, fires two rings
; and leaves. Each is 16x8, two sprites.

include "hardware.inc"
include "game.inc"

DEF SPAWN_X EQU 168                   ; just past the right edge
DEF GONE_X EQU 240                    ; X past 0 going left wraps up here
DEF FLASH_FRAMES EQU 4
DEF BOOM_FRAMES EQU 16

DEF BOSS_TIME EQU 2340                ; stage frames before the boss
DEF BOSS_HP EQU 48
DEF BOSS_STOP_X EQU 120
DEF BOSS_MIN_Y EQU 8
DEF BOSS_MAX_Y EQU FIELD_H - 24 - 8
DEF BOSS_DYING_FRAMES EQU 120
EXPORT BOSS_HP

; wBossState
DEF BOSS_NONE EQU 0
DEF BOSS_ENTER EQU 1
DEF BOSS_FIGHT EQU 2
DEF BOSS_DYING EQU 3
DEF BOSS_DEAD EQU 4
EXPORT BOSS_NONE, BOSS_ENTER, BOSS_FIGHT, BOSS_DYING, BOSS_DEAD

SECTION "Enemies", WRAM0, ALIGN[8]

wEnemies:: ds ENEMIES * E_STRIDE

SECTION "Stage state", WRAM0

wStageT:: dw                          ; frames since the stage began
wWavePtr: dw                          ; the next StageWaves entry
wBossState:: db
wBossX:: db
wBossY:: db
wBossDY: db                           ; 1 or -1
wBossHp:: db
wBossT: db                            ; frames in this state, wrapping
wBossFlash: db

SECTION "Enemy code", ROM0

EnemiesInit::
	ld hl, wEnemies
	ld bc, ENEMIES * E_STRIDE
	call ClearMem
	xor a
	ld [wStageT], a
	ld [wStageT + 1], a
	ld [wBossState], a
	ld [wBossFlash], a
	ld a, LOW(StageWaves)
	ld [wWavePtr], a
	ld a, HIGH(StageWaves)
	ld [wWavePtr + 1], a
	ret

; One frame: the clock and the script, every enemy, the boss.
EnemiesUpdate::
	; The clock.
	ld hl, wStageT
	inc [hl]
	jr nz, .ticked
	inc hl
	inc [hl]
.ticked
	; Spawn every entry whose frame has come.
.script
	ld hl, wWavePtr
	ld a, [hli]
	ld h, [hl]
	ld l, a
	ld a, [wStageT + 1]
	cp [hl]                           ; entries: frame (big-endian), kind, line
	jr c, .scripted
	jr nz, .spawn
	inc hl
	ld a, [wStageT]
	cp [hl]
	dec hl
	jr c, .scripted
.spawn
	inc hl
	inc hl
	ld b, [hl]                        ; kind
	inc hl
	ld c, [hl]                        ; line
	inc hl
	ld a, l
	ld [wWavePtr], a
	ld a, h
	ld [wWavePtr + 1], a
	call Spawn
	jr .script
.scripted
	; The boss's cue.
	ld a, [wBossState]
	and a
	jr nz, .bossDue
	ld a, [wStageT + 1]
	cp HIGH(BOSS_TIME)
	jr c, .bossDue
	ld a, [wStageT]
	cp LOW(BOSS_TIME)
	jr c, .bossDue
	call BossStart
.bossDue

	ld hl, wEnemies
.each
	ld a, [hl]
	and a
	call nz, EnemyStep
	ld a, l
	and %11110000
	add E_STRIDE
	ld l, a
	cp ENEMIES * E_STRIDE
	jr nz, .each
	jp BossUpdate

; Puts kind B at line C in a free slot, at the right edge.
Spawn::
	ld d, SPAWN_X
; The same at X = D.
SpawnAt::
	ld hl, wEnemies
.find
	ld a, [hl]
	and a
	jr z, .take
	ld a, l
	add E_STRIDE
	ld l, a
	cp ENEMIES * E_STRIDE
	jr nz, .find
	ret                               ; full: this one never comes
.take
	ld a, b
	ld [hli], a                       ; E_KIND
	ld a, d
	ld [hli], a                       ; E_X
	ld a, c
	ld [hli], a                       ; E_Y
	xor a
	ld [hli], a                       ; E_YF
	push hl
	ld hl, KindHp - 1
	ld a, l
	add b
	ld l, a
	adc h
	sub l
	ld h, a
	ld a, [hl]
	pop hl
	ld [hli], a                       ; E_HP
	xor a
	ld [hli], a                       ; E_T
	ld a, c
	ld [hli], a                       ; E_BASE
	xor a
	ld [hl], a                        ; E_FLASH
	ret

KindHp:
	db 1, 2, 6, 0                     ; DART, WAVE, POD, BOOM

; One enemy at HL (its E_KIND), any kind. Keeps HL.
EnemyStep:
	push hl
	inc l
	inc l
	inc l
	inc l
	inc l                             ; E_T
	inc [hl]
	ld e, [hl]                        ; E: frames alive
	ld a, l
	and %11110000
	ld l, a
	ld a, [hl]
	dec a
	jr z, Dart
	dec a
	jr z, Wave
	dec a
	jr z, Pod
	jp Boom

; X, Y of the enemy at HL's slot into B, C for firing from its nose.
MACRO nose
	ld a, l
	and %11110000
	inc a
	ld l, a
	ld b, [hl]                        ; E_X
	inc l
	ld c, [hl]                        ; E_Y
ENDM

Dart:
	; 1.5 px a frame: 2 on odd frames, 1 on even.
	inc l
	ld a, e
	and 1
	inc a
	ld b, a
	ld a, [hl]
	sub b
	ld [hl], a
	cp GONE_X
	jp nc, EnemyGone
	ld a, e
	cp 40
	jp nz, EnemyDraw
	push hl
	nose
	call AimDir
	ld e, 0
	call BulletFire
	pop hl
	jp EnemyDraw

Wave:
	inc l
	dec [hl]                          ; 1 px a frame
	ld a, [hl]
	cp GONE_X
	jp nc, EnemyGone
	; Y = base + sine(T * 4)
	ld a, e
	add a
	add a
	and 63
	add LOW(Sine)
	ld c, a
	ld a, 0
	adc HIGH(Sine)
	ld b, a
	ld a, [bc]
	ld b, a
	ld a, l
	add E_BASE - E_X
	ld l, a
	ld a, [hl]
	add b
	ld b, a
	ld a, l
	sub E_BASE - E_Y
	ld l, a
	ld [hl], b
	ld a, e
	cp 72
	jp nz, EnemyDraw
	push hl
	nose
	call AimDir
	ld e, 0
	call BulletFire
	pop hl
	jp EnemyDraw

Pod:
	inc l
	ld a, e
	cp 48
	jr nc, .stopped
	dec [hl]                          ; in, 1 px a frame
	jp EnemyDraw
.stopped
	cp 150
	jr c, .firing
	dec [hl]                          ; out, 2 px a frame
	dec [hl]
	ld a, [hl]
	cp GONE_X
	jp nc, EnemyGone
	jp EnemyDraw
.firing
	; Two full rings, each in halves on frames 64-65 and 112-113.
	cp 64
	jr z, .fire
	cp 65
	jr z, .fire
	cp 112
	jr z, .fire
	cp 113
	jp nz, EnemyDraw
.fire
	and 1                             ; even directions, then odd
	push hl
	push af
	nose
	ld a, b
	add 4
	ld b, a
	pop af
	ld e, 0
	call BulletRing
	pop hl
	jp EnemyDraw

Boom:
	ld a, e
	cp BOOM_FRAMES
	jr nc, EnemyGone
	; one sprite, two frames of animation
	inc l
	ld a, [hli]
	add OAM_X_OFS + 4
	ld c, a
	ld a, [hl]
	add OAM_Y_OFS
	ld b, a
	ld a, e
	rra
	rra
	rra
	and 1
	add T_BOOM
	ld d, a
	ld e, ATTR_ENEMY
	call SprPut
	pop hl
	ret

EnemyGone:
	ld a, l
	and %11110000
	ld l, a
	ld [hl], 0
	pop hl
	ret

; Two sprites for the enemy at HL's slot, flashing in OBP1 when hit.
EnemyDraw:
	ld a, l
	and %11110000
	ld l, a
	ld a, [hli]
	dec a
	add a
	add T_DART
	ld d, a                           ; tile
	ld a, [hli]
	add OAM_X_OFS
	ld c, a
	ld a, [hl]
	add OAM_Y_OFS
	ld b, a
	ld a, l
	add E_FLASH - E_Y
	ld l, a
	ld e, ATTR_ENEMY
	ld a, [hl]
	and a
	jr z, .noFlash
	dec [hl]
	ld e, ATTR_FLASH
.noFlash
	call SprPut
	ld a, c
	add 8
	jr c, .done                       ; the right half is past X 255
	ld c, a
	inc d
	call SprPut
.done
	pop hl
	ret

; Shots against enemies and the boss, enemies and the boss against the
; ship. Scores kills.
EnemiesCollide::
	ld hl, wShots
	ld c, SHOTS
.shot
	ld a, [hl]
	and a
	jr z, .nextShot
	push bc
	push hl
	add 6
	ld d, a                           ; the shot's tip
	inc hl
	ld a, [hl]
	add 3
	ld e, a
	call ShotHits
	pop hl
	pop bc
	jr nc, .nextShot
	ld [hl], 0                        ; the shot is spent
.nextShot
	inc hl
	inc hl
	dec c
	jr nz, .shot

	; Enemies and the boss against the ship's hitbox.
	ld a, [wInvuln]
	and a
	ret nz
	call PlayerHitbox
	ld d, b
	ld e, c
	ld hl, wEnemies
.enemy
	ld a, [hl]
	and a
	jr z, .nextEnemy
	cp KIND_BOOM
	jr z, .nextEnemy
	call InEnemy
	jr nc, .nextEnemy
	ld a, 1
	ld [wPlayerHit], a
	ret
.nextEnemy
	ld a, l
	and %11110000
	add E_STRIDE
	ld l, a
	cp ENEMIES * E_STRIDE
	jr nz, .enemy
	call InBoss
	ret nc
	ld a, 1
	ld [wPlayerHit], a
	ret

; Carry if point (D, E) is inside the enemy at HL (its E_KIND): its 16x8
; box. Keeps HL, DE.
InEnemy:
	push hl
	inc l
	ld a, d
	sub [hl]                          ; X - E_X
	cp 16
	jr nc, .out
	inc l
	ld a, e
	sub [hl]
	cp 8
.out
	pop hl
	ret

; Carry if point (D, E) is inside the fighting boss's 24x24 box. Keeps DE.
InBoss:
	ld a, [wBossState]
	cp BOSS_ENTER
	jr c, .no
	cp BOSS_DYING
	jr nc, .no
	ld a, [wBossX]
	ld b, a
	ld a, d
	sub b
	cp 24
	ret nc
	ld a, [wBossY]
	ld b, a
	ld a, e
	sub b
	cp 24
	ret
.no
	and a
	ret

; A shot's tip at (D, E): carry if it hit an enemy or the boss, which take
; the damage.
ShotHits:
	ld hl, wEnemies
.enemy
	ld a, [hl]
	and a
	jr z, .next
	cp KIND_BOOM
	jr z, .next
	call InEnemy
	jr c, .hit
.next
	ld a, l
	and %11110000
	add E_STRIDE
	ld l, a
	cp ENEMIES * E_STRIDE
	jr nz, .enemy
	call InBoss
	ret nc
	; The boss.
	ld a, FLASH_FRAMES
	ld [wBossFlash], a
	ld hl, wBossHp
	dec [hl]
	jr nz, .bossHurt
	ld a, BOSS_DYING
	ld [wBossState], a
	xor a
	ld [wBossT], a
	ld a, $01                         ; 10,000
	ld de, $0000
	call AddScore
	call SfxBoom
.bossHurt
	call HudBoss
	call SfxHit
	scf
	ret
.hit
	ld a, l
	add E_HP
	ld l, a
	dec [hl]
	jr z, .kill
	ld a, l
	add E_FLASH - E_HP
	ld l, a
	ld [hl], FLASH_FRAMES
	call SfxHit
	scf
	ret
.kill
	ld a, l
	and %11110000
	ld l, a
	ld a, [hl]                        ; the kind scores
	ld [hl], KIND_BOOM
	push hl
	ld hl, KindScore - 2
	add a
	add l
	ld l, a
	adc h
	sub l
	ld h, a
	ld a, [hli]
	ld d, a
	ld e, [hl]
	xor a
	call AddScore
	pop hl
	ld a, l
	add E_T
	ld l, a
	ld [hl], 0                        ; the explosion's clock
	call SfxBoom
	scf
	ret

; Points per kind, BCD hundreds and units: DART 100, WAVE 150, POD 500.
KindScore:
	db $01, $00
	db $01, $50
	db $05, $00

; ---------------------------------------------------------------------------
; The boss: 24x24, nine sprites. It flies in, then rides up and down,
; alternating an aimed fan and a ring, and adds aimed shots below half HP.

BossStart:
	ld a, BOSS_ENTER
	ld [wBossState], a
	ld a, SPAWN_X
	ld [wBossX], a
	ld a, (FIELD_H - 24) / 2
	ld [wBossY], a
	ld a, 1
	ld [wBossDY], a
	ld a, BOSS_HP
	ld [wBossHp], a
	xor a
	ld [wBossT], a
	jp HudBoss

BossUpdate:
	ld a, [wBossState]
	and a
	ret z
	cp BOSS_DEAD
	ret z
	ld hl, wBossT
	inc [hl]
	ld e, [hl]                        ; E: frames in this state
	cp BOSS_ENTER
	jr nz, .notEntering
	ld hl, wBossX
	dec [hl]
	ld a, [hl]
	cp BOSS_STOP_X
	jp nz, BossDraw
	ld a, BOSS_FIGHT
	ld [wBossState], a
	xor a
	ld [wBossT], a
	jp BossDraw
.notEntering
	cp BOSS_DYING
	jp z, BossDying

	; Fighting: ride up and down.
	ld a, [wBossDY]
	ld b, a
	ld hl, wBossY
	ld a, [hl]
	add b
	ld [hl], a
	cp BOSS_MIN_Y
	jr z, .turn
	cp BOSS_MAX_Y
	jr nz, .rode
.turn
	ld a, b
	cpl
	inc a
	ld [wBossDY], a
.rode
	; Every 64 frames a pattern: a fan of five on even, a ring on odd,
	; the ring in halves over two frames.
	ld a, e
	and 63
	cp 2
	jr nc, .noPattern
	ld a, e
	and 64
	jr nz, .ring
	ld a, e
	and 1
	jr nz, .noPattern
	call BossGun
	call AimDir
	ld d, b
	ld b, 5
	ld e, 0
	call BulletFan
	jr .noPattern
.ring
	call BossGun
	ld a, e                           ; even directions, then odd
	and 1
	ld e, 1
	call BulletRing
.noPattern
	; Below half HP, an aimed fast shot every 16 frames.
	ld a, [wBossHp]
	cp BOSS_HP / 2
	jp nc, BossDraw
	ld a, [wBossT]
	and 15
	cp 8
	jp nz, BossDraw
	call BossGun
	call AimDir
	ld e, 1
	call BulletFire
	jp BossDraw

; The boss's gun, its center: B = X, C = Y. Keeps E.
BossGun:
	ld a, [wBossX]
	add 8
	ld b, a
	ld a, [wBossY]
	add 8
	ld c, a
	ret

BossDying:
	; An explosion somewhere on the hull every 8 frames, then gone.
	ld a, e
	cp BOSS_DYING_FRAMES
	jr c, .burning
	ld a, BOSS_DEAD
	ld [wBossState], a
	jp HudBossOff
.burning
	and 7
	jp nz, BossDraw
	ld a, e
	and %00011000
	ld c, a
	ld a, [wBossY]
	add c
	ld c, a                           ; Y: down the hull by the clock
	ld a, e
	swap a
	and %00000011
	ld d, a
	add a
	add a
	add a                             ; 0, 8, 16, 24 across
	ld d, a
	ld a, [wBossX]
	add d
	ld d, a
	ld b, KIND_BOOM
	call SpawnAt
	jp BossDraw

; Nine sprites, flashing in OBP1 when hit; skipped every other frame
; while dying, so it flickers out.
BossDraw:
	ld a, [wBossState]
	cp BOSS_DYING
	jr nz, .solid
	ld a, [wBossT]
	and 2
	ret nz
.solid
	ld e, ATTR_BOSS
	ld hl, wBossFlash
	ld a, [hl]
	and a
	jr z, .noFlash
	dec [hl]
	ld e, ATTR_FLASH
.noFlash
	ld d, T_BOSS
	ld a, [wBossY]
	add OAM_Y_OFS
	ld b, a
	ld h, 3
.row
	ld a, [wBossX]
	add OAM_X_OFS
	ld c, a
	ld l, 3
.col
	push hl
	call SprPut
	pop hl
	inc d
	ld a, c
	add 8
	ld c, a
	dec l
	jr nz, .col
	ld a, b
	add 8
	ld b, a
	dec h
	jr nz, .row
	ret

; Signed offsets of a WAVE from its center line, 64 steps a cycle.
Sine:
	FOR I, 64
		db MUL(20.0, SIN(I * 1.0 / 64)) >> 16
	ENDR

; The stage: frame (big-endian), kind, line. Ends with a frame never reached.
MACRO wave
	db HIGH(\1), LOW(\1), \2, \3
ENDM
MACRO row ; first frame, kind, line, count, frames apart
	FOR J, \4
		wave (\1) + J * (\5), \2, \3
	ENDR
ENDM

StageWaves:
	wave 60, KIND_DART, 24
	wave 84, KIND_DART, 56
	wave 108, KIND_DART, 88
	row 200, KIND_WAVE, 40, 4, 20
	row 330, KIND_WAVE, 92, 4, 20
	wave 460, KIND_POD, 32
	wave 480, KIND_DART, 100
	wave 510, KIND_DART, 80
	wave 600, KIND_DART, 16
	wave 612, KIND_DART, 40
	wave 624, KIND_DART, 64
	wave 636, KIND_DART, 88
	wave 648, KIND_DART, 112
	wave 720, KIND_POD, 96
	row 760, KIND_WAVE, 48, 4, 18
	wave 900, KIND_POD, 24
	wave 905, KIND_POD, 104
	row 1060, KIND_DART, 30, 3, 32
	row 1076, KIND_DART, 90, 3, 32
	row 1200, KIND_WAVE, 64, 6, 16
	wave 1380, KIND_POD, 40
	wave 1400, KIND_POD, 88
	wave 1420, KIND_DART, 64
	wave 1560, KIND_DART, 20
	wave 1570, KIND_DART, 38
	wave 1580, KIND_DART, 56
	wave 1590, KIND_DART, 74
	wave 1600, KIND_DART, 92
	wave 1610, KIND_DART, 110
	row 1680, KIND_WAVE, 30, 4, 24
	row 1692, KIND_WAVE, 100, 4, 24
	wave 1850, KIND_POD, 64
	wave 1870, KIND_DART, 20
	wave 1890, KIND_DART, 108
	row 2000, KIND_WAVE, 50, 5, 18
	wave 2100, KIND_POD, 30
	wave 2110, KIND_POD, 96
	wave $FFFF, 0, 0
