; Made by tools/chart.py from assets/music/lantern.song; do not edit.
; The game's clock runs PREROLL ticks before the song starts, so notes
; can fall into view; a note's clock time is PREROLL + its driver tick.

IF !DEF(CHART_INC)
DEF CHART_INC EQU 1
DEF PREROLL EQU 64
DEF CHART_LEN EQU 165
DEF SONG_END_TICK EQU 4480
ENDC
