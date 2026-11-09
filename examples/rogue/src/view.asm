; The view: a camera over the 64x48 map, drawn into the 32x32 background as
; a ring. A new floor streams in a row a frame; while playing, each camera
; step streams one new row or column. The VBlank handler copies them in.

include "hardware.inc"
include "game.inc"

SECTION "View", ROM0

; Centers the camera on the player, clamped to the map, and sets the scroll.
CameraCenter::
	ld a, [wPlayerX]
	sub VIEW_W / 2
	jr nc, .xPos
	xor a
.xPos
	cp CAM_MAX_X + 1
	jr c, .xOk
	ld a, CAM_MAX_X
.xOk
	ld [wCamX], a
	ld a, [wPlayerY]
	sub VIEW_H / 2
	jr nc, .yPos
	xor a
.yPos
	cp CAM_MAX_Y + 1
	jr c, .yOk
	ld a, CAM_MAX_Y
.yOk
	ld [wCamY], a
	; fall through

; Sets wScx/wScy from the camera: each cell is 8 pixels, the ring wraps at 256.
SetScroll:
	ld a, [wCamX]
	add a
	add a
	add a
	ld [wScx], a
	ld a, [wCamY]
	add a
	add a
	add a
	ld [wScy], a
	ret

; Moves the camera one cell toward the player if the player has left the
; centered window, and queues the newly exposed row or column.
CameraFollow::
	ld a, [wCamX]
	ld b, a
	ld a, [wCamY]
	ld c, a
	call CameraCenter           ; the target camera, one cell away at most
	ld a, [wCamX]
	cp b
	jr z, .checkY
	jr c, .left
	; Moved right: the column at camX + DRAW_W - 1 comes into view.
	add DRAW_W - 1
	cp MAP_W
	ret nc
	jr QueueColumn
.left
	jr QueueColumn              ; the column at camX
.checkY
	ld a, [wCamY]
	cp c
	ret z
	jr c, .up
	add DRAW_H - 1
	cp MAP_H
	ret nc
	jr QueueRow
.up
	jr QueueRow

; Queues map column A (rows from the camera) for the VBlank handler.
QueueColumn:
	ld b, a
	ld a, [wCamY]
	ld c, a
	call CellAddr
	ld de, wStreamBuf
	ld a, MAP_H
	sub c
	cp DRAW_H
	jr c, .countOk
	ld a, DRAW_H
.countOk
	ld [wStreamLen], a
	push bc
	ld b, a
.copy
	ld a, [hl]
	ld [de], a
	inc de
	push bc
	ld bc, MAP_W
	add hl, bc
	pop bc
	dec b
	jr nz, .copy
	pop bc
	call RingAddr
	ld a, l
	ld [wStreamDst], a
	ld a, h
	ld [wStreamDst + 1], a
	ld a, STREAM_COL
	ld [wStreamKind], a
	ret

; Queues map row A (columns from the camera) for the VBlank handler.
QueueRow::
	ld c, a
	call FillRowBuffer
	ld a, STREAM_ROW
	ld [wStreamKind], a
	ret

; Copies map row C, from the camera's column, into wStreamBuf, and sets
; wStreamLen and wStreamDst for it.
FillRowBuffer:
	ld a, [wCamX]
	ld b, a
	call CellAddr
	push bc
	ld a, MAP_W
	sub b
	cp DRAW_W
	jr c, .countOk
	ld a, DRAW_W
.countOk
	ld [wStreamLen], a
	ld b, a
	ld de, wStreamBuf
.copy
	ld a, [hli]
	ld [de], a
	inc de
	dec b
	jr nz, .copy
	pop bc
	call RingAddr
	ld a, l
	ld [wStreamDst], a
	ld a, h
	ld [wStreamDst + 1], a
	ret

; Returns in HL the background address that shows map cell (B, C).
RingAddr::
	ld a, c
	and 31
	ld l, a
	ld h, 0
	add hl, hl
	add hl, hl
	add hl, hl
	add hl, hl
	add hl, hl
	ld a, b
	and 31
	or l
	ld l, a
	ld a, h
	add HIGH(TILEMAP0)
	ld h, a
	ret

; Writes wStreamBuf as a row at wStreamDst, wrapping within the 32-cell row.
WriteRow:
	ld a, [wStreamDst]
	ld l, a
	ld a, [wStreamDst + 1]
	ld h, a
	ld a, [wStreamLen]
	ld b, a
	ld de, wStreamBuf
.cell
	ld a, [de]
	ld [hl], a
	inc de
	ld a, l
	and %1110_0000
	ld c, a
	ld a, l
	inc a
	and %0001_1111
	or c
	ld l, a
	dec b
	jr nz, .cell
	ret

; Writes wStreamBuf as a column at wStreamDst, wrapping within the map.
WriteColumn:
	ld a, [wStreamDst]
	ld l, a
	ld a, [wStreamDst + 1]
	ld h, a
	ld a, [wStreamLen]
	ld b, a
	ld de, wStreamBuf
.cell
	ld a, [de]
	ld [hl], a
	inc de
	ld a, l
	add 32
	ld l, a
	jr nc, .noCarry
	inc h
.noCarry
	ld a, h
	and %11
	or HIGH(TILEMAP0)
	ld h, a
	dec b
	jr nz, .cell
	ret

; Called by the VBlank handler: copies the queued row or column, if any.
FlushStream::
	ld a, [wStreamKind]
	and a
	ret z
	cp STREAM_COL
	push af
	call z, WriteColumn
	pop af
	call nz, WriteRow
	xor a
	ld [wStreamKind], a
	ret

; Queues map cell (B, C) to be redrawn by the VBlank handler (an item taken).
QueueCellPatch::
	push hl
	call CellAddr
	ld a, [hl]
	ld [wPatchTile], a
	call RingAddr
	ld a, l
	ld [wPatchAddr], a
	ld a, h
	ld [wPatchAddr + 1], a
	ld a, 1
	ld [wPatchPending], a
	pop hl
	ret

; Called by the VBlank handler: redraws the queued cell, if any.
FlushPatch::
	ld a, [wPatchPending]
	and a
	ret z
	xor a
	ld [wPatchPending], a
	ld a, [wPatchAddr]
	ld l, a
	ld a, [wPatchAddr + 1]
	ld h, a
	ld a, [wPatchTile]
	ld [hl], a
	ret

; Puts the player's sprite at its cell relative to the camera.
PlacePlayerSprite::
	ld hl, wShadowOAM
	ld a, [wCamY]
	ld b, a
	ld a, [wPlayerY]
	sub b
	add a
	add a
	add a
	add OAM_Y_OFS
	ld [hli], a
	ld a, [wCamX]
	ld b, a
	ld a, [wPlayerX]
	sub b
	add a
	add a
	add a
	add OAM_X_OFS
	ld [hli], a
	ld a, S_PLAYER
	ld [hli], a
	xor a
	ld [hl], a
	ret

SECTION "View variables", WRAM0
wCamX:: db
wCamY:: db
wScx:: db
wScy:: db
wStreamKind:: db
wStreamLen: db
wStreamDst: dw
wStreamBuf: ds DRAW_W
wPatchPending:: db
wPatchAddr: dw
wPatchTile: db
