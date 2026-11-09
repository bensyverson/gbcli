; The keys as floating objects: each key still in the maze is projected from
; the pose the view is cast from, drawn as one frame of the key sheet chosen
; by its distance, and each of its 8x16 objects is hidden where the wall in
; its middle column is nearer than the key. tools/raymath.py's key_objects
; mirrors every step; keep the two in step.
;
; PlaceKeys writes the objects into wSprStage after the view is cast; the
; main loop hands them, with the view, to the OAM page of the tile set the
; view is copied into (StageKeys), and the VBlank handler shows that page
; when it flips to the view, so the keys always match the walls.

include "hardware.inc"
include "defs.inc"
include "images.inc"

SECTION "Keys", ROM0

; Projects every key from wRPosX, wRPosY, wRAngle (and wRBob) into wSprStage,
; KEY_SLOTS entries a key; an entry not drawn has y = 0.
PlaceKeys::
	; The view direction's cos and sin: magnitudes and signs (1: negative).
	ld a, [wRAngle]
	ld l, a
	ld a, [wRAngle + 1]
	ld b, a
	add HIGH(CosMag)
	ld h, a
	ld a, [hl]
	ldh [hCos], a
	ld a, b
	inc a
	rrca
	and 1                       ; quadrants 1 and 2: cos < 0
	ldh [hCosNeg], a
	ld a, b
	dec a
	and ANG_MASK_HI
	ld c, a
	add HIGH(CosMag)
	ld h, a
	ld a, [hl]
	ldh [hSin], a
	ld a, c
	inc a
	rrca
	and 1
	ldh [hSinNeg], a

	; The farthest wall in the view: a key beyond it is hidden whatever its
	; place, so it costs only its depth.
	ld hl, wDepth
	ld bc, (RAYS / 4) << 8      ; B = groups of 4 columns, C = the maximum
.max
	REPT 4
		ld a, [hli]
		cp c
		jr c, .notMax\@
		ld c, a
.notMax\@
	ENDR
	dec b
	jr nz, .max
	ld a, c
	ldh [hMaxDepth], a

	ld hl, wSprStage
	ld a, l
	ldh [hSlot], a
	ld a, h
	ldh [hSlot + 1], a
	xor a
.key
	ldh [hKey], a
	call PlaceKey
	ldh a, [hKey]
	inc a
	cp MAZE_KEYS
	jr nz, .key
	ret

; One key, hKey, into the KEY_SLOTS entries at hSlot; advances hSlot.
PlaceKey:
	ld e, a
	ld d, 0
	ld hl, wKeyCell
	add hl, de
	ld a, [hl]
	cp $FF
	jp z, HideKey               ; taken
	; dx, dy: the cell's middle less the pose, signed 8.8.
	ld b, a
	and $0F
	ld d, a
	ld e, $80
	ld a, [wRPosX]
	ld c, a
	ld a, e
	sub c
	ldh [hKDX], a
	ld a, [wRPosX + 1]
	ld c, a
	ld a, d
	sbc c
	ldh [hKDX + 1], a
	ld a, b
	swap a
	and $0F
	ld d, a
	ld a, [wRPosY]
	ld c, a
	ld a, e
	sub c
	ldh [hKDY], a
	ld a, [wRPosY + 1]
	ld c, a
	ld a, d
	sbc c
	ldh [hKDY + 1], a

	; depth = dx cos + dy sin
	ldh a, [hKDX]
	ld e, a
	ldh a, [hKDX + 1]
	ld d, a
	ldh a, [hCosNeg]
	ld c, a
	ldh a, [hCos]
	call SMul
	push hl
	ldh a, [hKDY]
	ld e, a
	ldh a, [hKDY + 1]
	ld d, a
	ldh a, [hSinNeg]
	ld c, a
	ldh a, [hSin]
	call SMul
	pop de
	add hl, de
	ld a, l
	ldh [hDepth], a
	ld a, h
	ldh [hDepth + 1], a
	; Behind, or nearer than DEPTH_MIN: not drawn.
	bit 7, h
	jp nz, HideKey
	and a
	jr nz, .deepEnough
	ld a, l
	cp DEPTH_MIN
	jp c, HideKey
