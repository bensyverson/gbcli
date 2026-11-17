; The shift: Elle's hands and the controls table, one rule a routine.
;
;   Facing        Holding          Input   Result
;   parts belt    a free hand      A       picks up the piece in front (if it matches)  ActionA
;   parts belt    one half         B       it goes back on the belt where she stands    ActionB
;   parts belt    two halves       B       the last half grabbed goes back              ActionB
;   either        two halves       B+A     assembles them into the finished toy         Assemble
;   Finished belt an assembled toy B       puts it on the Finished belt; score +1       ActionB
;   anything else                  A or B  nothing, with a buzz                         Buzz
;
; With two halves, A and B wait CHORD_FRAMES (tuning.inc) for each other:
; the other button pressed in that window, or both on one frame, makes the
; chord; otherwise the press acts alone when its window ends. With fewer
; halves (or a toy) a press acts at once.
;
; The counters (wGrabs ... wBuzzes) only count; tools/rules_check.py reads
; them to see which rule fired.

include "hardware.inc"
include "layout.inc"
include "tuning.inc"
include "gen/toys.inc"
include "bank.inc"

DEF FACING_PARTS    EQU 0           ; toward the camera: we see Elle's face
DEF FACING_FINISHED EQU 1           ; away: we see her back
EXPORT FACING_PARTS, FACING_FINISHED
	ASSERT CHORD_FRAMES >= 1, "CHORD_FRAMES must be 1 or more"
	ASSERT PICKUP_REACH <= 8, "PICKUP_REACH is at most 8: half a slot"

DEF ELLE_FRAME_BACK  EQU 0          ; frames of assets/sprites/elle.16x48
DEF ELLE_FRAME_FRONT EQU 1
DEF HELD_OBJECTS EQU 4              ; OAM 0-3: what Elle holds, over her
DEF ELLE_OAM EQU HELD_OBJECTS * 4

SECTION "Play", ROM0

; A new round's hands: empty, facing the parts belt, score 0.
PlayInit::
	xor a
	ld hl, wPlayVars
	ld b, wPlayVarsEnd - wPlayVars
.clear
	ld [hli], a
	dec b
	jr nz, .clear
	ld a, FACING_PARTS
	ld [wFacing], a
	ld a, 1
	ld [wScoreDirty], a
	ret

; Once a frame in a round, after BeltsUpdate.
PlayUpdate::
	ld a, [wBuzzTimer]
	and a
	jr z, .facing
	dec a
	ld [wBuzzTimer], a
.facing
	ldh a, [hJoyPressed]
	bit B_PAD_DOWN, a
	jr z, .notDown
	ld a, FACING_PARTS
	ld [wFacing], a
.notDown
	ldh a, [hJoyPressed]
	bit B_PAD_UP, a
	jr z, .buttons
	ld a, FACING_FINISHED
	ld [wFacing], a
.buttons
	ld a, [wHeldCount]
	cp 2
	jr z, .twoHalves
	; Fewer than two halves: each press acts now (A first).
	ldh a, [hJoyPressed]
	bit B_PAD_A, a
	call nz, ActionA
	ldh a, [hJoyPressed]
	bit B_PAD_B, a
	call nz, ActionB
	jp ElleDraw
.twoHalves
	ldh a, [hJoyPressed]
	and PAD_A | PAD_B
	cp PAD_A | PAD_B
	jr z, .chord                ; both on one frame
	bit B_PAD_B, a
	jr z, .notB
	ld a, [wChordA]
	and a
	jr nz, .chord               ; A was waiting: B inside its window
	ld a, CHORD_FRAMES
	ld [wChordB], a
	jr .windows
.notB
	bit B_PAD_A, a
	jr z, .windows
	ld a, [wChordB]
	and a
	jr nz, .chord               ; B was waiting: A inside its window
	ld a, CHORD_FRAMES
	ld [wChordA], a
	jr .windows
.chord
	call Assemble
	jp ElleDraw
	; A press whose window runs out acts alone.
.windows
	ld hl, wChordB
	ld a, [hl]
	and a
	jr z, .windowA
	dec [hl]
	call z, ActionB
