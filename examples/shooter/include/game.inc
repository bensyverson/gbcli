; Vesper Swarm: what the game's files share.

IF !DEF(GAME_INC)
DEF GAME_INC EQU 1

; The playfield: everything above the status bar, in screen pixels.
DEF FIELD_W EQU 160
DEF FIELD_H EQU 136

; Game states (wState).
DEF STATE_TITLE EQU 0
DEF STATE_PLAY EQU 1
DEF STATE_WIN EQU 2
DEF STATE_OVER EQU 3
DEF STATE_TEST EQU 4                  ; the sprite test (scene.asm), SELECT on the title

; Sprite tiles (block 0, $8000), drawn by gfx.asm.
DEF T_SHIP EQU $20                    ; 2 tiles, left then right
DEF T_SHOT EQU $22
DEF T_BULLET EQU $23
DEF T_DART EQU $24                    ; 2 tiles
DEF T_WAVE EQU $26                    ; 2 tiles
DEF T_POD EQU $28                     ; 2 tiles
DEF T_BOOM EQU $2A                    ; 2 frames of 1 tile
DEF T_BOSS EQU $30                    ; 9 tiles, 3x3, row by row

; Background tiles: the font (charmap.inc) at T_FONT, then the screen's own
; art from T_SCREEN: the sky's four bands in play, the title image on the
; title screen. Both are loaded with the LCD off when the screen changes.
DEF T_FONT EQU $40
DEF T_SCREEN EQU $70

; GBC palettes. BG: the sky's bands, the title, the text.
DEF PAL_STARS EQU 0
DEF PAL_PEAKS EQU 1
DEF PAL_FOREST EQU 2
DEF PAL_GROUND EQU 3
DEF PAL_TITLE EQU 4
DEF PAL_TEXT EQU 7
; Object attributes: the GBC palette in bits 0-2 (the DMG ignores them),
; and for the hit flash OBP1 on the DMG (white) and the white palette 4.
DEF ATTR_SHIP EQU 0
DEF ATTR_ENEMY EQU 1
DEF ATTR_SHOT EQU 2
DEF ATTR_BOSS EQU 3
DEF ATTR_FLASH EQU OAM_PAL1 | 4

; Enemy kinds (E_KIND); 0 is a free slot.
DEF KIND_DART EQU 1
DEF KIND_WAVE EQU 2
DEF KIND_POD EQU 3
DEF KIND_BOOM EQU 4                   ; an explosion: no hits, just frames

; One enemy, 16 bytes; 8 slots in one aligned block.
DEF ENEMIES EQU 8
RSRESET
DEF E_KIND RB 1
DEF E_X RB 1                          ; screen pixels
DEF E_Y RB 1
DEF E_YF RB 1                         ; fraction of Y
DEF E_HP RB 1
DEF E_T RB 1                          ; frames alive
DEF E_BASE RB 1                       ; WAVE: center line
DEF E_FLASH RB 1                      ; frames left of the hit flash
DEF E_STRIDE EQU 16

; One enemy bullet, 8 bytes: 8.8 X and Y, 8.8 velocity.
DEF BULLETS EQU 31                    ; 8 bytes each: one page, less one
RSRESET
DEF B_XF RB 1
DEF B_X RB 1                          ; 0 = free slot (bullets never sit at 0)
DEF B_YF RB 1
DEF B_Y RB 1
DEF B_DXF RB 1
DEF B_DX RB 1
DEF B_DYF RB 1
DEF B_DY RB 1
DEF B_STRIDE EQU 8

DEF SHOTS EQU 6                       ; player shots: X, Y each, X 0 = free

; Directions: 16, counterclockwise from right (0), 4 is up, 8 left, 12 down.
DEF DIR_RIGHT EQU 0
DEF DIR_UP EQU 4
DEF DIR_LEFT EQU 8
DEF DIR_DOWN EQU 12

ENDC
