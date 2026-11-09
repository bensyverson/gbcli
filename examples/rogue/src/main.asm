; Lanternfall: startup, the main loop and the VBlank handler.

include "hardware.inc"
include "game.inc"
include "images.inc"

SECTION "VBlank vector", ROM0[INT_HANDLER_VBLANK]
VBlankVector:
	jp VBlank

SECTION "Header", ROM0[$0100]
	nop
	jp Start
	ds $0150 - @, 0

SECTION "Start", ROM0

Start::
	di
	ld sp, $E000                ; the stack grows down from the top of WRAM

IF DEF(GBCLI_GBC)
	; `gbcli build --gbc` defines GBCLI_GBC; the same ROM must still run on
	; a DMG, so check at run time (A is BOOTUP_A_CGB on a GBC, and the
	; header's GBC flag must be set for it to run in GBC mode).
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

	; Clear WRAM below the stack, so no variable starts as garbage.
	ld hl, STARTOF(WRAM0)
	ld bc, $DF00 - STARTOF(WRAM0)
	call ClearMem

	call CopyOAMDMA
	call LoadTitle
	call ClearWindow
	ld a, STATUS_WY
	ldh [rWY], a
	ld a, WX_OFS
	ldh [rWX], a

	ld a, PLAY_PALETTE
	ldh [rBGP], a
	ld [wBgp], a
	ld a, HERO_OBP
	ldh [rOBP0], a
	ld [wObp], a
	xor a
	ldh [rSCX], a
	ldh [rSCY], a

	ld a, LCDC_TITLE
	ldh [rLCDC], a

	ld a, IE_VBLANK
	ldh [rIE], a
	xor a
	ldh [rIF], a
	call MusicInit
	ei

	call TitleWait              ; sets wSeed from the frame START was pressed on
	call EnterDungeon
	call PlayerInit
	call NewFloor

MainLoop:
	call WaitFrame
	call ReadJoypad
	call PlayerUpdate
	jr MainLoop

; ---------------------------------------------------------------------------

; Sleeps until the VBlank handler has counted a frame.
WaitFrame::
	ld a, [wFrame]
	ld b, a
.wait
	halt
	nop
	ld a, [wFrame]
	cp b
	jr z, .wait
	ret

; Waits for START, counting frames; the count becomes the run's seed.
TitleWait:
	call WaitFrame
	ld hl, wSeedCounter
	inc [hl]
	jr nz, .counted
	inc hl
	inc [hl]
.counted
	call ReadJoypad
	ld a, [wPadNew]
	and PAD_START
	jr z, TitleWait
	ld a, [wSeedCounter]
	ld [wSeed], a
	ld a, [wSeedCounter + 1]
	ld [wSeed + 1], a
	ret

; Reads the joypad into wPad (held) and wPadNew (pressed this frame), in the
; PAD_* layout: the D-pad in bits 4-7, the buttons in bits 0-3.
ReadJoypad::
	ld a, JOYP_GET_CTRL_PAD
	ldh [rJOYP], a
	ldh a, [rJOYP]
	ldh a, [rJOYP]
	cpl
	and JOYP_INPUTS
	swap a
	ld b, a
	ld a, JOYP_GET_BUTTONS
	ldh [rJOYP], a
	ldh a, [rJOYP]
	ldh a, [rJOYP]
	ldh a, [rJOYP]
	ldh a, [rJOYP]
	cpl
	and JOYP_INPUTS
	or b
	ld b, a
	ld a, JOYP_GET_NONE
	ldh [rJOYP], a
	ld a, [wPad]
	cpl
	and b
	ld [wPadNew], a
	ld a, b
	ld [wPad], a
	ret

; Makes floor wDepth from wSeed with the LCD on and the music playing: the
; palettes go dark, the floor generates over a few frames, the view streams
; in a row a frame, and the palettes come back. The game halts every frame
; throughout, so a floor change shows no lag frames.
NewFloor::
	ld a, DARK_PALETTE
	ld [wBgp], a
	ld [wObp], a
	call WaitFrame
	call GenerateFloor
	call CameraCenter
	call PlacePlayerSprite
	call PlaceEnemySprites
	call UpdateStatus
	ld a, [wCamY]
	ld c, a
	ld e, DRAW_H
.row
	ld a, c
	cp MAP_H
	jr nc, .drawn
	push bc
	push de
	call QueueRow
	call WaitFrame
	pop de
	pop bc
	inc c
	dec e
	jr nz, .row
.drawn
	ld a, HERO_OBP
	ld [wObp], a
	ld a, PLAY_PALETTE
	ld [wBgp], a
	ret

; ---------------------------------------------------------------------------
; VBlank: shadow OAM to OAM, the pending row or column into the background,
; the scroll registers, the frame count, then the music.

VBlank:
	push af
	push bc
	push de
	push hl
	ld a, HIGH(wShadowOAM)
	call hOAMDMA
	call FlushStream
	call FlushPatch
	call FlushStatus
	call SfxVBlank
	ld a, [wScx]
	ldh [rSCX], a
	ld a, [wScy]
	ldh [rSCY], a
	ld a, [wBgp]
	ldh [rBGP], a
	ld a, [wObp]
	ldh [rOBP0], a
	ld hl, wFrame
	inc [hl]
	jr nz, .counted
	inc hl
	inc [hl]
.counted
	call MusicVBlank
	pop hl
	pop de
	pop bc
	pop af
	reti

; Swaps the title screen for the dungeon's tiles and palettes, with the LCD
; off from one VBlank to the next, and starts the dungeon theme.
EnterDungeon:
	call WaitFrame
	di
.waitVBlank
	ldh a, [rLY]
	cp LY_VBLANK
	jr c, .waitVBlank
	xor a
	ldh [rLCDC], a
	call LoadTiles
	ld hl, TILEMAP0
	ld bc, $400
	xor a
	call FillMem
IF DEF(GBCLI_GBC)
	ldh a, [hGbc]
	and a
	call nz, ColorGbc
ENDC
	ld a, LCDC_PLAY
	ldh [rLCDC], a
	xor a
	ldh [rIF], a
	ei
	jp MusicDungeon

IF DEF(GBCLI_GBC)
; GBC mode only, with the LCD off: the dungeon's background palette, and an
; object palette each for the lantern-bearer (0) and the creatures (1).
ColorGbc:
	ld a, BGPI_AUTOINC
	ldh [rBCPS], a
	ld hl, PaletteStone
	ld b, PaletteStoneEnd - PaletteStone
.bg
	ld a, [hli]
	ldh [rBCPD], a
	dec b
	jr nz, .bg
	ld a, OBPI_AUTOINC
	ldh [rOCPS], a
	ld hl, HeroPaletteLantern
	ld b, HeroPaletteLanternEnd - HeroPaletteLantern
.obj
	ld a, [hli]
	ldh [rOCPD], a
	dec b
	jr nz, .obj
	ld hl, FoesPaletteBlight
	ld b, FoesPaletteBlightEnd - FoesPaletteBlight
.foes
	ld a, [hli]
	ldh [rOCPD], a
	dec b
	jr nz, .foes
	ld a, 1
	ldh [rVBK], a
	ld hl, ATTRMAP0
	ld bc, $800                 ; the background's and the window's
	xor a
	call FillMem
	xor a
	ldh [rVBK], a
	ret

ENDC

; ---------------------------------------------------------------------------
; Memory helpers.

; Writes BC zero bytes from HL.
ClearMem::
	xor a
; Writes BC copies of A from HL.
FillMem::
	ld d, a
.loop
	ld a, d
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
wSeedCounter:: dw               ; frames on the title screen
wSeed:: dw                      ; the run's seed: one seed, one dungeon
wDepth:: db                     ; floor number, 0 first
wPad:: db
wBgp:: db                       ; BGP and OBP0, written by the VBlank handler
wObp:: db
wPadNew:: db

IF DEF(GBCLI_GBC)
SECTION "GBC mode", HRAM
hGbc:: db
ENDC

SECTION "Shadow OAM", WRAM0, ALIGN[8]
wShadowOAM:: ds OAM_SIZE
