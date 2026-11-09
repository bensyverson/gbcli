; The sprite test, from SELECT on the title screen: 48 bullets, more than
; OAM's 40, each with its own tile number (SCENE_TILE0 + its index) so
; `gbcli view oam` can tell them apart (tools/oam_coverage.py). Bullets
; 0-15 ride one line, back and forth, so that line carries 16 sprites
; against the hardware's 10; the other 32 bounce around the playfield.

include "hardware.inc"
include "game.inc"
include "images.inc"

DEF SCENE_OBJS EQU 48
DEF SCENE_ROW EQU 16                  ; bullets on the crowded line
DEF SCENE_TILE0 EQU $B0               ; free while the sky is loaded
DEF ROW_Y EQU 64 + OAM_Y_OFS
DEF MIN_X EQU OAM_X_OFS
DEF MAX_X EQU 152 + OAM_X_OFS
DEF MIN_Y EQU OAM_Y_OFS
DEF MAX_Y EQU 128 + OAM_Y_OFS

SECTION "Scene objects", WRAM0, ALIGN[8]

wObjs: ds SCENE_OBJS * 4              ; X, Y, DX, DY; one page, so `inc l` walks it

SECTION "Scene", ROM0

; With the LCD off.
SceneInit::
	ld hl, TILEBLOCK0 + SCENE_TILE0 * 16
	ld b, SCENE_OBJS
.tiles
	ld de, BulletTiles
	push bc
	ld bc, 16
	call CopyMem
	pop bc
	dec b
	jr nz, .tiles

	ld hl, wObjs
	ld c, 0                           ; object index
.init
	ld a, c
	cp SCENE_ROW
	jr nc, .free
	; On the crowded line: 9 px apart, all moving the same way.
	add a
	add a
	add a
	add c
	add MIN_X
	ld [hli], a                       ; X
	ld a, ROW_Y
	ld [hli], a                       ; Y
	ld a, 1
	ld [hli], a                       ; DX
	xor a
	ld [hli], a                       ; DY
	jr .next
.free
	; Spread over the playfield by two strides: X by 41, Y by 23.
	ld b, a
	add a
	add a
	add a
	ld e, a                           ; 8c
	add a
	add a                             ; 32c
	add b
	add e                             ; 41c
	and %01111111
	add MIN_X + 8
	ld [hli], a                       ; X
	ld a, e
	add a
	add a                             ; 32c
	sub e
	sub b                             ; 23c
	and %01111111
	add MIN_Y
	ld [hli], a                       ; Y
	ld a, c
	and %00000010
	dec a                             ; -1 or 1
	ld [hli], a                       ; DX
	ld a, c
	rra
	and %00000010
	dec a
	ld [hli], a                       ; DY
.next
	inc c
	ld a, c
	cp SCENE_OBJS
	jr nz, .init
	ret

; Moves every bullet one step, bouncing off the playfield's edges, and
; writes each straight into the frame's sprite list (SprPut's format).
SceneUpdate::
	ld a, [wSprCount]
	ld b, a
	add SCENE_OBJS
	ld [wSprCount], a
	ld a, b
	add a
	add a
	ld e, a
	ld d, HIGH(wSprList)              ; DE: the next list entry
	ld hl, wObjs
	ld c, SCENE_TILE0                 ; C: this bullet's tile
.loop
	; X += DX, bouncing.
	inc l
	inc l
	ld b, [hl]                        ; DX
	dec l
	dec l
	ld a, [hl]
	add b
	cp MIN_X
	jr c, .bounceX
	cp MAX_X + 1
	jr c, .xOk
.bounceX
	ld a, b
	cpl
	inc a
	inc l
	inc l
	ld [hl], a                        ; DX = -DX
	dec l
	dec l
	add [hl]
.xOk
	ld [hli], a
	inc e
	ld [de], a                        ; the entry's X
	dec e
	; Y += DY, bouncing.
	inc l
	inc l
	ld b, [hl]                        ; DY
	dec l
	dec l
	ld a, [hl]
	add b
	cp MIN_Y
	jr c, .bounceY
	cp MAX_Y + 1
	jr c, .yOk
.bounceY
	ld a, b
	cpl
	inc a
	inc l
	inc l
	ld [hl], a
	dec l
	dec l
	add [hl]
.yOk
	ld [hli], a
	ld [de], a                        ; the entry's Y
	inc e
	inc e
	ld a, c
	ld [de], a                        ; tile
	inc e
	ld a, ATTR_SHOT
	ld [de], a                        ; attributes
	inc e
	inc l
	inc l                             ; the next bullet
	inc c
	ld a, c
	cp SCENE_TILE0 + SCENE_OBJS
	jr nz, .loop
	ret
