; Driving: speed and steering from the joypad, the track's curves and hills
; eased in as the car reaches them, and the player's car.
;
; Controls: A or Up accelerates, B or Down brakes, Left and Right steer.
; Off the road (|wPlayerX| past ROAD_EDGE) the car can't go faster than
; OFFROAD_SPEED. objects.asm slows it on a bump and stops it on a crash.

include "hardware.inc"
include "images.inc"
include "road_geometry.inc"
include "race.inc"

DEF MAX_SPEED EQU 64            ; wPos units a frame: 4 depth units
DEF ACCEL EQU 1
DEF BRAKE EQU 3
DEF STEER EQU 2                 ; pixels a frame
DEF PLAYER_LIMIT EQU 112        ; how far off center the car may go
DEF ROAD_EDGE EQU 90            ; past this the car is on the grass
DEF OFFROAD_SPEED EQU 24
DEF EASE_FRAMES EQU 4           ; frames per level step toward the track's targets

SECTION "Drive", ROM0

InitDrive::
	ld a, FLAT
	ld [wHill], a
	ld [wCurve], a
	xor a
	ld [wSpeed], a
	ld [wPlayerX], a
	ld [wFinished], a
	ld [wStunned], a
	ld hl, wPos
	ld [hli], a
	ld [hli], a
	ld [hl], a
	ld hl, Track
	call LoadSegment
	jp PlaceCar

; Called once a frame. Only in RACE does the joypad drive; otherwise the car
; rolls to a stop.
UpdateDrive::
	call UpdateSpeed
	call CurveTimesSpeed
	ld a, l
	ld [wCurveSpeed], a
	ld a, h
	ld [wCurveSpeed + 1], a
	call UpdateTrack
	ld a, [wState]
	cp STATE_RACE
	call z, UpdateSteering
	call UpdateSky
	ret

UpdateSpeed:
	ld a, [wSpeed]
	ld b, a
	ld a, [wStunned]
	and a
	jr z, .notStunned
	dec a
	ld [wStunned], a
	jr .coast
.notStunned
	ld a, [wState]
	cp STATE_RACE
	jr nz, .brake
	ldh a, [hPad]
	and PAD_A | PAD_UP
	jr z, .noGas
	ld a, b
	add ACCEL
	cp MAX_SPEED
	jr c, .limit
	ld a, MAX_SPEED
	jr .limit
.noGas
	ldh a, [hPad]
	and PAD_B | PAD_DOWN
	jr z, .coast
.brake
	ld a, b
	sub BRAKE
	jr nc, .limit
	xor a
	jr .limit
.coast
	ld a, [wFrame]
	and 7
	ld a, b
	jr nz, .limit
	and a
	jr z, .limit
	dec a
.limit
	; On the grass, slide down to OFFROAD_SPEED.
	ld b, a
	ld a, [wPlayerX]
	bit 7, a
	jr z, .positive
	cpl
	inc a
.positive
	cp ROAD_EDGE
	ld a, b
	jr c, .store
	cp OFFROAD_SPEED + 1
	jr c, .store
	sub 2
.store
	ld [wSpeed], a
	; wPos += speed (24 bits)
	ld hl, wPos
	add [hl]
	ld [hli], a
	ret nc
	inc [hl]
	ret nz
	inc hl
	inc [hl]
	ret

; Segments of a length (in 256 wPos units), a curve level and a hill level
; to ease toward. The end of the list is the finish line.
UpdateTrack:
	ld a, [wSpeed]
	ld b, a
	ld hl, wSegLeft
	ld a, [hl]
	sub b
	ld [hli], a
	jr nc, .ease
	dec [hl]
	bit 7, [hl]
	jr z, .ease
	ld hl, wSegPtr
	ld a, [hli]
	ld h, [hl]
	ld l, a
	call LoadSegment
.ease
	ld a, [wFrame]
	and EASE_FRAMES - 1
	ret nz
	ld hl, wCurve
	ld a, [wCurveTarget]
	call EaseToward
	ld hl, wHill
	ld a, [wHillTarget]
	; fall through

; Moves [HL] one step toward A.
EaseToward:
	cp [hl]
	ret z
	jr c, .down
	inc [hl]
	ret
.down
	dec [hl]
	ret

; Loads the segment at HL and points wSegPtr past it. At the end of the
; track, sets wFinished and drives straight on.
LoadSegment:
	ld a, [hl]
	and a
	jr nz, .load
	ld a, 1
	ld [wFinished], a
	ld hl, RunOut
	ld a, [hl]
.load
	ld [wSegLeft + 1], a
	xor a
	ld [wSegLeft], a
	inc hl
	ld a, [hli]
	ld [wCurveTarget], a
	ld a, [hli]
	ld [wHillTarget], a
	ld a, l
	ld [wSegPtr], a
	ld a, h
	ld [wSegPtr + 1], a
	ret

; Left and Right move the car across the road; the curve pushes it outward.
UpdateSteering:
	ld a, [wSpeed]
	and a
	ret z
	ld a, [wPlayerX]
	ld b, a
	ldh a, [hPad]
	bit B_PAD_LEFT, a
	jr z, .notLeft
	ld a, b
	sub STEER
	ld b, a
.notLeft
	ldh a, [hPad]
	bit B_PAD_RIGHT, a
	jr z, .notRight
	ld a, b
	add STEER
	ld b, a
.notRight
	; Drift: (curve - FLAT) * speed / 1024 pixels outward (left on a right
	; bend), the fraction kept in wDrift.
	call LoadCurveSpeed         ; HL = (curve - FLAT) * speed, signed
	sra h
	rr l
	sra h
	rr l
	ld a, [wDrift]
	sub l
	ld [wDrift], a
	ld a, b
	sbc h
	; Clamp to +/- PLAYER_LIMIT.
	bit 7, a
	jr nz, .negative
	cp PLAYER_LIMIT
	jr c, .store
	ld a, PLAYER_LIMIT
	jr .store
.negative
	cp -PLAYER_LIMIT
	jr nc, .store
	ld a, -PLAYER_LIMIT
.store
	ld [wPlayerX], a
	ret

; The sky slides against the curve: (curve - FLAT) * speed / 64 in 8.8.
UpdateSky:
	call LoadCurveSpeed
	sra h
	rr l
	sra h
	rr l
	ld a, [wSkyX]
	add l
	ld [wSkyX], a
	ld a, [wSkyX + 1]
	adc h
	ld [wSkyX + 1], a
	ret

; HL = this frame's (wCurve - FLAT) * wSpeed. Keeps B.
LoadCurveSpeed:
	ld a, [wCurveSpeed]
	ld l, a
	ld a, [wCurveSpeed + 1]
	ld h, a
	ret

; HL = (wCurve - FLAT) * wSpeed, signed.
CurveTimesSpeed:
	ld hl, 0
	ld a, [wSpeed]
	ld e, a
	ld d, 0
	ld a, [wCurve]
	sub FLAT
	ret z
	jr c, .left
.right
	add hl, de
	dec a
	jr nz, .right
	ret
.left
	ld c, a
.leftLoop
	ld a, l
	sub e
	ld l, a
	ld a, h
	sbc d
	ld h, a
	inc c
	jr nz, .leftLoop
	ret

; The car's four objects take OAM slots 0-3 for good: the first slots win
; when objects overlap, and the 10-per-line limit drops later slots first.
PlaceCar:
	ld hl, CarFrame0
	ld a, [hli]
	ld b, a
	ld de, wShadowOAM
.obj
	ld a, [hli]
	add CAR_Y + OAM_Y_OFS
	ld [de], a
	inc e
	ld a, [hli]
	add CAR_X + OAM_X_OFS
	ld [de], a
	inc e
	ld a, [hli]
	add CAR_BASE
	ld [de], a
	inc e
	ld a, [hli]
	ld [de], a
	inc e
	dec b
	jr nz, .obj
	ret

; length (x256 wPos units, 16 depth units), curve level, hill level.
; 625 units: 10,000 depth units, about 42 s at full speed.
Track:
	db 40, FLAT, FLAT
	db 40, FLAT + 10, FLAT
	db 30, FLAT, FLAT + 12
	db 40, FLAT - 12, FLAT + 12
	db 30, FLAT - 12, FLAT - 6
	db 30, FLAT, FLAT - 2
	db 50, FLAT + 14, FLAT
	db 30, FLAT, FLAT
	db 40, FLAT - 8, FLAT + 8
	db 40, FLAT + 8, FLAT
	db 30, FLAT, FLAT - 4
	db 50, FLAT - 14, FLAT
	db 40, FLAT + 12, FLAT + 10
	db 40, FLAT, FLAT
	db 45, FLAT - 6, FLAT - 8
	db 50, FLAT, FLAT
	db 0
; Past the finish: straight and level, for as long as the car rolls.
RunOut:
	db 255, FLAT, FLAT

SECTION "Drive variables", WRAM0
wSpeed:: db
wSegLeft: dw
wSegPtr: dw
wCurveTarget: db
wHillTarget: db
wDrift: db
wCurveSpeed: dw                 ; (wCurve - FLAT) * wSpeed, once a frame
wFinished:: db                  ; nonzero once the car crosses the finish
wStunned:: db                   ; frames the car can't accelerate (a crash)