.deepEnough
	; Its depth in quarter cells (as wDepth's); beyond the farthest wall,
	; hidden.
	ld a, l
	and $C0
	or h
	rlca
	rlca
	ldh [hKD], a
	ld b, a
	ldh a, [hMaxDepth]
	cp b
	jp c, HideKey

	; lat = dy cos - dx sin
	ldh a, [hKDY]
	ld e, a
	ldh a, [hKDY + 1]
	ld d, a
	ldh a, [hCosNeg]
	ld c, a
	ldh a, [hCos]
	call SMul
	push hl
	ldh a, [hKDX]
	ld e, a
	ldh a, [hKDX + 1]
	ld d, a
	ldh a, [hSinNeg]
	ld c, a
	ldh a, [hSin]
	call SMul
	pop de
	ld a, e
	sub l
	ld e, a
	ld a, d
	sbc h
	ld d, a                     ; DE = lat

	; The key's x, from Proj by depth >> 4 (capped at 255), and its half
	; height.
	ldh a, [hDepth + 1]
	ld h, a
	ldh a, [hDepth]
	ld l, a
	ld a, h
	cp 16
	ld a, $FF
	jr nc, .far
	ld a, l
	and $F0
	or h
	swap a
.far
	ld l, a
	ld h, HIGH(Proj)
	ld a, [hl]
	ld c, 0
	call SMul                   ; HL = lat * Proj[kd] >> 8: pixels from the middle
	ld de, KEY_X
	add hl, de
	ld a, h
	and a
	jp nz, HideKey
	ld a, l
	cp 8
	jp c, HideKey
	cp 144 + 1
	jp nc, HideKey
	ldh [hBase], a

	ldh a, [hDepth]
	ld l, a
	ldh a, [hDepth + 1]
	ld h, a
	srl h
	rr l
	srl h
	rr l
	srl h
	rr l
	ld a, h
	add HIGH(HalfHeight)
	ld h, a
	ld b, [hl]                  ; B = half height

	; The frame, and its bob: full for frame 0, half for 1, none past it.
	ld a, [wRBob]
	add LOW(Bob)
	ld l, a
	ld h, HIGH(Bob)
	ld c, [hl]
	ld a, b
	ld e, 0
	cp FRAME0_HH
	jr nc, .gotFrame
	sra c
	inc e
	cp FRAME1_HH
	jr nc, .gotFrame
	ld c, 0
	inc e
	cp FRAME2_HH
	jr nc, .gotFrame
	inc e
.gotFrame
	; hKeyY = KEY_Y + (hh >> 2) + bob
	ld a, b
	srl a
	srl a
	add KEY_Y
	add c
	ldh [hKeyY], a
	; DE = the frame's object list.
	ld d, 0
	sla e
	ld hl, KeyFrames
	add hl, de
	ld a, [hli]
	ld h, [hl]
	ld l, a
	ld a, [hli]
	ld c, a                     ; C = objects in the frame
	ld b, KEY_SLOTS
	; DE = the slot.
	ldh a, [hSlot]
	ld e, a
	ldh a, [hSlot + 1]
	ld d, a
.object
	; HL: the object's y, x, tile, attributes (from the frame's top left).
	push bc
	ld a, [hli]
	ld b, a                     ; y in the frame
	ldh a, [hBase]
	add [hl]                    ; OAM x = base + x
	inc hl
	ld c, a
	; Hidden unless its left edge is in the view (OAM x 24 to 144) and the
	; wall in its middle column is no nearer than the key.
	sub 24
	jr c, .hide
	cp 120 + 1
	jr nc, .hide
	add 4
	srl a
	push hl
	ld l, a
	ld h, HIGH(wDepth)
	ldh a, [hKD]
	cp [hl]
	pop hl
	jr z, .show
	jr nc, .hide                ; key deeper than the wall
.show
	ldh a, [hKeyY]
	add b
	ld [de], a
	inc de
	ld a, c
	ld [de], a
	inc de
	ld a, [hli]
	add KEY_TILE_BASE
	ld [de], a
	inc de
	ld a, [hli]
	ld [de], a
	inc de
	jr .next
