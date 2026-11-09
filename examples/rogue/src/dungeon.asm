; The floor generator: rooms placed at random without touching, each joined
; to the one before it by an L-shaped corridor, so every floor cell is
; reachable from the first room. The player starts in the first room and the
; stairs go in the last. Seeded from wSeed and wDepth: one seed, one dungeon.

include "hardware.inc"
include "game.inc"

SECTION "Dungeon", ROM0

; Generates floor wDepth into wMap, with the rooms in wRooms, the player's
; start in wPlayerX/Y and the stairs in wStairsX/Y.
GenerateFloor::
	; Seed: the run's seed, stirred by the depth.
	ld a, [wDepth]
	ld b, a
	ld a, [wSeed]
	add b
	ld l, a
	ld a, b
	swap a
	add a                       ; depth * 32
	ld b, a
	ld a, [wSeed + 1]
	xor b
	ld h, a
	call SeedRand

	; Rock everywhere, half the map a frame.
	ld hl, wMap
	call FillHalfMap
	call MaybeYield
	ld hl, wMap + MAP_SIZE / 2
	call FillHalfMap
	call MaybeYield

	xor a
	ld [wRoomCount], a
	ld [wAttempts], a
.try
	call MaybeYield
	ld a, [wRoomCount]
	cp MAX_ROOMS
	jr z, .roomsDone
	ld a, [wAttempts]
	inc a
	ld [wAttempts], a
	cp MAX_ATTEMPTS
	jr c, .attempt
	ld a, [wRoomCount]
	cp MIN_ROOMS
	jr nc, .roomsDone
.attempt
	ld a, ROOM_W_RANGE
	call RandBelow
	add ROOM_MIN_W
	ld [wNewW], a
	ld a, ROOM_H_RANGE
	call RandBelow
	add ROOM_MIN_H
	ld [wNewH], a
	; x in [1, MAP_W - 1 - w], so the room keeps a rock border.
	ld a, [wNewW]
	ld b, a
	ld a, MAP_W - 1
	sub b
	call RandBelow
	inc a
	ld [wNewX], a
	ld a, [wNewH]
	ld b, a
	ld a, MAP_H - 1
	sub b
	call RandBelow
	inc a
	ld [wNewY], a
	call RoomOverlaps
	jr c, .try
	call AddRoom
	jr .try

.roomsDone
	call MaybeYield
	; The player starts at the first room's center.
	ld hl, wRooms
	call RoomCenter
	ld a, b
	ld [wPlayerX], a
	ld a, c
	ld [wPlayerY], a
	; The stairs go at the center of the room farthest from the start, by
	; x + y distance.
	xor a
	ld [wBest], a
	ld a, [wRoomCount]
	ld [wLeft], a
	ld hl, wRooms
.farthest
	push hl
	call RoomCenter
	ld a, [wPlayerX]
	sub b
	jr nc, .dxPos
	cpl
	inc a
.dxPos
	ld d, a
	ld a, [wPlayerY]
	sub c
	jr nc, .dyPos
	cpl
	inc a
.dyPos
	add d
	ld d, a
	ld a, [wBest]
	cp d
	jr nc, .notFarther
	ld a, d
	ld [wBest], a
	ld a, b
	ld [wStairsX], a
	ld a, c
	ld [wStairsY], a
.notFarther
	pop hl
	ld de, ROOM_SIZE
	add hl, de
	ld a, [wLeft]
	dec a
	ld [wLeft], a
	jr nz, .farthest
	ld a, [wStairsX]
	ld b, a
	ld a, [wStairsY]
	ld c, a
	call CellAddr
	ld a, [wDepth]
	cp LAST_DEPTH
	ld a, T_STAIRS
	jr nz, .putStairs
	ld a, T_EMBER               ; the last floor: the goal instead of stairs
.putStairs
	ld [hl], a
	call PlaceItems
	call SpawnEnemies
.done
	ret

; Lays a potion on every floor (two from floor 3 on), and the blade on
; floor BLADE_DEPTH + 1, each on a floor cell of a room other than the first.
PlaceItems:
	ld a, T_POTION
	call PlaceItem
	ld a, [wDepth]
	cp 2
	jr c, .onePotion
	ld a, T_POTION
	call PlaceItem
.onePotion
	ld a, [wDepth]
	cp BLADE_DEPTH
	ret nz
	ld a, T_BLADE
	; fall through

; Puts item A on a random floor cell of a room other than the first.
PlaceItem:
	ld [wItem], a
.retry
	call RandomRoomCell
	call CellAddr
	ld a, [hl]
	cp T_FLOOR
	jr nz, .retry
	ld a, [wItem]
	ld [hl], a
	ret

; Sets carry if the new room (wNewX/Y/W/H) touches or overlaps any room so
; far, counting a one-cell border around each.
RoomOverlaps:
	ld a, [wRoomCount]
	and a
	ret z                       ; no rooms: no carry
	ld b, a
	ld hl, wRooms
.room
	; Apart if newX > r.x + r.w, or r.x > newX + newW, or the same in y.
	ld a, [hli]                 ; r.x
	ld c, a
	ld a, [hli]                 ; r.y
	ld d, a
	ld a, [hli]                 ; r.w
	add c
	ld e, a                     ; r.x + r.w
	ld a, [wNewX]
	cp e
	jr z, .xNear
	jr nc, .apartSkipH          ; newX > r.x + r.w
.xNear
	ld a, [wNewX]
	ld e, a
	ld a, [wNewW]
	add e                       ; newX + newW
	cp c
	jr c, .apartSkipH           ; newX + newW < r.x
	ld a, [hli]                 ; r.h
	add d
	ld e, a                     ; r.y + r.h
	ld a, [wNewY]
	cp e
	jr z, .yNear
	jr nc, .apart               ; newY > r.y + r.h
.yNear
	ld a, [wNewY]
	ld e, a
	ld a, [wNewH]
	add e                       ; newY + newH
	cp d
	jr c, .apart                ; newY + newH < r.y
	scf
	ret
.apartSkipH
	inc hl
.apart
	dec b
	jr nz, .room
	and a                       ; no carry
	ret

; Stores the new room, carves it, and joins it to the room before it.
AddRoom:
	ld a, [wRoomCount]
	add a
	add a
	ld e, a
	ld d, 0
	ld hl, wRooms
	add hl, de
	ld a, [wNewX]
	ld [hli], a
	ld a, [wNewY]
	ld [hli], a
	ld a, [wNewW]
	ld [hli], a
	ld a, [wNewH]
	ld [hl], a

	; Rock along the row above the room becomes wall face.
	ld a, [wNewY]
	dec a
	ld c, a
	ld a, [wNewX]
	ld b, a
	call CellAddr
	ld a, [wNewW]
	ld d, a
.wall
	ld a, [hl]
	cp T_ROCK
	jr nz, .notRock
	ld [hl], T_WALL
.notRock
	inc hl
	dec d
	jr nz, .wall

	; Carve the room, a row at a time.
	ld a, [wNewY]
	ld c, a
	ld a, [wNewH]
	ld e, a
.row
	ld a, [wNewX]
	ld b, a
	call CellAddr
	ld a, [wNewW]
	ld d, a
	ld a, T_FLOOR
.cell
	ld [hli], a
	dec d
	jr nz, .cell
	inc c
	dec e
	jr nz, .row

	; Join it to the previous room, center to center.
	ld a, [wRoomCount]
	inc a
	ld [wRoomCount], a
	cp 2
	ret c
	sub 2
	add a
	add a
	ld e, a
	ld d, 0
	ld hl, wRooms
	add hl, de
	call RoomCenter             ; bc = previous center
	push bc
	ld a, [wNewX]
	ld b, a
	ld a, [wNewW]
	srl a
	add b
	ld d, a
	ld a, [wNewY]
	ld b, a
	ld a, [wNewH]
	srl a
	add b
	ld e, a                     ; de = new center
	pop bc
	call Rand
	and 1
	ld [wHorizFirst], a
	jp CarvePath

; Returns the center of the room at HL in B (x) and C (y). Keeps DE.
RoomCenter:
	ld a, [hli]
	ld b, a
	ld a, [hli]
	ld c, a
	ld a, [hli]
	srl a
	add b
	ld b, a
	ld a, [hl]
	srl a
	add c
	ld c, a
	ret

; Carves floor from (B, C) to (D, E): along x then y if wHorizFirst, else
; along y then x.
CarvePath:
	call CarveCell
	ld a, [wHorizFirst]
	and a
	jr z, .vFirst
	call CarveX
	jr CarveY
.vFirst
	call CarveY
	jr CarveX

; Steps B toward D, carving each cell.
CarveX:
	ld a, b
	cp d
	ret z
	jr c, .right
	dec b
	jr .carve
.right
	inc b
.carve
	call CarveCell
	jr CarveX

; Steps C toward E, carving each cell.
CarveY:
	ld a, c
	cp e
	ret z
	jr c, .down
	dec c
	jr .carve
.down
	inc c
.carve
	call CarveCell
	jr CarveY

; Makes cell (B, C) floor, and rock above it wall face. Keeps BC and DE.
CarveCell:
	call CellAddr
	ld [hl], T_FLOOR
	ld a, l
	sub MAP_W
	ld l, a
	jr nc, .noBorrow
	dec h
.noBorrow
	ld a, [hl]
	cp T_ROCK
	ret nz
	ld [hl], T_WALL
	ret

; Fills half the map with rock from HL, 16 cells a loop.
FillHalfMap:
	ld a, T_ROCK
	ld b, MAP_SIZE / 2 / 16
.loop
	REPT 16
	ld [hli], a
	ENDR
	dec b
	jr nz, .loop
	ret

; Generation runs with the LCD on while the game halts each frame, so a step
; of it must never run into VBlank. The longest step (a room and a corridor
; across the whole map) takes about 50 lines, so from line 80 on, wait for
; the next frame before the next step.
DEF YIELD_LY EQU 80
MaybeYield::
	ldh a, [rLY]
	cp YIELD_LY
	ret c
	cp LY_VBLANK
	ret nc
	jp WaitFrame

; Returns the address of map cell (B, C) in HL. Keeps BC and DE.
CellAddr::
	ld a, c
	srl a
	srl a
	add HIGH(wMap)
	ld h, a
	ld a, c
	and 3
	rrca
	rrca
	or b
	ld l, a
	ret

SECTION "Map", WRAMX, ALIGN[8]
wMap:: ds MAP_SIZE

SECTION "Dungeon variables", WRAM0
wRooms:: ds MAX_ROOMS * ROOM_SIZE
wRoomCount:: db
wAttempts: db
wNewX: db
wNewY: db
wNewW: db
wNewH: db
wHorizFirst: db
wItem: db
wBest: db
wLeft: db
wPlayerX:: db
wPlayerY:: db
wStairsX:: db
wStairsY:: db
