; The game: find the keys, reach the door before the lamp runs dry.
;
; Keys are glowing blocks in the maze: walk into one to take it. The exit
; door opens to a walk into it once every key is held, and that wins. The
; lamp's oil burns a little every frame; when it runs out, the game is lost.
; START after either ending begins a new game.

include "hardware.inc"
include "defs.inc"
include "charmap.inc"

SECTION "Game", ROM0

; Copies the font into the shared tiles. Call with the LCD off.
LoadFont::
	ld hl, TILEBLOCK1 + FONT_BASE * 16 - $800
	ld de, FontTiles
	ld bc, FontTilesEnd - FontTiles
	jp CopyMem

; Starts a game: the maze, the player at the start, no keys, a full lamp,
; and the whole HUD to draw.
NewGame::
	ld hl, wMap
	ld de, Maze
	ld bc, MAP_W * MAP_W
	call CopyMem
	ld hl, wPosX
	ld a, $80
	ld [hli], a
	ld a, MAZE_START_X
	ld [hli], a
	ld a, $80
	ld [hli], a
	ld a, MAZE_START_Y
	ld [hli], a
	xor a                       ; facing east
	ld [hli], a
	ld [hl], a
	ld [wKeys], a
	ld [wOilSub], a
	ld [wState], a              ; STATE_PLAY
	ld [wGoTitle], a
	ld a, OIL_PIXELS
	ld [wOilPx], a
	ld a, MSG_FIND
	ld [wMsg], a
	ld a, HUD_ALL
	ld [wHudFlags], a
	ret

; One frame of the game, from the VBlank handler after the joypad is read.
GameTick::
	ld a, [wState]
	and a
	jr z, .play
	; An ending: START goes back to the title (the main loop takes it there).
	ld a, [wPressed]
	and PAD_START
	ret z
	ld [wGoTitle], a
	ld a, SFX_START
	jp SfxPlay
.play
	call MovePlayer
	ld a, [wBumpCell]
	cp CELL_KEY
	jr z, .key
	cp CELL_DOOR
	jr z, .door
.burn
	; The lamp: one pixel of the oil bar every OIL_SUB frames.
	ld hl, wOilSub
	inc [hl]
	ld a, [hl]
	cp OIL_SUB
	ret c
	ld [hl], 0
	ld hl, wOilPx
	dec [hl]
	ld a, HUD_OIL
	call MarkHud
	ld a, [wOilPx]
	and a
	ret nz
	ld a, STATE_LOST
	ld [wState], a
	ld a, SFX_LOSE
	call SfxPlay
	ld a, MSG_LOST
	jr SetMessage
.key
	; Take the key: the block becomes floor.
	ld a, [wBumpIdx]
	ld l, a
	ld h, HIGH(wMap)
	ld [hl], 0
	ld hl, wKeys
	inc [hl]
	ld a, HUD_KEYS
	call MarkHud
	ld a, SFX_KEY
	call SfxPlay
	ld a, [wKeys]
	cp MAZE_KEYS
	ld a, MSG_KEY
	jr c, .keyMsg
	ld a, MSG_DOOR
.keyMsg
	call SetMessage
	jr .burn
.door
	ld a, [wKeys]
	cp MAZE_KEYS
	jr nc, .escape
	ld a, [wMsg]
	cp MSG_LOCKED
	ld a, SFX_LOCKED
	call nz, SfxPlay            ; once, when the door first refuses
	ld a, MSG_LOCKED
	call SetMessage
	jr .burn
.escape
	ld a, STATE_WON
	ld [wState], a
	ld a, SFX_WIN
	call SfxPlay
	ld a, MSG_WON
	; fall through

; Shows message A on the HUD (unless it is already showing).
SetMessage:
	ld hl, wMsg
	cp [hl]
	ret z
	ld [hl], a
	ld a, HUD_MSG
	; fall through

; Marks HUD parts A (HUD_* bits) to redraw in the next VBlank.
MarkHud:
	ld hl, wHudFlags
	or [hl]
	ld [hl], a
	ret

; Redraws one HUD part marked in wHudFlags (labels, keys, oil, then the
; message), in both background maps. Runs in VBlank, before the view copy,
; which then takes what VBlank is left; one part a VBlank keeps it short.
UpdateHud::
	ld hl, wHudFlags
	ld a, [hl]
	and a
	ret z
	bit B_HUD_LABELS, a
	jr z, .noLabels
	res B_HUD_LABELS, [hl]
	ld de, LabelKeys
	ld hl, HUD_KEYS_LABEL
	ld b, LabelKeys.end - LabelKeys
	call HudPut
	ld de, LabelOil
	ld hl, HUD_OIL_LABEL
	ld b, LabelOil.end - LabelOil
	jp HudPut
.noLabels
	bit B_HUD_KEYS, a
	jr z, .noKeys
	res B_HUD_KEYS, [hl]
	ld a, [wKeys]
	add '0'
	ld hl, HUD_KEYS_DIGIT
	jp HudPutA
.noKeys
	bit B_HUD_OIL, a
	jr z, .noOil
	res B_HUD_OIL, [hl]
	jr DrawOil
.noOil
	res B_HUD_MSG, [hl]
	; The message's two lines, 16 tiles each.
	ld a, [wMsg]
	add a
	add a
	add a
	add a
	add a                       ; 32 bytes a message
	ld e, a
	ld d, 0
	ld hl, Messages
	add hl, de
	ld d, h
	ld e, l
	ld hl, HUD_MSG_ROW
	ld b, 16
	call HudPut
	ld hl, HUD_MSG_ROW + 32
	ld b, 16
	jp HudPut

; The oil bar: OIL_PIXELS / 8 tiles, each full, partly full or empty, built
; in wOilTiles and then written as one row.
DrawOil:
	ld a, [wOilPx]
	ld c, a
	ld hl, wOilTiles
	ld b, OIL_PIXELS / 8
.tile
	ld a, c
	cp 8
	jr c, .partial
	sub 8
	ld c, a
	ld a, FONT_BAR0 + 8
	jr .put
.partial
	add FONT_BAR0
	ld c, 0
.put
	ld [hli], a
	dec b
	jr nz, .tile
	ld de, wOilTiles
	ld hl, HUD_OIL_BAR
	ld b, OIL_PIXELS / 8
	jp HudPut

; Writes tile A at HL (an offset into a background map) in both maps.
HudPutA:
	ld de, TILEMAP0
	add hl, de
	ld [hl], a
	ld de, TILEMAP1 - TILEMAP0
	add hl, de
	ld [hl], a
	ret

; Writes B tiles from DE at HL (an offset into a background map), in both.
HudPut:
	push hl
	push de
	push bc
	ld a, h
	add HIGH(TILEMAP0)
	ld h, a
	call .row
	pop bc
	pop de
	pop hl
	ld a, h
	add HIGH(TILEMAP1)
	ld h, a
.row
	ld a, [de]
	inc de
	ld [hli], a
	dec b
	jr nz, .row
	ret

LabelKeys:
	db "KEYS  /"
	db '0' + MAZE_KEYS
.end
LabelOil:
	db "OIL"
.end

; Two lines of 16 tiles per message, in MSG_* order.
Messages:
	db "  FIND THE KEYS ", "                "
	db " YOU FOUND A KEY", "                "
	db " DOOR IS LOCKED ", "                "
	db "  FIND THE DOOR ", "                "
	db "  YOU ESCAPED!  ", "   PRESS START  "
	db " YOUR LAMP DIED ", "   PRESS START  "

SECTION "Game variables", WRAM0
wState:: db                     ; STATE_PLAY, STATE_WON or STATE_LOST
wKeys:: db                      ; keys held
wOilPx:: db                     ; oil left, in pixels of the bar
wOilSub:: db                    ; frames burned toward the next pixel
wMsg:: db                       ; the message on the HUD (MSG_*)
wHudFlags:: db                  ; HUD parts to redraw (HUD_*)
wPadPrev:: db
wPressed:: db                   ; buttons pressed this frame
wOilTiles: ds OIL_PIXELS / 8
