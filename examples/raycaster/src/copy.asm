; The VRAM copy: a view buffer to the tile set and map that are not on
; screen, in VBlank only.
;
; Each view's copy is a list of 16-byte chunks: first the view's map rows
; that changed (from its meta page into the back map), then each edge tile
; DrawView listed (from the buffer into the back set). Uniform tiles are not
; copied: the map points them at shared tiles.

include "hardware.inc"
include "defs.inc"

SECTION "VRAM copy", ROM0

; Called from the VBlank handler while wCopyPending is set: copies chunks
; while LY is below COPY_LAST_LY, and after the last one shows the view
; (ShowView) and its keys, now or at the start of the next VBlank. Reads the source with pop (SP
; points into WRAM), so interrupts must be off, as they are in the handler.
CopyChunks::
	ld a, [wCopyLeft]
	and a
	jp z, .done                 ; nothing (left) to copy: a view can need none
	ld b, a                     ; chunks left
	ld [wSavedSP], sp
	ld a, [wCopyIdx]
	ld c, a
.chunk
	; Start a chunk only while it and OAM DMA after it end inside VBlank.
	; (LY reads 0 for most of line 153, so a reading below 144 also stops.)
	ldh a, [rLY]
	sub LY_VBLANK
	cp COPY_LAST_LY - LY_VBLANK
	jr nc, .copied
	ldh a, [hRowCount]
	ld e, a
	ld a, c
	sub e
	jr nc, .tile
	; A changed map row r = wRowList[c]: from the meta page to the back map.
	ld a, c
	add LOW(wRowList)
	ld l, a
	ld h, HIGH(wRowList)
	ld e, [hl]
	ld a, e
	swap a
	ld l, a
	ld a, [wCopyMeta]
	ld h, a
	ld sp, hl
	ld a, e
	add a
	add LOW(MapRowOffset)
	ld l, a
	ld h, HIGH(MapRowOffset)
	ld a, [hli]
	ld e, a
	ld a, [wBackMap]
	add [hl]
	ld h, a
	ld l, e
	jr .copy16
.tile
	; Tile t from the list: buffer + t * 16 to the back set + t * 16.
	add LIST_START
	ld l, a
	ld a, [wCopyMeta]
	ld h, a
	ld a, [hl]
	swap a
	ld e, a
	and $0F
	ld d, a
	xor e                       ; the low nibble moved up: (t & 15) << 4
	ld e, a
	ld a, [wCopySrc]
	add d
	ld h, a
	ld l, e
	ld sp, hl
	ld a, [wBackSet]
	add d
	ld h, a
	ld l, e
.copy16
	; 16 bytes; a chunk never crosses a 256-byte page, so inc l suffices.
	REPT 8
		pop de
		ld [hl], e
		inc l
		ld [hl], d
		inc l
	ENDR
	inc c
	dec b
	jr nz, .chunk
.copied::                       ; every VRAM write of this VBlank is done

	ld hl, wSavedSP
	ld a, [hli]
	ld h, [hl]
	ld l, a
	ld sp, hl
	ld a, c
	ld [wCopyIdx], a
	ld a, b
	ld [wCopyLeft], a
	and a
	ret nz
.done
	; Show it now if VBlank has room for the flip and a second OAM DMA; if
	; not (or frame-locked), the VBlank handler shows it first thing in the
	; next VBlank. Either way the walls and the keys change in one frame.
	ld a, [wLock]
	and a
	jr nz, .later
	ldh a, [rLY]
	sub LY_VBLANK
	cp FLIP_LAST_LY - LY_VBLANK
	jr c, ShowView
.later
	ld a, 1
	ld [wFlipReady], a
	ret

; The whole view is in the back set and map: show them, and its keys' OAM
; page (OAM DMA). Call only in VBlank.
ShowView::
	ldh a, [rLCDC]
	xor LCDC_BLOCK01 | LCDC_BG_9C00
	ldh [rLCDC], a
.flipped::                      ; the view's last write to the PPU
	ld a, [wShown]
	xor 1
	ld [wShown], a
	add HIGH(wOamA)             ; its OAM page (sprite.asm)
	call hOAMDMA
.oamDone::                      ; and its keys'; the rest needs no VBlank
	call SetBack
	ld hl, wCopyPose
	ld de, wViewX
	ld b, 7                     ; with wViewBob
.pose
	ld a, [hli]
	ld [de], a
	inc de
	dec b
	jr nz, .pose
	ld hl, wViews
	inc [hl]
	jr nz, .counted
	inc hl
	inc [hl]
.counted
	xor a
	ld [wCopyPending], a
	ld [wFlipReady], a
	ret

; Sets wBackSet and wBackMap from wShown: set A ($8000) shows with the map
; at $9800, set B ($9000) with the map at $9C00; the copy writes the other.
SetBack::
	ld a, [wShown]
	and a
	ld a, HIGH(SET_B)
	ld b, HIGH(TILEMAP1)
	jr z, .gotBack
	ld a, HIGH(SET_A)
	ld b, HIGH(TILEMAP0)
.gotBack
	ld [wBackSet], a
	ld a, b
	ld [wBackMap], a
	ret

SECTION "Map row offsets", ROM0, ALIGN[4]
; The view's map rows, as offsets from a background map's start.
MapRowOffset:
	FOR R, VIEW_TH
		dw (VIEW_MAP_Y + R) * 32 + VIEW_MAP_X
	ENDR

SECTION "Copy variables", WRAM0

wCopyPending:: db               ; nonzero while a view waits to be copied
wCopySrc:: db                   ; the high byte of its buffer (wBuf0 or wBuf1)
wCopyMeta:: db                  ; the high byte of its meta page (wMeta0 or wMeta1)
wCopyPose:: ds 7                ; the pose it was cast from, and its keys' bob phase
wFlipReady:: db                 ; nonzero once it is all copied: the next VBlank shows it
wCopyLeft:: db                  ; chunks still to copy
wCopyIdx:: db                   ; the next chunk: changed map rows, then the list
wShown:: db                     ; 0: set A on screen; 1: set B
wBackSet:: db                   ; the high byte of the set the copy writes
wBackMap:: db                   ; the high byte of the map the copy writes
wSavedSP:: dw
wViews:: dw                     ; views shown since power-on
; The pose the view on screen was cast from: what tools/reference.py renders.
wViewX:: dw
wViewY:: dw
wViewA:: dw
wViewBob:: db                   ; and its keys' bob phase
; Nonzero in the frame-locked bench mode (SELECT held at power-on): a view is
; shown only on frames that are a multiple of LOCK_PERIOD, so the screen is
; the same frame for frame whatever the renderer's speed (gbcli equiv).
wLock:: db

SECTION "Copy row list", WRAM0, ALIGN[3]
wRowList:: ds VIEW_TH           ; the view's changed map rows, for the copy

SECTION "Copy HRAM", HRAM
hRowCount:: db                  ; entries in wRowList
