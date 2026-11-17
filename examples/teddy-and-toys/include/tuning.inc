; Teddy & Toys: every gameplay number, in one place.
;
; Each constant has its unit and what it does. Change a number, then run
;     python3 tools/make.py
; from examples/teddy-and-toys/: it regenerates what depends on this file (the
; clock's rotations), builds both ROMs, and runs the rules check.
;
; A frame is 1/60 s here (the Game Boy runs at 59.73 frames a second).
; A speed in "1/256 px a frame" means 256 is one pixel every frame.

IF !DEF(TUNING_INC)
DEF TUNING_INC EQU 1

; Names for the belt directions below (not numbers to tune).
DEF BELT_LEFT  EQU 0                ; things on the belt travel right to left
DEF BELT_RIGHT EQU 1                ; things on the belt travel left to right

; --- The round -------------------------------------------------------------

DEF ROUND_SECONDS      EQU 60       ; seconds: the length of a round
DEF CLOCK_STEPS        EQU 60       ; hand positions over a round (pre-made rotations; must divide ROUND_SECONDS * 60)
DEF CLOCK_RED_SECONDS  EQU 10       ; seconds left when the clock face turns red (GBC only)

; --- The belts -------------------------------------------------------------

DEF BELT_PARTS_SPEED    EQU 96      ; 1/256 px a frame: the front (parts) belt
DEF BELT_PARTS_DIR      EQU BELT_LEFT   ; which way the parts belt runs
DEF BELT_FINISHED_SPEED EQU 96      ; 1/256 px a frame: the back (Finished) belt
DEF BELT_FINISHED_DIR   EQU BELT_RIGHT  ; which way the Finished belt runs

; --- The supply (what the parts belt brings) ---------------------------------
; The belt is cut into 16 px slots; each slot that comes out from under the
; key column holds one piece or nothing.

DEF SUPPLY_CHANCE        EQU 192    ; out of 256 (0-255): the chance a slot brings a piece
DEF SUPPLY_MIN_GAP       EQU 1      ; slots: empty slots forced after each piece (piece spacing)
DEF SUPPLY_MATCH_CHANCE  EQU 96     ; out of 256 (0-255): while Elle holds one half, the chance a new piece is its other half
DEF SUPPLY_MATCH_MAX_GAP EQU 3      ; pieces: after this many new pieces that don't match, the next one does

; --- Hands -----------------------------------------------------------------

DEF PICKUP_REACH  EQU 6             ; px (0-8): how far a piece's middle may be from Elle's middle for A to grab it
DEF CHORD_FRAMES  EQU 5             ; frames (1+): with two halves, the other button on the same frame or up to CHORD_FRAMES - 1 frames later assembles (1 = same frame only)
DEF BUZZ_COOLDOWN EQU 12            ; frames: after a buzz sounds, more refusals stay silent this long

; --- Elle --------------------------------------------------------------------

DEF TURN_FRAMES   EQU 8             ; frames (0+): how long Up or Down takes to turn her; A and B do nothing meanwhile (0 = instant)

ENDC
