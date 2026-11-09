; Lantern: a first-person raycaster for the Game Boy.
;
; The main loop casts one ray per 2-pixel column into a WRAM buffer laid out
; exactly as the view's tiles (two buffers: it casts into one while the other
; is copied). The VBlank handler copies the buffer's edge tiles and changed
; map rows into whichever VRAM tile set and map are not on screen, as many
; 16-byte chunks as VBlank allows, and flips LCDC bits 4 and 3 at the start
; of the VBlank after the copy is done, along with the keys' OAM page, so the
; view never shows half a frame, the keys always match it, and no VRAM write
; lands outside VBlank.

include "hardware.inc"
include "defs.inc"

SECTION "Restart vectors", ROM0[$0000]
	ds $40, $FF                 ; rst $38

SECTION "VBlank vector", ROM0[INT_HANDLER_VBLANK]
VBlankVector:
	jp VBlank

SECTION "Header", ROM0[$0100]
	nop
	jp Start
	ds $0150 - @, 0

SECTION "Start", ROM0

Start:
	di
	ld sp, wStackTop

IF DEF(GBCLI_GBC)
	; `gbcli build --gbc` defines GBCLI_GBC; the same ROM still runs on a DMG,
	; so the check happens at run time (see the starter's notes).
	DEF HEADER_GBC_FLAG EQU $0143
	DEF HEADER_GBC EQU $80
	cp BOOTUP_A_CGB
	ld a, 0
	jr nz, .gbcKnown
	ld a, [HEADER_GBC_FLAG]
	and HEADER_GBC
.gbcKnown
	ldh [hGbc], a
ENDC

	; Turn the LCD off, but only during VBlank.
.waitVBlank
	ldh a, [rLY]
	cp LY_VBLANK
	jr c, .waitVBlank
	xor a
	ldh [rLCDC], a
	ldh [rNR52], a

IF DEF(GBCLI_GBC)
	; On a GBC, switch to double speed: the copy and the casting both get
	; twice the cycles. Ben's ruling: the DMG build must reach its target
	; without this.
	ldh a, [hGbc]
	and a
	jr z, .singleSpeed
	xor a
	ldh [rIE], a
	ld a, JOYP_GET_NONE
	ldh [rP1], a
	ld a, SPD_PREPARE
	ldh [rKEY1], a
	stop
.singleSpeed
ENDC

	; Start the timer: the LCD comes on at its first overflow, a fixed time
	; from here whatever the setup below costs, so two builds' frames line up
	; for gbcli equiv. The setup must finish first (gbcli measure
	; Start.timed..Start.setUp shows its length; the overflow is 262,144 T).
.timed::
	xor a
	ldh [rTAC], a
	ldh [rTIMA], a
	ldh [rTMA], a
	ldh [rDIV], a
	ldh [rIF], a
	ld a, TAC_START | TAC_4KHZ
	ldh [rTAC], a

	; Clear WRAM below the stack, so every variable starts at 0.
	ld hl, $C000
	xor a
	ld b, 0                     ; 256 blocks: $C000-$CFFF
	call FillFast
	ld b, (wStackBottom - $D000) / 16
	call FillFast

	; SELECT held at power-on: the frame-locked bench mode (see wLock).
	call ReadJoypad
	ld a, [wPad]
	and PAD_SELECT
	ld [wLock], a

	call CopyOAMDMA
	ld a, %11_10_01_00
	ldh [rBGP], a
	ldh [rOBP0], a
	xor a
	ldh [rSCX], a
	ldh [rSCY], a
IF DEF(GBCLI_GBC)
	ldh a, [hGbc]
	and a
	call nz, ColorGbc
ENDC
	call SoundInit

.setUp::
	; Wait for the timer's first overflow (see Start.timed). IME is off, so
	; halt wakes on the flag without calling a handler.
	ld a, IE_TIMER
	ldh [rIE], a
.waitTimer
	halt
	nop
	ldh a, [rIF]
	and IF_TIMER
	jr z, .waitTimer
	xor a
	ldh [rTAC], a
	jp Title

; Sets up the maze's video with the LCD off: both view tile sets cleared,
; the shared tiles (shades, walls, font), both maps, and the HUD.
InitMazeVideo::
	ld hl, SET_A
	xor a
	ld b, 0                     ; $8000-$8FFF
	call FillFast
	ld b, $800 / 16             ; $9000-$97FF
	call FillFast
	ld hl, TILEBLOCK1 + TILE_SOLID * 16 - $800
	ld b, 1
	ld a, $FF
	call FillFast
	ld hl, TILEBLOCK1 + TILE_SHADE1 * 16 - $800
	ld b, 8
.shade1
	ld a, $FF
	ld [hli], a
	xor a
	ld [hli], a
	dec b
	jr nz, .shade1
	ld b, 8
.shade2                         ; TILE_SHADE2 follows TILE_SHADE1
	xor a
	ld [hli], a
	dec a
	ld [hli], a
	dec b
	jr nz, .shade2

	; The shared wall tiles: every row's low plane from TileX, high $FF.
	ld hl, TILEBLOCK1 + TILE_WALL0 * 16 - $800
	ld de, TileX
	ld c, 16
.wallTile
	ld a, [de]
	inc de
	ld b, 8
.wallRow
	ld [hli], a
	ld [hl], $FF
	inc hl
	dec b
	jr nz, .wallRow
	dec c
	jr nz, .wallTile
	call LoadFont
	call LoadKeys

	; Both background maps, one per tile set: the view's map is copied into
	; the one not on screen along with its tiles.
	ld hl, TILEMAP0
	call InitMap
	ld hl, TILEMAP1
	call InitMap
	; Set A shows first, so the first copy goes to set B.
	xor a
	ld [wShown], a
	ld [wCopyPending], a
	ld [wFlipReady], a
	call SetBack
	; Each buffer's meta map starts as the back maps do (DrawView copies
	; only the rows that differ from it).
	ld hl, wMeta0
	ld a, TILE_BLANK
	ld b, VIEW_TW * VIEW_TH / 16
	call FillFast
	ld hl, wMeta1
	ld b, VIEW_TW * VIEW_TH / 16
	call FillFast
	ld a, HIGH(wBuf0)
	ldh [hDrawBuf], a
	ld a, HIGH(wMeta0)
	ldh [hDrawMeta], a
