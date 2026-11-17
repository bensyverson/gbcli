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
include "images.inc"

DEF FACING_PARTS    EQU 0           ; toward the camera: we see Elle's face
DEF FACING_FINISHED EQU 1           ; away: we see her back
EXPORT FACING_PARTS, FACING_FINISHED
	ASSERT CHORD_FRAMES >= 1, "CHORD_FRAMES must be 1 or more"
	ASSERT PICKUP_REACH <= 8, "PICKUP_REACH is at most 8: half a slot"

DEF ELLE_FRAME_BACK  EQU 0          ; frames of assets/sprites/elle.16x48
DEF ELLE_FRAME_FRONT EQU 1
DEF ELLE_FRAME_SIDE  EQU 2          ; mid-turn
DEF HELD_OBJECTS EQU 4              ; what Elle holds: two 16x16 things
DEF ELLE_OBJECTS EQU (ELLE_FRAME_WIDTH / 8) * (ELLE_FRAME_HEIGHT / 16)   ; at most, per frame
DEF ELLE_OAM EQU HELD_OBJECTS * 4   ; facing the parts belt: held things in OAM 0-3, then her
DEF PIECE_ATTR EQU OBJ_PAL_PIECES   ; a held piece's GBC palette; a DMG ignores bits 0-2

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
	; A turn: the pad, A and B do nothing until its last frame has passed;
	; then the new facing holds and that frame's presses act.
	ld a, [wTurnTimer]
	and a
	jr z, .pad
	dec a
	ld [wTurnTimer], a
	jp nz, ElleDraw
	ld a, [wTurnTo]
	ld [wFacing], a
.pad
	ldh a, [hJoyPressed]
	ld b, FACING_PARTS
	bit B_PAD_DOWN, a
	jr nz, .turn
	ld b, FACING_FINISHED
	bit B_PAD_UP, a
	jr z, .buttons
.turn
	ld a, [wFacing]
	cp b
	jr z, .buttons              ; already facing that belt
	ld a, b
IF TURN_FRAMES == 0
	ld [wFacing], a
ELSE
	ld [wTurnTo], a
	ld a, TURN_FRAMES
	ld [wTurnTimer], a
	ld a, SFX_TURN
	call SfxPlay
	; This frame's A or B is ignored with the turn, and so is a press
	; waiting for its chord partner. (To buffer presses through a turn
	; instead, keep these and act on them when wTurnTimer reaches 0.)
	xor a
	ld [wChordA], a
	ld [wChordB], a
	jp ElleDraw
ENDC
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
	ld a, SFX_GRAB
	call SfxPlay
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
	ld a, SFX_SCORE
	call SfxPlay
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
	ld a, SFX_ASSEMBLE
	call SfxPlay
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

; wWantedPiece: the piece that completes Elle's one half, else 0. A new want
; restarts the supply's count toward a forced match.
SetWanted:
	ld a, [wHeldCount]
	dec a
	ld a, 0
	jr nz, .set
	ld a, [wHands]
	add LOW(PiecePartner - 1)   ; the piece that completes it (gen/toys.asm)
	ld l, a
	adc HIGH(PiecePartner - 1)
	sub l
	ld h, a
	ld a, [hl]
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
	; Her frame and height: mid-turn her side, between the two belts.
	ld a, [wTurnTimer]
	and a
	ld b, ELLE_FRAME_SIDE
	ld c, ELLE_Y_SIDE
	jr nz, .pose
	ld a, [wFacing]
	cp FACING_PARTS
	ld b, ELLE_FRAME_FRONT
	ld c, ELLE_Y_PARTS
	jr z, .pose
	ld b, ELLE_FRAME_BACK
	ld c, ELLE_Y_FINISHED
.pose
	ld a, b
	ld [wElleFrame], a
	ld a, c
	ld [wElleY], a
	; Hide every object first: a frame may use fewer than another.
	ld hl, wShadowOAM
	ld b, (HELD_OBJECTS + ELLE_OBJECTS) * 4
	xor a
