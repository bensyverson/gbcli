; Shared by the race's sources: game states, where the tiles live, and the
; player car's place on screen.

IF !DEF(RACE_INC)
DEF RACE_INC EQU 1

include "images.inc"

; wState: scripts can wait on it, e.g. `gbcli run --until '[wState] == 2'`.
DEF STATE_READY EQU 0           ; the countdown before the start
DEF STATE_RACE EQU 1
DEF STATE_WON EQU 2             ; crossed the finish before time ran out
DEF STATE_LOST EQU 3            ; time ran out
DEF STATE_TITLE EQU 4           ; the title screen, before START

; Tiles, all in $8000-$8FFF (the BG and objects both read it unsigned):
; the road, the font, then the object sheets, which start on an even tile
; because 8x16 objects ignore bit 0 of the tile number.
DEF FONT_BASE EQU ROAD_TILES
DEF CAR_BASE EQU (FONT_BASE + FONT_TILES + 1) & ~1
DEF RIVAL_BASE EQU CAR_BASE + CAR_TILES
DEF TREE_BASE EQU RIVAL_BASE + RIVAL_TILES
DEF SPRITE_TILES_END EQU TREE_BASE + TREE_TILES
ASSERT SPRITE_TILES_END <= 256

; The player's car: top-left on screen, 24x16 (three objects, OAM slots
; 0-2), above the HUD.
DEF CAR_OBJECTS EQU 3
DEF CAR_X EQU 68
DEF CAR_Y EQU 118
DEF CAR_CENTER EQU CAR_X + CAR_FRAME_WIDTH / 2
DEF HUD_Y EQU 136

ENDC