.windowA
	ld hl, wChordA
	ld a, [hl]
	and a
	jr z, .done
	dec [hl]
	call z, ActionA
.done
	jp ElleDraw

; A alone: picks up the piece in front, if a hand is free and it fits.
ActionA:
	ld a, [wFacing]
	cp FACING_PARTS
	jp nz, Buzz
	ld a, [wToy]
	and a
	jp nz, Buzz                 ; hands full with a toy
	ld a, [wHeldCount]
	cp 2
	jp nc, Buzz                 ; no free hand
	ld a, [wPartsInReach]
	and a
	jp z, Buzz                  ; nothing close enough in front
	call PartsSlotUnder         ; hl: the slot, a: its piece
	and a
	jp z, Buzz                  ; an empty stretch of belt
	ld b, a
	ld a, [wHeldCount]
	and a
	jr z, .take
	ld a, [wWantedPiece]
	cp b
	jp nz, Buzz                 ; doesn't match the half she holds
.take
	ld [hl], 0
	ld a, [wPartsUnder]
	call QueueSlot
	ld a, [wHeldCount]
	ld e, a
	ld d, 0
	ld hl, wHands
	add hl, de
	ld [hl], b
	inc a
	ld [wHeldCount], a
	ld hl, wGrabs
	inc [hl]
	jp SetWanted

; B alone: drops a toy on the Finished belt, or gives back the last half.
ActionB:
	ld a, [wToy]
	and a
	jr z, .half
	ld a, [wFacing]
	cp FACING_FINISHED
	jp nz, Buzz                 ; a toy goes on the Finished belt only
	ld a, [wFinishedUnder]
	ld e, a
	ld d, 0
	ld hl, wFinishedSlots
	add hl, de
	ld a, [hl]
	and a
	jp nz, Buzz                 ; that stretch of belt already has a toy
	ld a, [wToy]
	ld [hl], a
	ld a, [wFinishedUnder]
	or 1 << 4                   ; the Finished belt
	call QueueSlot
	xor a
	ld [wToy], a
	ld hl, wDrops
	inc [hl]
	; Score +1, in BCD.
	ld hl, wScore
	ld a, [hl]
	add 1
	daa
	ld [hli], a
	ld a, [hl]
	adc 0
	daa
	ld [hl], a
	ld a, 1
	ld [wScoreDirty], a
	ret
.half
	ld a, [wHeldCount]
	and a
	jp z, Buzz                  ; nothing to give back
	ld a, [wFacing]
	cp FACING_PARTS
	jp nz, Buzz                 ; halves go back on the parts belt only
	call PartsSlotUnder
	and a
	jp nz, Buzz                 ; no room on the belt where she stands
	ld a, [wHeldCount]
	dec a
	ld [wHeldCount], a
	ld e, a
	ld d, 0
	push hl
	ld hl, wHands
	add hl, de
	ld a, [hl]                  ; the last half grabbed
	ld [hl], 0
	pop hl
	ld [hl], a
	ld a, [wPartsUnder]
	call QueueSlot
	ld hl, wReturns
	inc [hl]
	jp SetWanted

; B+A with two halves: they become the finished toy.
Assemble:
	xor a
	ld [wChordA], a
	ld [wChordB], a
	ld a, [wHands]
	dec a
	srl a
	inc a
	ld [wToy], a
	xor a
	ld [wHands], a
	ld [wHands + 1], a
	ld [wHeldCount], a
	ld hl, wAssembles
	inc [hl]
	jp SetWanted

; Refused: count it, and sound the buzz unless one just sounded.
Buzz::
	ld hl, wBuzzes
	inc [hl]
	ld a, [wBuzzTimer]
	and a
	ret nz
	ld a, BUZZ_COOLDOWN
	ld [wBuzzTimer], a
	jp SoundBuzz

; wWantedPiece: the other half of Elle's one half, else 0. A new want
; restarts the supply's count toward a forced match.
SetWanted:
	ld a, [wHeldCount]
	dec a
	ld a, 0
	jr nz, .set
	ld a, [wHands]
	dec a
	xor 1                       ; the toy's other half
	inc a
