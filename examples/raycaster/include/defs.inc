; Shared constants. tools/raymath.py holds the same numbers for the host.

IF !DEF(DEFS_INC)
DEF DEFS_INC EQU 1

DEF RAYS        EQU 64          ; one ray per 2-pixel column
DEF VIEW_H      EQU 64          ; view height in pixels
DEF HALF_H      EQU VIEW_H / 2
DEF VIEW_TW     EQU RAYS / 4    ; view width in tiles (16)
DEF VIEW_TH     EQU VIEW_H / 8  ; view height in tiles (8)
DEF COL_BYTES   EQU VIEW_H * 2  ; one tile column of the buffer (128)
DEF BUF_SIZE    EQU VIEW_TW * COL_BYTES ; 2048
DEF BUF_CHUNKS  EQU BUF_SIZE / 16

; Where the view sits on the 32x32 background map, in tiles.
DEF VIEW_MAP_X  EQU 2
DEF VIEW_MAP_Y  EQU 2

; The two VRAM tile sets the view alternates between. Set A is tiles $00-$7F
; at $8000 (LCDC bit 4 set); set B is tiles $00-$7F at $9000 (bit 4 clear).
; The same background map indexes both, so a flip is one LCDC write.
DEF SET_A       EQU $8000
DEF SET_B       EQU $9000
; Tiles $80-$FF ($8800) are seen by both sets and by sprites: the HUD.
DEF TILE_BLANK  EQU $80
DEF TILE_SOLID  EQU $81
DEF TILE_SHADE1 EQU $82
DEF TILE_SHADE2 EQU $83
; Tiles $90-$9F: a whole tile inside the walls of a tile column's four rays,
; one per mix of x sides (shade 3) and y sides (shade 2).
DEF TILE_WALL0  EQU $90
; Tiles $E0-$F1: the key sheet's 8x16 objects (assets/sprites/key.24x16).
DEF KEY_TILE_BASE EQU $E0

; The keys as objects (src/sprite.asm; tools/raymath.py's key_objects holds
; the same numbers).
DEF KEY_COUNT   EQU 3           ; the keys' list length: the maze's MAZE_KEYS
DEF KEY_SLOTS   EQU 3           ; OAM entries per key: the widest frame's
DEF DEPTH_MIN   EQU 64          ; a key nearer than 1/4 cell is not drawn
DEF FRAME0_HH   EQU 21          ; the half height from which frame 0 is used
DEF FRAME1_HH   EQU 14          ; frame 1
DEF FRAME2_HH   EQU 8           ; frame 2; below it, frame 3
DEF KEY_Y       EQU 56          ; OAM y of the frame's top at the horizon, before lift and bob
DEF KEY_X       EQU 76          ; OAM x of the frame's left for a key dead ahead

; Each view buffer's meta page: the view's map, then the copy list.
DEF LIST_START  EQU 128
DEF MAP_ROWS_CHUNKS EQU VIEW_TH ; the copy writes the view's 8 map rows too

; Four tiles of shades 0-3 on the map, for tools/reference.py's SWATCH.
DEF SWATCH_MAP_X EQU 16
DEF SWATCH_MAP_Y EQU 16

; The VBlank copy (after OAM DMA) starts a 16-byte chunk only while LY is
; below this: a chunk takes just over a line (at most 117 M-cycles to a line's
; 114), and the flip follows the last at once, so every write ends inside
; VBlank (lines 144-153). gbcli checks it: see the README.
DEF COPY_LAST_LY EQU 152
; A copy that ends while LY is below this shows its view at once: the flip
; and a second OAM DMA (ShowView to ShowView.oamDone, under 1.6 lines) still
; end inside VBlank. Later, the next VBlank shows it first thing.
DEF FLIP_LAST_LY EQU 152

DEF LOCK_PERIOD EQU 32          ; frames per view in the frame-locked bench mode

DEF MAP_W       EQU 16

; Map cells.
DEF CELL_STONE  EQU 1
DEF CELL_KEY    EQU 2
DEF CELL_DOOR   EQU 3

; The game.
DEF MODE_TITLE  EQU 0
DEF MODE_MAZE   EQU 1
DEF STATE_PLAY  EQU 0
DEF STATE_WON   EQU 1
DEF STATE_LOST  EQU 2
DEF OIL_PIXELS  EQU 128         ; the oil bar's length, 16 tiles
DEF OIL_SUB     EQU 75          ; frames per pixel: a full lamp lasts 9,600 frames
DEF KEY_OIL     EQU 32          ; pixels of oil a key refills (a quarter lamp, 2,400 frames)

RSRESET
DEF SFX_KEY     RB 1
DEF SFX_LOCKED  RB 1
DEF SFX_WIN     RB 1
DEF SFX_LOSE    RB 1
DEF SFX_START   RB 1

RSRESET
DEF MSG_FIND    RB 1
DEF MSG_KEY     RB 1
DEF MSG_LOCKED  RB 1
DEF MSG_DOOR    RB 1
DEF MSG_WON     RB 1
DEF MSG_LOST    RB 1

DEF B_HUD_LABELS EQU 0
DEF B_HUD_KEYS  EQU 1
DEF B_HUD_OIL   EQU 2
DEF B_HUD_MSG   EQU 3
DEF HUD_LABELS  EQU 1 << B_HUD_LABELS
DEF HUD_KEYS    EQU 1 << B_HUD_KEYS
DEF HUD_OIL     EQU 1 << B_HUD_OIL
DEF HUD_MSG     EQU 1 << B_HUD_MSG
DEF HUD_ALL     EQU HUD_LABELS | HUD_KEYS | HUD_OIL | HUD_MSG

; HUD places, as offsets into a background map.
DEF HUD_KEYS_LABEL EQU 11 * 32 + 2
DEF HUD_KEYS_DIGIT EQU HUD_KEYS_LABEL + 5
DEF HUD_OIL_LABEL  EQU 11 * 32 + 15
DEF HUD_OIL_BAR    EQU 12 * 32 + 2
DEF HUD_MSG_ROW    EQU 14 * 32 + 2
DEF ANG_MASK_HI EQU 3           ; angles are 10 bits: 0-1023

DEF TURN_SPEED  EQU 6           ; angle units a frame
DEF MARGIN      EQU 64          ; how close the player may come to a wall (1/4 cell)

; Shades (BGP %11_10_01_00).
DEF SHADE_CEIL  EQU 0
DEF SHADE_FLOOR EQU 1
DEF SHADE_WALLY EQU 2
DEF SHADE_WALLX EQU 3

ENDC
