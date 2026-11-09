; Lanternfall: constants shared by every source file.

IF !DEF(GAME_INC)
DEF GAME_INC EQU 1

; The dungeon map: one byte a cell, row-major, 64 cells a row so a cell's
; address is wMap + y * 64 + x with shifts. Bigger than the 32x32 background,
; so the view streams rows and columns into the background as it scrolls.
DEF MAP_W EQU 64
DEF MAP_H EQU 48
DEF MAP_SIZE EQU MAP_W * MAP_H

; The screen shows 20x18 cells; one extra row and column are kept drawn so a
; scroll never shows a stale cell.
DEF VIEW_W EQU 20
DEF VIEW_H EQU 18                   ; the bottom row is under the status line
DEF DRAW_W EQU VIEW_W + 1
DEF DRAW_H EQU VIEW_H + 1
DEF CAM_MAX_X EQU MAP_W - VIEW_W
DEF CAM_MAX_Y EQU MAP_H - VIEW_H

; Cell values. Each is also the background tile drawn for it.
DEF T_ROCK   EQU 0
DEF T_FLOOR  EQU 1
DEF T_WALL   EQU 2                  ; rock with floor below it: a wall face
DEF T_STAIRS EQU 3
DEF T_POTION EQU 4                  ; items lie on the floor as cells
DEF T_BLADE  EQU 5
DEF T_EMBER  EQU 6                  ; the goal, on the last floor
DEF T_FIRST_ITEM EQU T_POTION

; Sprite tiles follow the background's in block 0; the font follows them.
DEF S_PLAYER EQU 32
DEF S_RAT    EQU 33
DEF S_GHOUL  EQU 34
DEF FONT_BASE EQU 64

; The descent: floors 0 to LAST_DEPTH; the last holds the Ember, not stairs.
DEF LAST_DEPTH EQU 4
DEF BLADE_DEPTH EQU 1

; The player.
DEF PLAYER_HP   EQU 20
DEF PLAYER_ATK  EQU 2
DEF BLADE_ATK   EQU 4
DEF POTION_HEAL EQU 8

; Enemies: up to MAX_ENEMIES a floor, 2 + depth of them, each in OAM after
; the player. A kind's numbers are in EnemyKinds (enemies.asm).
DEF MAX_ENEMIES EQU 8
DEF KIND_RAT    EQU 0
DEF KIND_GHOUL  EQU 1
DEF CHASE_RANGE EQU 9               ; x + y distance at which an enemy notices

; Game states.
DEF STATE_PLAY EQU 0
DEF STATE_WON  EQU 1
DEF STATE_DEAD EQU 2

; The window: a status row at the bottom of the screen.
DEF STATUS_WY EQU 136
DEF LCDC_TITLE EQU LCDC_ON | LCDC_BG_ON | LCDC_OBJ_OFF | LCDC_BLOCK01 | LCDC_BG_9800 | LCDC_WIN_OFF
DEF LCDC_PLAY EQU LCDC_ON | LCDC_BG_ON | LCDC_OBJ_ON | LCDC_OBJ_8 | LCDC_BLOCK01 | LCDC_BG_9800 | LCDC_WIN_ON | LCDC_WIN_9C00

; Room generation.
DEF MAX_ROOMS    EQU 10
DEF MIN_ROOMS    EQU 4
DEF MAX_ATTEMPTS EQU 80
DEF ROOM_MIN_W   EQU 4
DEF ROOM_W_RANGE EQU 8              ; widths 4-11
DEF ROOM_MIN_H   EQU 3
DEF ROOM_H_RANGE EQU 5              ; heights 3-7
DEF ROOM_SIZE    EQU 4              ; x, y, w, h

; Streaming one row or column into the background during VBlank.
DEF STREAM_NONE EQU 0
DEF STREAM_COL  EQU 1
DEF STREAM_ROW  EQU 2

; Palettes: shades of indices 3-0, darkest first.
DEF PLAY_PALETTE EQU %11_10_01_00
DEF DARK_PALETTE EQU %11_11_11_11

; Sound effects (sfx.asm), for SfxPlay.
DEF SFX_STEP   EQU 0
DEF SFX_HIT    EQU 1
DEF SFX_HURT   EQU 2
DEF SFX_PICKUP EQU 3
DEF SFX_STAIRS EQU 4

; Held-direction repeat, in frames.
DEF REPEAT_FIRST EQU 12
DEF REPEAT_NEXT  EQU 6

ENDC