.set
	ld hl, wWantedPiece
	cp [hl]
	ret z
	ld [hl], a
	xor a
	ld [wSinceMatch], a
	ret

; hl = the parts slot under Elle, a = its piece. Keeps bc.
PartsSlotUnder:
	ld a, [wPartsUnder]
	ld e, a
	ld d, 0
	ld hl, wPartsSlots
	add hl, de
	ld a, [hl]
	ret

; Writes Elle (facing the right way) and what she holds into shadow OAM.
ElleDraw::
	; Held things first, so they draw over her.
	ld hl, wShadowOAM
	ld a, [wToy]
	and a
	jr z, .halves
	dec a
	call ToyTile
	add 8                       ; the finished frame
	ld b, TOY_HELD_Y + OAM_Y_OFS
	ld c, TOY_HELD_X + OAM_X_OFS
	call Put16
	call Hide2
	jr .elle
.halves
	ld a, [wHands]
	ld c, HAND_LEFT_X + OAM_X_OFS
	call PutHeldHalf
	ld a, [wHands + 1]
	ld c, HAND_RIGHT_X + OAM_X_OFS
	call PutHeldHalf
.elle
	SWITCH_BANK ElleFrames
	ld a, [wFacing]
	cp FACING_PARTS
	ld a, ELLE_FRAME_FRONT
	jr z, .frame
	ld a, ELLE_FRAME_BACK
.frame
	add a
	ld e, a
	ld d, 0
	ld hl, ElleFrames
	add hl, de
	ld a, [hli]
	ld h, [hl]
	ld l, a                     ; hl: the frame's object list
	ld b, [hl]
	inc hl
	ld de, wShadowOAM + ELLE_OAM
.object
	ld a, [hli]
	add ELLE_Y + OAM_Y_OFS
	ld [de], a
	inc e
	ld a, [hli]
	add ELLE_X + OAM_X_OFS
	ld [de], a
	inc e
	ld a, [hli]
	add T_ELLE
	ld [de], a
	inc e
	ld a, [hli]
	ld [de], a
	inc e
	dec b
	jr nz, .object
	ret

; A held half (piece a, 0 for none) at x c (OAM), HAND_Y; hl advances 8.
PutHeldHalf:
	and a
	jr z, Hide2
	dec a
	ld e, a                     ; toy * 2 + half
	srl a
	call ToyTile                ; the toy's first tile (clobbers b)
	bit 0, e
	jr z, .halfA
	add 4
.halfA
	ld b, HAND_Y + OAM_Y_OFS
	; fall through

; Two 8x16 objects at OAM y b, x c, tiles a (left column) and a + 2.
Put16:
	ld [hl], b
	inc l
	ld [hl], c
	inc l
	ld [hli], a
	ld [hl], 0
	inc l
	ld [hl], b
	inc l
	ld d, a
	ld a, c
	add 8
	ld [hli], a
	ld a, d
	add 2
	ld [hli], a
	ld [hl], 0
	inc l
	ret

; Two unused objects (y 0 hides them); hl advances 8.
Hide2:
	xor a
	REPT 8
	ld [hli], a
	ENDR
	ret

SECTION "Play variables", WRAM0

wFacing:: db                       ; FACING_PARTS or FACING_FINISHED
wPlayVars:
wHands:: ds 2                      ; the halves held (pieces), first grabbed first; 0 = empty hand
wHeldCount:: db                    ; how many halves: 0-2
wToy:: db                          ; the finished toy held: toy + 1, or 0
wChordA:: db                       ; frames left for B to join a waiting A, or 0
wChordB:: db                       ; frames left for A to join a waiting B, or 0
wBuzzTimer: db                     ; frames until a buzz may sound again
wScore:: dw                        ; toys made this round, BCD (low byte: tens and ones)
wScoreDirty:: db                   ; 1: the VBlank handler redraws the score
wGrabs:: db                        ; counters for the rules check
wReturns:: db
wAssembles:: db
wDrops:: db
wBuzzes:: db
wPlayVarsEnd:
