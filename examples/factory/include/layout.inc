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
DEF BACK_BELT_ROW  EQU 7            ; rows 7-10: stripe, toys (2), stripe
DEF FLOOR_ROW      EQU 11           ; rows 11-13
DEF FRONT_BELT_ROW EQU 14           ; rows 14-17: stripe, pieces (2), stripe
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

; Elle's place on screen (top left of her 16x48 frame, in pixels).
DEF ELLE_X EQU 60
DEF ELLE_Y EQU 52
DEF ELLE_CENTER_X EQU ELLE_X + 8    ; the belts' slot "under Elle" is the one under this x
; Where held things show (top left of each 16x16).
DEF HAND_LEFT_X  EQU ELLE_X - 13
DEF HAND_RIGHT_X EQU ELLE_X + 13
DEF HAND_Y       EQU ELLE_Y + 14
DEF TOY_HELD_X   EQU ELLE_X
DEF TOY_HELD_Y   EQU ELLE_Y + 16

; The results screen: its text lines, a 20-tile buffer each.
DEF T_RESULTS EQU 128               ; 5 lines x 20 tiles (reloaded by ScreenInit after)

; Tile numbers ($8000 addressing: BG, window and objects share them).
DEF T_WALL        EQU 0
DEF T_STRIPE      EQU 1             ; a belt's edge: diagonal stripes show it moving
DEF T_BELT        EQU 2             ; a belt's surface, where nothing rides
DEF T_FLOOR       EQU 3
DEF T_KEY_BG      EQU 4
DEF T_KEY_DIVIDER EQU 5
DEF T_BOX         EQU 6             ; 9 tiles: the score box's frame and inside
DEF T_CLOCK_A     EQU 16            ; 16 tiles: clock buffer A
DEF T_CLOCK_B     EQU 32            ; 16 tiles: clock buffer B
DEF T_DIGITS      EQU 48            ; 10 tiles: 0-9
DEF T_TOYS        EQU 64            ; TOY_COUNT * TOY_TILES tiles (even, for 8x16 pairs)
DEF T_ELLE        EQU 144           ; Elle's object tiles

ENDC