.hide
	inc hl
	inc hl
	xor a
	ld [de], a
	inc de
	inc de
	inc de
	inc de
.next
	pop bc
	dec b
	dec c
	jr nz, .object
	; The frame's unused slots.
	xor a
	inc b
	jr .tailTest
.tail
	ld [de], a
	inc de
	inc de
	inc de
	inc de
.tailTest
	dec b
	jr nz, .tail
	ld a, e
	ldh [hSlot], a
	ld a, d
	ldh [hSlot + 1], a
	ret

; Hides the key's KEY_SLOTS entries at hSlot and advances hSlot.
HideKey:
	ldh a, [hSlot]
	ld l, a
	ldh a, [hSlot + 1]
	ld h, a
	ld b, KEY_SLOTS
	xor a
.slot
	ld [hli], a
	inc hl
	inc hl
	inc hl
	dec b
	jr nz, .slot
	ld a, l
	ldh [hSlot], a
	ld a, h
	ldh [hSlot + 1], a
	ret

; HL = (|DE| * A) >> 8, negated if DE < 0 xor C bit 0 is set: a signed
; 8.8 value times a magnitude with its own sign. Clobbers BC, DE.
SMul:
	ld b, a
	ld a, d
	add a                       ; carry: DE < 0
	ld a, c
	jr nc, .positive
	xor 1
	push af
	xor a
	sub e
	ld e, a
	ld a, 0
	sbc d
	ld d, a
	pop af
.positive
	ldh [hSign], a
	ld a, b
	call Mul16x8
	ldh a, [hSign]
	rra
	ret nc
	xor a
	sub l
	ld l, a
	ld a, 0
	sbc h
	ld h, a
	ret

; Copies wSprStage into the OAM page of the tile set the copy writes: the
; page the VBlank handler shows once it flips to this view. Call while no
; view is waiting (wCopyPending = 0).
StageKeys::
	ld a, [wShown]
	xor 1
	add HIGH(wOamA)
	ld d, a
	ld e, 0
	ld hl, wSprStage
	ld b, MAZE_KEYS * KEY_SLOTS * 4
.copy
	ld a, [hli]
	ld [de], a
	inc e
	dec b
	jr nz, .copy
	ret

; Loads the key sheet's tiles (the GBC's colors in GBC mode) and its palette.
; Call with the LCD off.
LoadKeys::
	ld a, KEY_OBP
	ldh [rOBP0], a
	ld hl, TILEBLOCK1 + KEY_TILE_BASE * 16 - $800
	ld de, KeyTiles
IF DEF(GBCLI_GBC)
	ldh a, [hGbc]
	and a
	jr z, .gotTiles
	ld de, KeyTilesGbc
	ld a, OBPI_AUTOINC
	ldh [rOCPS], a
	push hl
	ld hl, ObjPalettesGbc
	ld b, ObjPalettesGbcEnd - ObjPalettesGbc
.palette
	ld a, [hli]
	ldh [rOCPD], a
	dec b
	jr nz, .palette
	pop hl
.gotTiles
ENDC
	ld bc, KEY_TILES * 16
	call CopyMem
	; Both OAM pages empty.
	ld hl, wOamA
	ld b, 512 / 16
	xor a
	jp FillFast

	ASSERT MAZE_KEYS == KEY_COUNT, "the maze's keys (MAZE_KEYS) must be KEY_COUNT"

SECTION "Key variables", WRAM0
wKeyCell:: ds KEY_COUNT          ; each key's map index (y * 16 + x), $FF once taken
wSprStage:: ds KEY_COUNT * KEY_SLOTS * 4 ; the cast view's key objects, for StageKeys

; One shadow OAM page per view tile set, in consecutive pages: the page for
; set A (wShown = 0) is HIGH(wOamA), set B's the next.
SECTION "OAM pages", WRAMX, ALIGN[8]
wOamA:: ds 256
wOamB:: ds OAM_SIZE

SECTION "Key HRAM", HRAM
hCos: db
hCosNeg: db
hSin: db
hSinNeg: db
hKey: db
hSlot: dw
hKDX: dw
hKDY: dw
hDepth: dw
hKD: db
hMaxDepth: db
hBase: db
hKeyY: db
hSign: db
