; Equivalence-check test program, the original build with a longer start-up:
; 3,750 passes of a 28 T loop (105,000 T, about a frame and a half) before the
; LCD comes on, so every vblank lands at another time; see equiv.inc.
; Written for this project; same license as gbcli.
DEF START_DELAY EQU 3750
INCLUDE "equiv_a.asm"
