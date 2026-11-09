; Text: maps each character to its font tile, so `db "TIME"` assembles to
; tile numbers. The order is tools/font.py's GLYPHS; the font's tiles are
; loaded at FONT_BASE.

IF !DEF(CHARMAP_INC)
DEF CHARMAP_INC EQU 1

include "race.inc"

DEF GLYPHS EQUS " 0123456789ABCDEFGHIJKLMNOPQRSTUVWXYZ:.!-"
FOR K, STRLEN("{GLYPHS}")
	CHARMAP STRSLICE("{GLYPHS}", K, K + 1), FONT_BASE + K
ENDR
DEF FONT_DIGIT0 EQU FONT_BASE + 1

ENDC