IF DEF(GBCLI_GBC)
	ldh a, [hGbc]
	and a
	call nz, ClearAttrs
ENDC
	; The whole HUD, now, while the LCD is off.
.hud
	call UpdateHud
	ld a, [wHudFlags]
	and a
	jr nz, .hud
	ret

; Starts a game: the LCD off, the maze's video, a new game, the maze theme,
; and the LCD back on. Comes from the title with interrupts off.
StartMaze::
	call LcdOff
	call NewGame
	call InitMazeVideo
	ld hl, song_maze
	call MusicPlay
	ld a, MODE_MAZE
	ld [wMode], a
	ld a, LCDC_ON | LCDC_BG_ON | LCDC_OBJ_ON | LCDC_OBJ_16 | LCDC_BLOCK01 | LCDC_BG_9800
	ldh [rLCDC], a
	ld a, IE_VBLANK
	ldh [rIE], a
	xor a
	ldh [rIF], a
	ei

MainLoop:
	; After an ending, START goes back to the title.
	ld a, [wGoTitle]
	and a
	jp nz, Title
	call RenderView             ; into the draw buffer
	; Wait for the VBlank handler to finish copying the previous view.
.waitCopy
	ld a, [wCopyPending]
	and a
	jr z, .handOff
	halt
	nop
	jr .waitCopy
.handOff
	; Hand this buffer, its keys, and the pose it was cast from, to the
	; VBlank handler.
	call StageKeys
	ld hl, wRPosX
	ld de, wCopyPose
	ld b, 7                     ; wRPosX, wRPosY, wRAngle, wRBob
.pose
	ld a, [hli]
	ld [de], a
	inc de
	dec b
	jr nz, .pose
	ldh a, [hDrawBuf]
	ld [wCopySrc], a
	xor HIGH(wBuf0) ^ HIGH(wBuf1)
	ldh [hDrawBuf], a           ; cast the next view into the other buffer
	ldh a, [hDrawMeta]
	ld [wCopyMeta], a
	xor HIGH(wMeta0) ^ HIGH(wMeta1)
	ldh [hDrawMeta], a
	; The changed map rows, as a list for the copy.
	ldh a, [hDirty]
	ld b, a
	ld hl, wRowList
	ld d, 0                     ; rows listed
	ld c, d                     ; row
.rows
	srl b
	jr nc, .clean
	ld [hl], c
	inc hl
	inc d
.clean
	inc c
	ld a, c
	cp VIEW_TH
	jr nz, .rows
	ld a, d
	ldh [hRowCount], a
	ldh a, [hListLen]
	add d
	ld [wCopyLeft], a
	xor a
	ld [wCopyIdx], a
	inc a
	ld [wCopyPending], a
	; Frame-locked, the next view is cast only once this one is shown, so
	; its pose is taken on the same frame whatever the renderer's speed.
	ld a, [wLock]
	and a
	jr z, MainLoop
.waitShown
	halt
	nop
	ld a, [wCopyPending]
	and a
	jr nz, .waitShown
	jr MainLoop

; ---------------------------------------------------------------------------
; VBlank: OAM DMA, then (in the maze) the HUD and the view copy while VBlank
; lasts; then the frame count, input, the game, the music and the effects,
; none of which need VRAM.

VBlank:
	push af
	push bc
	push de
	push hl
	; OAM DMA first, so it never runs past VBlank; the copy then takes the
	; rest of VBlank, as LY allows. A view copied in full by an earlier
	; VBlank is shown here, its keys' OAM page with it (ShowView's own DMA).
	; Frame-locked, it is shown only on the period's tick; on the title,
	; never (a view handed off as the game ended must not flip the title).
	ld a, [wMode]
	and a                       ; MODE_TITLE
	jr z, .noFlip
	ld a, [wFlipReady]
	and a
	jr z, .noFlip
	ld a, [wLock]
	and a
	jr z, .flip
	ld a, [wFrame]
	and LOCK_PERIOD - 1
	jr nz, .noFlip
.flip
	call ShowView
	jr .oamDone
.noFlip
	ld a, [wShown]
	add HIGH(wOamA)             ; the shown set's OAM page
	call hOAMDMA
.oamDone::
	ld a, [wMode]
	and a                       ; MODE_TITLE
	jr z, .vramDone
	call UpdateHud
.hudDone::
	ld a, [wCopyPending]
	and a
	jr z, .vramDone
	ld a, [wFlipReady]
	and a
	call z, CopyChunks
.vramDone
	ld hl, wFrame
	inc [hl]
	jr nz, .counted
	inc hl
	inc [hl]
.counted
	call ReadJoypad
	ld a, [wMode]
	and a
	call nz, GameTick
	call MusicTick
	call SfxTick
	pop hl
	pop de
	pop bc
	pop af
	reti

; Waits for VBlank, then turns the LCD off. Call with interrupts off.
LcdOff::
	ldh a, [rLCDC]
	bit B_LCDC_ENABLE, a
	ret z
.wait
	ldh a, [rLY]
	cp LY_VBLANK
	jr c, .wait
	xor a
	ldh [rLCDC], a
	ret

IF DEF(GBCLI_GBC)
; GBC mode only: BG palette 0 in lamplight colors, one per DMG shade, so the
; picture is the DMG's recolored (tools/reference.py reads the
; shades from the swatch).
ColorGbc:
	ld a, BGPI_AUTOINC
	ldh [rBCPS], a
	ld hl, GrayPalette
	ld b, GrayPalette.end - GrayPalette
.bg
	ld a, [hli]
	ldh [rBCPD], a
	dec b
	jr nz, .bg
	ret

; GBC mode only, LCD off: attribute 0 (palette 0) for every tile of both maps.
ClearAttrs::
	ld a, 1
	ldh [rVBK], a
	ld hl, ATTRMAP0
	xor a
	ld b, $800 / 16
	call FillFast
	xor a
	ldh [rVBK], a
	ret

MACRO rgb
	dw (\1) | (\2) << 5 | (\3) << 10
ENDM

; Lamplight: cream, tan, brown and soot, in the DMG's four shades' order.
GrayPalette:
	rgb 31, 29, 22
	rgb 24, 17, 9
	rgb 12, 7, 4
	rgb 3, 2, 2
.end
ENDC

; Fills the background map at HL: the solid frame, a blank view, the swatch.
InitMap:
	push hl
	ld b, $400 / 16
	ld a, TILE_SOLID
	call FillFast
	pop hl
	push hl
	ld de, VIEW_MAP_Y * 32 + VIEW_MAP_X
	add hl, de
	ld c, VIEW_TH
.row
	ld b, VIEW_TW
	ld a, TILE_BLANK
.col
	ld [hli], a
	dec b
	jr nz, .col
	ld de, 32 - VIEW_TW
	add hl, de
	dec c
	jr nz, .row
	; The shade swatch: all four grays on screen, lightest first, so
	; tools/reference.py can tell them apart on any palette.
	pop hl
	ld de, SWATCH_MAP_Y * 32 + SWATCH_MAP_X
	add hl, de
	ld a, TILE_BLANK
	ld [hli], a
	ld a, TILE_SHADE1
	ld [hli], a
	inc a
	ld [hli], a
	ld [hl], TILE_SOLID
	ret

; ---------------------------------------------------------------------------
; Memory helpers.

; Writes B blocks of 16 copies of A from HL (B = 0: 256 blocks).
FillFast::
	REPT 16
		ld [hli], a
	ENDR
	dec b
	jr nz, FillFast
	ret

; Writes BC zero bytes from HL.
ClearMem::
	xor a
; Writes BC copies of A from HL.
FillMem::
	ld e, a
.loop
	ld a, e
	ld [hli], a
	dec bc
	ld a, b
	or c
	jr nz, .loop
	ret

; Copies BC bytes from DE to HL.
CopyMem::
	ld a, [de]
	ld [hli], a
	inc de
	dec bc
	ld a, b
	or c
	jr nz, CopyMem
	ret

; ---------------------------------------------------------------------------

SECTION "Variables", WRAM0

wFrame:: dw                     ; VBlanks since the LCD came on
wPad:: db                       ; buttons held: PAD_* bits

; The player, in 8.8 map units, and the view angle (0-1023).
wPosX:: dw
wPosY:: dw
wAngle:: dw

wMode:: db                      ; MODE_TITLE or MODE_MAZE
wGoTitle:: db                   ; nonzero: the main loop goes back to the title

IF DEF(GBCLI_GBC)
SECTION "GBC mode", HRAM
hGbc:: db                       ; nonzero in GBC mode
ENDC


SECTION "Stack", WRAMX[$DF00]
wStackBottom:: ds $100
wStackTop::