.hide
	ld [hli], a
	dec b
	jr nz, .hide
	; Facing the parts belt (or mid-turn), what she holds is in front of
	; her: its objects come first in OAM. Facing away, she is in front of
	; it: hers come first. A GBC draws the lower OAM index on top; a DMG
	; draws the smaller X on top and breaks ties by index, so facing away
	; a held thing's columns that overlap her share her columns' X. The
	; toy is wholly behind her; each half's outer column peeks out.
	ld a, [wElleFrame]
	cp ELLE_FRAME_BACK
	ld a, LOW(wShadowOAM)
	ld b, LOW(wShadowOAM + ELLE_OAM)
	ld d, HAND_LEFT_X + OAM_X_OFS
	ld e, HAND_RIGHT_X + OAM_X_OFS
	jr nz, .order
	ld a, LOW(wShadowOAM + ELLE_OBJECTS * 4)
	ld b, LOW(wShadowOAM)
	ld d, HAND_BACK_LEFT_X + OAM_X_OFS
	ld e, HAND_BACK_RIGHT_X + OAM_X_OFS
.order
	ld l, a                     ; hl: the held things' objects
	ld a, b
	ld [wElleOam], a
	ld a, d
	ld [wHandX], a
	ld a, e
	ld [wHandX + 1], a
	ld a, [wToy]
	and a
	jr z, .halves
	dec a
	call ToyTile
	add 8                       ; the finished frame
	ld d, a
	ld a, [wElleY]
	add TOY_HELD_DY + OAM_Y_OFS
	ld b, a
	ld a, d
	ld c, TOY_HELD_X + OAM_X_OFS
	call Put16
	jr .elle
.halves
	ld a, [wHandX]
	ld c, a
	ld a, [wHands]
	call PutHeldHalf
	ld a, [wHandX + 1]
	ld c, a
	ld a, [wHands + 1]
	call PutHeldHalf
.elle
	SWITCH_BANK ElleFrames
	ld a, [wElleY]
	add OAM_Y_OFS
	ld c, a
	ld a, [wElleFrame]
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
	ld a, [wElleOam]
	ld e, a
	ld d, HIGH(wShadowOAM)
.object
	ld a, [hli]
	ld [wElleObjY], a
	add c
	ld [de], a
	inc e
	ld a, [hli]
	ld [wElleObjX], a
	add ELLE_X + OAM_X_OFS
	ld [de], a
	inc e
	ld a, [hli]
	add T_ELLE
	ld [de], a
	inc e
IF DEF(GBCLI_GBC)
	; Her GBC palette for this object (src/colors.asm); a DMG ignores
	; these bits.
	push hl
	push bc
	ld a, [wElleObjY]
	swap a
	and $0F                     ; the row: 0-2
	add a
	ld b, a
	ld a, [wElleObjX]
	rrca
	rrca
	rrca
	and 1                       ; the column
	add b
	ld b, a
	ld a, [wElleFrame]
	ld c, a
	add a
	add c
	add a                       ; frame * 6
	add b
	add LOW(ElleObjPalettes)
	ld l, a
	adc HIGH(ElleObjPalettes)
	sub l
	ld h, a
	ld a, [hl]
	ld [wElleObjPal], a
	pop bc
	pop hl
	ld a, [wElleObjPal]
	or [hl]
	inc hl
ELSE
	ld a, [hli]
ENDC
	ld [de], a
	inc e
	dec b
	jr nz, .object
	ret

; A held half (piece a, 0 for none) at x c (OAM), in Elle's hands; hl
; advances 8.
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
	ld d, a
	ld a, [wElleY]
	add HAND_DY + OAM_Y_OFS
	ld b, a
	ld a, d
	; fall through

; Two 8x16 objects at OAM y b, x c, tiles a (left column) and a + 2.
Put16:
	ld [hl], b
	inc l
	ld [hl], c
	inc l
	ld [hli], a
	ld [hl], PIECE_ATTR
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
	ld [hl], PIECE_ATTR
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
wTurnTimer:: db                    ; frames left in a turn (TURN_FRAMES), or 0
wTurnTo: db                        ; the facing a turn ends in
wElleFrame:: db                    ; ELLE_FRAME_*: how ElleDraw last drew her
wElleY:: db                        ; her top's screen y, as ElleDraw last drew her
wElleObjY: db                      ; ElleDraw: the object's place in her frame
wElleObjX: db
wElleObjPal: db                    ; ElleDraw: its GBC palette
wElleOam: db                       ; ElleDraw: the low byte of her first object in wShadowOAM
wHandX: ds 2                       ; ElleDraw: the held halves' OAM x, left hand then right
wPlayVarsEnd:
