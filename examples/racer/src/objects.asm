; Rival cars and roadside trees: where they are on the track, where they
; land on screen, which pre-scaled size they use, and what happens when the
; player's car meets one.
;
; An object has a depth (its track distance ahead of the player, in depth
; units) and a lane. Depth picks a road strip (DepthStrip), the strip picks
; the screen line (searched in this frame's hill profile) and a size
; (StripSize), and the line's SCX in the back buffer plus the lane's offset
; at that strip (LaneX) gives its x. So objects sit on the road the HBlank
; interrupt will draw next frame, curves and hills included.

include "hardware.inc"
include "images.inc"
include "road_geometry.inc"
include "race.inc"

DEF NUM_RIVALS EQU 10
DEF RIVAL_SPEED EQU 40          ; wPos units a frame (2.5 depth units)
DEF RIVAL_FIRST EQU 400         ; depth of the first rival at the start
DEF RIVAL_GAP EQU 500           ; depth units between rivals
DEF BUMP_SPEED EQU 24           ; the player's speed after hitting a rival
DEF TREE_FIRST EQU 64           ; depth of the first tree
DEF TREE_GAP EQU 96             ; depth units between trees, sides alternating
DEF TREES_SHOWN EQU 3           ; at most this many trees are in view
DEF NEAREST EQU 24              ; nearer than this an object has passed the car
DEF HIT_NEAR EQU 25             ; the car's depth span, for collisions
DEF HIT_FAR EQU 30
DEF HIT_RIVAL_X EQU 22          ; centers closer than this collide
DEF HIT_TREE_X EQU 14
DEF CRASH_FRAMES EQU 45
DEF CRASH_BOUNCE EQU 28         ; pixels the car is thrown back toward the road
DEF LANE_LEFT_SIDE EQU 0
DEF LANE_RIGHT_SIDE EQU 3

DEF KIND_RIVAL EQU 0
DEF KIND_TREE EQU 1

SECTION "Objects", ROM0

InitObjects::
	xor a
	ld hl, wRivalTravel
	ld [hli], a
	ld [hli], a
	ld [hl], a
	ld [wRivalK], a
	ld [wTreeK], a
	ld [wTreeK + 1], a
	ret

; Moves the rivals, draws every object in view into shadow OAM after the
; player's car, and applies collisions. Call it after ComputeRoad.
UpdateObjects::
	; wD = wPos / 16: the player's depth on the track.
	ld a, [wPos]
	swap a
	and $0F
	ld b, a
	ld a, [wPos + 1]
	swap a
	and $F0
	or b
	ld [wD], a
	ld a, [wPos + 1]
	swap a
	and $0F
	ld b, a
	ld a, [wPos + 2]
	swap a
	and $F0
	or b
	ld [wD + 1], a

	ld a, LOW(wShadowOAM) + 4 * CAR_OBJECTS
	ld [wOamCursor], a

	call DrawRivals
	call DrawTrees

	; Hide the slots no object took.
	ld a, [wOamCursor]
	ld l, a
	ld h, HIGH(wShadowOAM)
.clear
	ld a, l
	cp LOW(wShadowOAM) + OAM_SIZE
	ret nc
	xor a
	ld [hli], a
	inc l
	inc l
	inc l
	jr .clear


; The rivals all drive at RIVAL_SPEED, so they share one distance travelled,
; wRivalTravel: rival k is at depth RIVAL_FIRST + k * RIVAL_GAP + wRivalTravel
; / 16, in lane 1 or 2. Their gap is wider than the view, so at most one is
; in it: the first one not yet passed, wRivalK.
DrawRivals:
	ld a, [wState]
	cp STATE_READY
	jr z, .moved
	ld hl, wRivalTravel
	ld a, [hl]
	add RIVAL_SPEED
	ld [hli], a
	jr nc, .moved
	inc [hl]
	jr nz, .moved
	inc hl
	inc [hl]
.moved
	; wRivalOffset = wRivalTravel / 16
	ld a, [wRivalTravel]
	swap a
	and $0F
	ld b, a
	ld a, [wRivalTravel + 1]
	swap a
	and $F0
	or b
	ld [wRivalOffset], a
	ld a, [wRivalTravel + 1]
	swap a
	and $0F
	ld b, a
	ld a, [wRivalTravel + 2]
	swap a
	and $F0
	or b
	ld [wRivalOffset + 1], a
.skip
	ld a, [wRivalK]
	cp NUM_RIVALS
	ret nc                      ; all passed
	call RivalZ
	bit 7, b
	jr nz, .passed
	ld a, b
	and a
	ret nz                      ; 256 or more ahead
	ld a, c
	cp NEAREST
	jr nc, .draw
.passed
	ld hl, wRivalK
	inc [hl]
	jr .skip
.draw
	ld a, [wRivalK]
	and 1
	inc a
	ld [wObjLane], a
	ld a, KIND_RIVAL
	ld [wObjKind], a
	ld a, c
	jp DrawObject

; BC = depth of rival wRivalK - wD.
RivalZ:
	ld a, [wRivalK]
	add a
	ld e, a
	ld d, 0
	ld hl, RivalStarts
	add hl, de
	ld a, [hli]
	ld h, [hl]
	ld l, a
	ld a, [wRivalOffset]
	ld e, a
	ld a, [wRivalOffset + 1]
	ld d, a
	add hl, de
	ld a, [wD]
	ld c, a
	ld a, l
	sub c
	ld c, a
	ld a, [wD + 1]
	ld b, a
	ld a, h
	sbc b
	ld b, a
	ret

RivalStarts:
FOR K, NUM_RIVALS
	dw RIVAL_FIRST + K * RIVAL_GAP
ENDR

; Tree k stands at depth TREE_FIRST + k * TREE_GAP, left for even k.
DrawTrees:
	; Skip the trees the car has passed: while depth(k) - wD < NEAREST.
.skip
	call TreeZ                  ; BC = depth(wTreeK) - wD
	bit 7, b
	jr nz, .passed
	ld a, b
	and a
	jr nz, .draw
	ld a, c
	cp NEAREST
	jr nc, .draw
.passed
	ld hl, wTreeK
	inc [hl]
	jr nz, .skip
	inc hl
	inc [hl]
	jr .skip
.draw
	ld a, TREES_SHOWN
	ld [wTreeCount], a
	ld a, [wTreeK]
	ld [wTreeJ], a
	ld a, [wTreeK + 1]
	ld [wTreeJ + 1], a
.tree
	ld hl, wTreeJ
	call TreeZAt
	ld a, b
	and a
	ret nz                      ; 256 or more ahead: the rest are farther
	ld a, [wTreeJ]
	and 1
	ld a, LANE_LEFT_SIDE
	jr z, .lane
	ld a, LANE_RIGHT_SIDE
.lane
	ld [wObjLane], a
	ld a, KIND_TREE
	ld [wObjKind], a
	ld a, c
	call DrawObject
	ld hl, wTreeJ
	inc [hl]
	jr nz, .counted
	inc hl
	inc [hl]
.counted
	ld hl, wTreeCount
	dec [hl]
	jr nz, .tree
	ret

; BC = depth of tree [HL] (a 16-bit index) - wD. TreeZ uses wTreeK.
TreeZ:
	ld hl, wTreeK
TreeZAt:
	ld a, [hli]
	ld h, [hl]
	ld l, a
	; HL = k * 96 = k * 64 + k * 32
	ASSERT TREE_GAP == 96
	add hl, hl
	add hl, hl
	add hl, hl
	add hl, hl
	add hl, hl                  ; k * 32
	ld b, h
	ld c, l
	add hl, hl                  ; k * 64
	add hl, bc
	ld bc, TREE_FIRST
	add hl, bc
	ld a, [wD]
	ld c, a
	ld a, l
	sub c
	ld c, a
	ld a, [wD + 1]
	ld b, a
	ld a, h
	sbc b
	ld b, a
	ret

; Draws the object of kind wObjKind in lane wObjLane at depth A (0-255),
; and checks it against the player's car.
DrawObject:
	ld [wObjZ], a
	ld e, a
	ld d, 0
	ld hl, DepthStrip
	add hl, de
	ld a, [hl]
	cp $FF
	ret z                       ; below the screen
	ld [wObjStrip], a
	ld e, a
	ld hl, StripSize
	add hl, de
	ld a, [hl]
	cp $FF
	ret z                       ; too far to draw
	ld [wObjSize], a

	; The line: the first from the horizon down whose strip is at least
	; this one, by bisection; none if the horizon's strip is already nearer
	; (the object is behind a crest).
	ld a, [wHill]
	call LevelOffsetObj
	ld de, HillProfiles - FIRST_LINE
	add hl, de                  ; HL + y = the profile's byte for line y
	ld a, [wHorizon]
	ld c, a                     ; lo
	ld b, 143                   ; hi
	; behind a crest?
	ld e, c
	ld d, 0
	push hl
	add hl, de
	ld a, [wObjStrip]
	cp [hl]
	pop hl
	ret c
.bisect
	ld a, c
	cp b
	jr z, .found
	add b
	rra                         ; (lo + hi) / 2, the carry holds bit 8
	ld e, a
	push hl
	add hl, de
	ld a, [wObjStrip]
	cp [hl]                     ; strip < profile[mid]? carry; equal: z
	pop hl
	jr z, .atOrPast
	jr c, .atOrPast
	ld c, e
	inc c                       ; lo = mid + 1
	jr .bisect
.atOrPast
	ld b, e                     ; hi = mid
	jr .bisect
.found
	ld a, c
	ld [wObjY], a

	; x: the road's center on line y (its SCX, in the back buffer at index
	; y - 1), as a signed offset from the screen's center, plus the lane's.
	dec a
	ld l, a
	ld a, [wBack]
	ld h, a
	ld a, ROAD_SCX0
	sub [hl]                    ; center - 80, signed
	ld c, a
	ld a, [wObjLane]
	ld e, a
	ld d, 0
	ld hl, LaneX
	; HL += lane * ROAD_ROWS (80) + strip
	ASSERT ROAD_ROWS == 80
	ld a, e
	add a
	add a
	add e                       ; lane * 5
	swap a                      ; * 16 (lane <= 3, so no high bits)
	ld e, a
	add hl, de
	ld a, [wObjStrip]
	ld e, a
	add hl, de
	; BC = (center - 80) + lane offset, both signed 8-bit, summed in 16
	ld a, c
	ld c, a
	add a
	sbc a
	ld b, a                     ; BC = sign-extended center offset
	ld a, [hl]
	ld l, a
	add a
	sbc a
	ld h, a
	add hl, bc                  ; HL = x - 80, signed
	ld a, l
	add 80
	ld l, a
	jr nc, .xDone
	inc h
.xDone
	; HL = the object's screen x. Draw only from -16 to 175.
	ld a, l
	add 16
	ld e, a
	ld a, h
	adc 0
	ret nz                      ; < -16 or >= 240
	ld a, e
	cp 176 + 16
	ret nc
	ld a, l
	ld [wObjX], a

	call Collide

	; The frame: kind's frame list, entry wObjSize.
	; Each kind's frames are anchored at their bottom center.
	ld a, [wObjKind]
	and a
	ld hl, RivalFrames
	ld d, RIVAL_BASE
	ld e, OAM_PAL0
	ld b, OAM_X_OFS - RIVAL_FRAME_WIDTH / 2
	ld c, OAM_Y_OFS - RIVAL_FRAME_HEIGHT
	jr z, .kindKnown
	ld hl, TreeFrames
	ld d, TREE_BASE
	ld e, OAM_PAL1
	ld b, OAM_X_OFS - TREE_FRAME_WIDTH / 2
	ld c, OAM_Y_OFS - TREE_FRAME_HEIGHT
.kindKnown
	ld a, d
	ld [wObjTile], a
	ld a, e
	ld [wObjAttr], a
	ld a, [wObjX]
	add b
	ld [wObjX], a
	ld a, [wObjY]
	add c
	ld [wObjY], a
	ld a, [wObjSize]
	add a
	ld e, a
	ld d, 0
	add hl, de
	ld a, [hli]
	ld h, [hl]
	ld l, a                     ; HL = the frame: count, then y, x, tile, attr
	ld a, [hli]
	ld b, a                     ; objects
	ld a, [wOamCursor]
	ld e, a
	ld d, HIGH(wShadowOAM)
.obj
	ld a, e
	cp LOW(wShadowOAM) + OAM_SIZE
	jr nc, .full
	; y and x: the frame's corner in OAM terms (set above), plus the
	; object's place in the frame
	ld a, [wObjY]
	add [hl]
	inc hl
	ld [de], a
	inc e
	ld a, [wObjX]
	add [hl]
	inc hl
	ld [de], a
	inc e
	ld a, [wObjTile]
	add [hl]
	inc hl
	ld [de], a
	inc e
	ld a, [wObjAttr]
	or [hl]
	inc hl
	ld [de], a
	inc e
	dec b
	jr nz, .obj
.full
	ld a, e
	ld [wOamCursor], a
	ret

; If the object is level with the car (depth HIT_NEAR-HIT_FAR) and its
; center is near the car's: a rival slows the car to BUMP_SPEED, a tree
; stops it dead for CRASH_FRAMES.
Collide:
	ld a, [wObjZ]
	cp HIT_NEAR
	ret c
	cp HIT_FAR + 1
	ret nc
	ld a, [wObjX]
	sub CAR_CENTER
	jr nc, .abs
	cpl
	inc a
.abs
	ld b, a
	ld a, [wObjKind]
	and a
	jr nz, .tree
	ld a, b
	cp HIT_RIVAL_X
	ret nc
	ld a, [wSpeed]
	cp BUMP_SPEED + 1
	ret c
	ld a, BUMP_SPEED
	ld [wSpeed], a
	ld hl, wBumps
	inc [hl]
	ret
