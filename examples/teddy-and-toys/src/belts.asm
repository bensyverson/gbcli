; The belts: two bands of the background, each scrolled by its own SCX,
; and what rides on them.
;
; Each belt's position is 8.8 fixed point (a byte of 1/256 px, then the
; pixel), moved every frame by its speed from tuning.inc. The VBlank
; handler sets SCX 0 for the HUD and arms LYC for the first split; the
; STAT handler then runs on each split's line, sets LYC for the next one,
; and waits for that line's HBlank to write SCX, so the band's first line
; and every line after it draw with the new scroll.
;
; What rides a belt lives in its slots: the band's 256-pixel map is 16
; slots of 16 px (2x2 tiles), each empty or holding one piece (parts belt:
; 1-12, toy * 2 + half + 1) or one finished toy (Finished belt: toy + 1).
; Pieces are background tiles, so the hardware scroll moves them and no
; object limit applies. A slot is written only while it is hidden: under
; the key column or off the screen's edge, as it comes round again. The
; parts belt deals each arriving slot a piece or a gap (supply.asm); the
; Finished belt clears each arriving slot.

include "hardware.inc"
include "layout.inc"
include "tuning.inc"
include "gen/toys.inc"

DEF SLOTS EQU 16
DEF BELT_PARTS    EQU 0             ; draw-queue entries: belt << 4 | slot
DEF BELT_FINISHED EQU 1
DEF DRAW_QUEUE_SIZE EQU 16

SECTION "STAT vector", ROM0[INT_HANDLER_STAT]
	jp StatSplit

SECTION "Belts", ROM0

; Moves one belt: \1 its position, \2 its speed, \3 its direction.
; Things on a belt travel left when the view (SCX) moves right.
MACRO BELT_MOVE
	ld hl, \1
	IF (\3) == BELT_LEFT
		ld a, [hl]
		add LOW(\2)
		ld [hli], a
		ld a, [hl]
		adc HIGH(\2)
		ld [hl], a
	ELSE
		ld a, [hl]
		sub LOW(\2)
		ld [hli], a
		ld a, [hl]
		sbc HIGH(\2)
		ld [hl], a
	ENDC
ENDM

; a = the slot arriving (fully hidden, about to come into view) for a belt
; whose SCX is in a and whose direction is \1.
MACRO ARRIVING_SLOT
	IF (\1) == BELT_LEFT
		add KEY_X + 15          ; just right of the key column's left edge
	ELSE
		sub 16                  ; just off the screen's left edge
	ENDC
	swap a
	and SLOTS - 1
ENDM

; Starts both belts for a round: positions 0, the Finished belt empty and
; the parts belt dealt from the supply. Draws them: LCD off.
BeltsInit::
	xor a
	ld hl, wPartsX
	ld [hli], a
	ld [hli], a
	ld [hli], a                 ; wFinishedX
	ld [hl], a
	ld [wDrawCount], a
	ld hl, wFinishedSlots
	ld b, SLOTS
.clear
	ld [hli], a
	dec b
	jr nz, .clear
	ld hl, wPartsSlots
	ld b, SLOTS
.deal
	push bc
	push hl
	call SupplyNext
	pop hl
	pop bc
	ld [hli], a
	dec b
	jr nz, .deal
	; Remember which slots are arriving now, so only the next ones deal.
	xor a
	ARRIVING_SLOT BELT_PARTS_DIR
	ld [wPartsArriving], a
	xor a
	ARRIVING_SLOT BELT_FINISHED_DIR
	ld [wFinishedArriving], a
	; Draw every slot.
	ld c, 0
.draw
	ld a, c
	call DrawSlot
	ld a, c
	or BELT_FINISHED << 4
	call DrawSlot
	inc c
	ld a, c
	cp SLOTS
	jr nz, .draw
	jp BeltsUnderElle

; Once a frame, from the main loop: move, deal and clear arriving slots,
; and work out the slots under Elle.
BeltsUpdate::
	BELT_MOVE wPartsX, BELT_PARTS_SPEED, BELT_PARTS_DIR
	BELT_MOVE wFinishedX, BELT_FINISHED_SPEED, BELT_FINISHED_DIR

	ld a, [wPartsX + 1]
	ARRIVING_SLOT BELT_PARTS_DIR
	ld hl, wPartsArriving
	cp [hl]
	jr z, .partsDone
	ld [hl], a
	push af
	call SupplyNext
	ld b, a
	pop af
	ld e, a
	ld d, 0
	ld hl, wPartsSlots
	add hl, de
	ld [hl], b
	call QueueSlot              ; a = slot, belt parts
.partsDone
	ld a, [wFinishedX + 1]
	ARRIVING_SLOT BELT_FINISHED_DIR
	ld hl, wFinishedArriving
	cp [hl]
	jr z, BeltsUnderElle
	ld [hl], a
	ld e, a
	ld d, 0
	ld hl, wFinishedSlots
	add hl, de
	ld [hl], 0
	or BELT_FINISHED << 4
	call QueueSlot
	; fall through

; wPartsUnder, wPartsInReach and wFinishedUnder from the scrolls and
; where Elle stands (wElleX).
BeltsUnderElle::
	ld a, [wElleX]
	add ELLE_CENTER_DX
	ld c, a                     ; c: Elle's middle, screen x
	ld a, [wPartsX + 1]
	add c                       ; the map x under Elle's middle
	ld b, a
	swap a
	and SLOTS - 1
	ld [wPartsUnder], a
	ld a, b
	and 15
	sub 8                       ; px from the slot's middle, -8..7
	jr nc, .abs
	cpl
	inc a
.abs
	cp PICKUP_REACH + 1
	ld a, 0
	jr nc, .reach
	inc a
.reach
	ld [wPartsInReach], a
	ld a, [wFinishedX + 1]
	add c
	swap a
	and SLOTS - 1
	ld [wFinishedUnder], a
	ret

; Queues slot a (belt << 4 | slot) to be redrawn in VBlank. Keeps bc, de.
QueueSlot::
	ld l, a
	ld a, [wDrawCount]
	cp DRAW_QUEUE_SIZE
	ret nc                      ; can't happen: at most a few a frame
	push de
	ld e, a
	inc a
	ld [wDrawCount], a
	ld d, 0
	ld a, l
	ld hl, wDrawQueue
	add hl, de
	ld [hl], a
	pop de
	ret

; From the VBlank handler: redraws every queued slot.
BeltsVBlank::
	ld a, [wDrawCount]
	and a
	ret z
	ld b, a
	ld hl, wDrawQueue
.next
	ld a, [hli]
	push hl
	push bc
	call DrawSlot
	pop bc
	pop hl
	dec b
	jr nz, .next
	xor a
	ld [wDrawCount], a
	ret

; Draws slot a (belt << 4 | slot) from its contents, and on a GBC
; colors it with its toy's palette (an empty slot with the belt's). VRAM
; must be open. Keeps c.
DrawSlot:
	ld b, a
	and SLOTS - 1
	ld e, a
	ld d, 0                     ; de: the slot
	add a                       ; its first map column
	ld l, a
	ld h, 0
	bit 4, b
	jr nz, .finished
	push de
	ld de, TILEMAP0 + (FRONT_BELT_ROW + BELT_ITEM_ROW_OFS) * 32
	add hl, de
	pop de
	push hl
	ld hl, wPartsSlots
	add hl, de
	ld a, [hl]                  ; a piece, 1-12, or 0
	ld e, a
	and a
	jr z, .partsToy
	dec a
	srl a
	inc a                       ; its toy + 1
.partsToy
	ld b, a
	ld hl, PartsTiles
	jr .tile
.finished
	push de
	ld de, TILEMAP0 + (BACK_BELT_ROW + BELT_ITEM_ROW_OFS) * 32
	add hl, de
	pop de
	push hl
	ld hl, wFinishedSlots
	add hl, de
	ld a, [hl]                  ; a toy + 1, or 0
	ld e, a
	ld b, a
	ld hl, FinishedTiles
.tile
	; e: the table index, 0 for empty; b: the toy + 1, 0 for empty
	ld d, 0
	add hl, de
	ld d, [hl]                  ; the first tile
	pop hl                      ; the slot's map address
IF DEF(GBCLI_GBC)
	push de
	ld a, b
	call SlotColors             ; keeps bc, hl
	pop de
ENDC
	ld a, e
	and a
	jr z, .empty
	ld a, d
	jp Draw2x2
.empty
	ld a, T_BELT
	ld [hli], a
	ld [hld], a
	ld de, 32
	add hl, de
	ld [hli], a
	ld [hl], a
	ret

; The first of a piece's four tiles, by its number (1-12; 0 unused).
PartsTiles:
	db 0
	FOR TOY, TOY_COUNT
		db T_BELT_TOYS + TOY * TOY_TILES, T_BELT_TOYS + TOY * TOY_TILES + 4
	ENDR
; The first of a finished toy's four tiles, by toy + 1.
FinishedTiles:
	db 0
	FOR TOY, TOY_COUNT
		db T_BELT_TOYS + TOY * TOY_TILES + 8
	ENDR

; The LY=LYC handler: one split a call, chosen by LYC.
StatSplit:
	push af
	push hl
	ldh a, [rLYC]
	cp SPLIT_BACK_LYC
	jr nz, .notBack
	ld a, [wFinishedX + 1]
	ld l, a
	ld a, SPLIT_FLOOR_LYC
	jr .write
.notBack
	cp SPLIT_FLOOR_LYC
	jr nz, .front
	ld l, 0
	ld a, SPLIT_FRONT_LYC
	jr .write
.front
	ld a, [wPartsX + 1]
	ld l, a
	ld a, SPLIT_BACK_LYC        ; for the next frame (VBlank sets it too)
.write
	ldh [rLYC], a
	; Wait for this line's HBlank: the handler starts in its OAM scan.
.wait
	ldh a, [rSTAT]
	and STAT_MODE
	jr nz, .wait
	ld a, l
	ldh [rSCX], a
	pop hl
	pop af
	reti

SECTION "Belt variables", WRAM0

wPartsX:: dw                       ; 8.8: the parts belt's SCX
wFinishedX:: dw                    ; 8.8: the Finished belt's SCX
wPartsSlots:: ds SLOTS             ; per slot: 0 empty, else a piece (toy * 2 + half + 1)
wFinishedSlots:: ds SLOTS          ; per slot: 0 empty, else toy + 1
wPartsUnder:: db                   ; the parts slot under Elle's middle
wPartsInReach:: db                 ; 1 if that slot's middle is within PICKUP_REACH
wFinishedUnder:: db                ; the Finished slot under Elle's middle
wPartsArriving: db                 ; the slot last dealt
wFinishedArriving: db              ; the slot last cleared
wDrawCount: db
wDrawQueue: ds DRAW_QUEUE_SIZE
