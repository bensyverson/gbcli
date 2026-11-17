; The screen's layout and tile memory: where things are, not how the game
; plays (that is tuning.inc). Changing these means changing the art.
;
;   lines   0- 55  HUD: the wall, the clock and the score box (SCX 0)
;   lines  56- 87  the Finished belt, at Elle's waist (its own SCX)
;   lines  88-111  the floor Elle stands on (SCX 0)
;   lines 112-143  the parts belt, in front (its own SCX)
;   x 136-159      the key column: the window, from line 0 down, so it
;                  never scrolls and covers the belts' right ends
;
; The window covers everything right of WX and below WY, so it can only
; be a column on the right from the top: the HUD sits left of it.

IF !DEF(LAYOUT_INC)
DEF LAYOUT_INC EQU 1

; Bands, in tile rows of 8 lines.
DEF BACK_BELT_ROW  EQU 7            ; rows 7-10: top edge, toys (2), bottom edge
DEF FLOOR_ROW      EQU 11           ; rows 11-13
DEF FRONT_BELT_ROW EQU 14           ; rows 14-17: top edge, pieces (2), bottom edge
DEF BELT_ITEM_ROW_OFS EQU 1         ; a belt's toys sit one row below its top stripe

; The scroll splits: LYC is the line before a band, and its handler writes
; SCX in that line's HBlank, so the band's first line draws with it.
DEF SPLIT_BACK_LYC  EQU BACK_BELT_ROW * 8 - 1    ; 55
DEF SPLIT_FLOOR_LYC EQU FLOOR_ROW * 8 - 1        ; 87
DEF SPLIT_FRONT_LYC EQU FRONT_BELT_ROW * 8 - 1   ; 111

; The key column (window): 3 tiles wide; a divider, then 2 tiles of icon.
DEF KEY_X     EQU 136
DEF KEY_WX    EQU KEY_X + 7
DEF KEY_ROWS_PER_TOY EQU 3          ; icon (2 rows) and a gap

; HUD positions, in tiles.
DEF CLOCK_ROW EQU 1
DEF CLOCK_COL EQU 1
DEF SCORE_ROW EQU 1                 ; the box is 3 tiles tall
DEF SCORE_COL EQU 6                 ; and 5 wide: frame, 3 digits, frame

; Elle's place on screen (top left of her 16x48 frame, in pixels). She
; steps toward the belt she faces: up toward the Finished belt, down toward
; the parts belt, and stands between the two mid-turn.
DEF ELLE_X EQU 60
DEF ELLE_Y_PARTS    EQU 58
DEF ELLE_Y_FINISHED EQU ELLE_Y_PARTS - 8
DEF ELLE_Y_SIDE     EQU (ELLE_Y_PARTS + ELLE_Y_FINISHED) / 2
DEF ELLE_CENTER_X EQU ELLE_X + 8    ; the belts' slot "under Elle" is the one under this x
; Where held things show (top left of each 16x16), relative to Elle.
DEF HAND_LEFT_X  EQU ELLE_X - 13
DEF HAND_RIGHT_X EQU ELLE_X + 13
DEF HAND_DY      EQU 14             ; below Elle's top
DEF TOY_HELD_X   EQU ELLE_X
DEF TOY_HELD_DY  EQU 16

; Color numbers. Pieces are background tiles on the belts and objects in
; Elle's hands, from one tile set, so color 0 is the belts' dark gray on
; the background and clear on objects (tools/tiles.py says the same).
DEF COLOR_DARK  EQU 0
DEF COLOR_WHITE EQU 1
DEF COLOR_LIGHT EQU 2
DEF COLOR_INK   EQU 3
DEF BGP_GAME    EQU %11_01_00_10    ; color 0 dark gray, 1 white, 2 light gray, 3 black
DEF OBP_GAME    EQU %11_01_00_00    ; color 1 white, 2 light gray, 3 black

; GBC palettes (src/colors.asm). BG:
DEF PAL_SCENE EQU 0                 ; the belts, the floor, the key column, the pieces on them
DEF PAL_CLOCK EQU 1                 ; the clock face (src/clock.asm swaps it red)
DEF PAL_WALL  EQU 2                 ; the HUD band: the wall and the score box
; OBJ: Elle's parts, then the pieces she holds.
DEF OBJ_PAL_HEAD   EQU 0
DEF OBJ_PAL_TOP    EQU 1            ; the denim top from the front
DEF OBJ_PAL_HAIR   EQU 2            ; the hair down her back
DEF OBJ_PAL_LEGS   EQU 3
DEF OBJ_PAL_PIECES EQU 4

; Tile numbers ($8000 addressing: BG, window and objects share them).
; The scene's (T_BLANK = 0 ... T_BOX, SCENE_TILES of them) come from
; art/scene.pixels by tools/gen_scene.py.
include "gen/scene.inc"
DEF T_FLOOR       EQU T_BLANK       ; the floor is white
DEF T_CLOCK_A     EQU 32            ; 16 tiles: clock buffer A (256-byte aligned: src/clock.asm sets only the high byte)
DEF T_CLOCK_B     EQU 48            ; 16 tiles: clock buffer B (aligned too)
DEF T_DIGITS      EQU 64            ; 10 tiles: 0-9
DEF T_TOYS        EQU 74            ; TOY_COUNT * TOY_TILES tiles (even, for 8x16 pairs)
DEF T_ELLE        EQU 146           ; Elle's object tiles (even)
	ASSERT SCENE_TILES <= T_CLOCK_A && T_CLOCK_A % 16 == 0 && T_CLOCK_B % 16 == 0

; The title and results screens keep tile 0 (white) and the toys, and
; reuse the rest (ScreenInit reloads them for a round): two small lines of
; text, a 20-tile buffer each; a scratch line that the big lines are set
; in; and the big lines at twice the size, 4 tiles per font tile
; (src/bigtext.asm).
DEF RESULTS_LINE_TILES EQU 20
DEF T_RESULTS EQU T_BLANK + 20      ; 2 lines x 20 tiles, to T_TOYS
DEF T_SCRATCH EQU T_ELLE            ; 20 tiles
DEF T_BIG     EQU T_SCRATCH + 20    ; to 255
	ASSERT T_RESULTS + 2 * RESULTS_LINE_TILES <= T_TOYS

ENDC