.tree
	ld a, b
	cp HIT_TREE_X
	ret nc
	ld a, [wStunned]
	and a
	ret nz
	xor a
	ld [wSpeed], a
	ld a, CRASH_FRAMES
	ld [wStunned], a
	ld hl, wCrashes
	inc [hl]
	; Bounce back toward the road, clear of the tree.
	ld a, [wPlayerX]
	bit 7, a
	jr z, .fromRight
	add CRASH_BOUNCE
	ld [wPlayerX], a
	ret
.fromRight
	sub CRASH_BOUNCE
	ld [wPlayerX], a
	ret

; HL = A * 128.
LevelOffsetObj:
	srl a
	ld h, a
	ld a, 0
	rra
	ld l, a
	ret

SECTION "Object variables", WRAM0
wRivalTravel: ds 3              ; how far the rivals have driven, wPos units
wRivalOffset: dw                ; wRivalTravel / 16
wRivalK: db                     ; the first rival not yet passed
wTreeK: dw                      ; the nearest tree not yet passed
wTreeJ: dw
wTreeCount: db
wD:: dw                         ; the player's depth, wPos / 16
wOamCursor: db
wObjZ: db
wObjLane: db
wObjKind: db
wObjStrip: db
wObjSize: db
wObjY: db
wObjX: db
wObjTile: db
wObjAttr: db
wBumps:: db                     ; rivals hit this race
wCrashes:: db                   ; trees hit this race
