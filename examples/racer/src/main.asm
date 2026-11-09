; Switchback: a pseudo-3D road racer for the Game Boy.
;
; main.asm: the header, startup, the main loop and the VBlank handler.
; road.asm draws the road (the per-line scroll tables and their HBlank
; interrupt); drive.asm moves the car along the track.

include "hardware.inc"
include "images.inc"
include "race.inc"

; ---------------------------------------------------------------------------
; Interrupt vectors. The STAT vector ($48) is in road.asm.

SECTION "VBlank vector", ROM0[INT_HANDLER_VBLANK]
VBlankVector:
	jp VBlank

; ---------------------------------------------------------------------------
; Cartridge header. The boot ROM jumps to $0100; rgbfix fills in the logo,
; the checksums and the rest of the header between $0104 and $014F.

SECTION "Header", ROM0[$0100]
	nop
	jp Start
	ds $0150 - @, 0

; ---------------------------------------------------------------------------

SECTION "Start", ROM0

Start:
	di
	ld sp, $E000                ; the stack grows down from the top of WRAM

IF DEF(GBCLI_GBC)
	; `gbcli build --gbc` defines GBCLI_GBC; the same ROM must still run on
	; a DMG, so check at run time (A = BOOTUP_A_CGB, and the header's flag).
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
	jp Title

; Turns the LCD off (in VBlank: never mid-frame on a DMG), with interrupts
; off, and clears WRAM0 (variables, shadow OAM, the RAM code, the tables).
ScreenOff:
	di
	xor a
	ldh [rIE], a
.waitVBlank
	ldh a, [rLY]
	cp LY_VBLANK
	jr c, .waitVBlank
	xor a
	ldh [rLCDC], a
	ldh [rNR52], a
	ldh [hInRace], a
	ld hl, STARTOF(WRAM0)
	ld bc, $1000
	call ClearMem
	jp CopyOAMDMA

; The title screen: its picture and song until START.
Title::
	ld sp, $E000
	call ScreenOff
	ld hl, STARTOF(VRAM)
	ld de, TitleTiles
	ld bc, TitleTilesEnd - TitleTiles
	call CopyMem
	ld hl, TILEMAP0
	ld de, TitleTilemap
	ld b, TITLE_HEIGHT
.row
	ld c, TITLE_WIDTH
.tile
	ld a, [de]
	inc de
	ld [hli], a
	dec c
	jr nz, .tile
	ld a, l
	add 32 - TITLE_WIDTH
	ld l, a
	jr nc, .rowDone
	inc h
.rowDone
	dec b
	jr nz, .row
	ld a, TITLE_BGP
	ldh [rBGP], a
	xor a
	ldh [rSCX], a
	ldh [rSCY], a
IF DEF(GBCLI_GBC)
	ldh a, [hGbc]
	and a
	call nz, ColorTitle
ENDC
	ld a, STATE_TITLE
	ld [wState], a
	ld a, LCDC_ON | LCDC_BG_ON | LCDC_BLOCK01 | LCDC_BG_9800
	ldh [rLCDC], a
	ld a, IE_VBLANK
	ldh [rIE], a
	call MusicPlayTitle
	xor a
	ldh [rIF], a
	ei
.loop
	call WaitFrame
	call ReadPad
	ldh a, [hPadNew]
	and PAD_START
	jr z, .loop
	; fall through

; The race: from the countdown to the result, then START goes back to the
; title (race.asm).
RaceStart::
	ld sp, $E000
	call ScreenOff

	; Tiles, all at $8000-$8FFF, which the BG (LCDC_BLOCK01) and objects
	; both read unsigned: the road, the font, the object sheets (race.inc).
	ld hl, STARTOF(VRAM)
	ld de, RoadTiles
	ld bc, RoadTilesEnd - RoadTiles
	call CopyMem
	ld hl, STARTOF(VRAM) + FONT_BASE * 16
	ld de, FontTiles
	ld bc, FontTilesEnd - FontTiles
	call CopyMem
	ld hl, STARTOF(VRAM) + CAR_BASE * 16
	ld de, CarTiles
	ld bc, CarTilesEnd - CarTiles
	call CopyMem
	ld hl, STARTOF(VRAM) + RIVAL_BASE * 16
	ld de, RivalTiles
	ld bc, RivalTilesEnd - RivalTiles
	call CopyMem
	ld hl, STARTOF(VRAM) + TREE_BASE * 16
	ld de, TreeTiles
	ld bc, TreeTilesEnd - TreeTiles
	call CopyMem

	; The map: 32x32 tiles, the whole 256x256 BG. The window's map ($9C00)
	; is the HUD; its first row is all that shows.
	ld hl, TILEMAP0
	ld de, RoadTilemap
	ld bc, RoadTilemapEnd - RoadTilemap
	call CopyMem
	ld hl, TILEMAP1             ; spaces: tile 0 is the road's

	ld b, 32
	ld a, FONT_BASE
.hudRow
	ld [hli], a
	dec b
	jr nz, .hudRow
	ld a, HUD_Y
	ldh [rWY], a
	ld a, 7
	ldh [rWX], a

	; Rivals share the car's shades; trees have their own.
	ASSERT RIVAL_OBP == CAR_OBP
	ld a, ROAD_BGP
	ldh [rBGP], a
	ld a, CAR_OBP
	ldh [rOBP0], a
	ld a, TREE_OBP
	ldh [rOBP1], a

IF DEF(GBCLI_GBC)
	ldh a, [hGbc]
	and a
	call nz, ColorGbc
ENDC

	call InitRace
	call CopyHud
	call InitDrive
	call InitObjects
	call InitRoad
	call SetLineZero

	; LCD on: BG, 8x16 objects, tiles from $8000, the road's map at $9800,
	; the HUD window's at $9C00.
	ld a, LCDC_ON | LCDC_BG_ON | LCDC_OBJ_ON | LCDC_OBJ_16 | LCDC_BLOCK01 | LCDC_BG_9800 | LCDC_WIN_ON | LCDC_WIN_9C00
	ldh [rLCDC], a

	; STAT raises a request on every HBlank (mode 0), but only VBlank is
	; enabled for now: from the first VBlank on, RoadVBlank and the timer
	; enable the HBlank interrupt for the road's lines each frame. Enabling
	; it here would let a request left pending during MusicInit fire at
	; `ei`, mid-line.
	ld a, STAT_MODE_0
	ldh [rSTAT], a
	ld a, IE_VBLANK
	ldh [rIE], a
	call MusicPlayRace
	call SfxInit
	ld a, 1
	ldh [hInRace], a
	xor a
	ldh [rIF], a
	ei

MainLoop:
	call WaitFrame
	call ReadPad
	call SfxUpdate
	call UpdateRace
	call UpdateDrive
	call ComputeRoad            ; the back buffer, shown from the next frame
	call UpdateObjects          ; on that road, into shadow OAM
	jr MainLoop

; Sleeps until the VBlank handler has counted a frame. Other interrupts
; (the road's HBlank and timer) wake the halt too, so check the count.
WaitFrame:
	ld a, [wFrame]
	ld b, a
.wait
	halt
	nop
	ld a, [wFrame]
	cp b
	jr z, .wait
	ret

; ---------------------------------------------------------------------------
; VBlank: in the race, swap in the road buffer ComputeRoad finished, set line 0's scroll,
; copy shadow OAM, count the frame; then, with interrupts back on so the
; road's HBlank interrupts keep time, tick the music.

VBlank:
	push af
	push bc
	push de
	push hl
	ldh a, [hInRace]
	and a
	jr z, .title
	call RoadVBlank
	ld a, HIGH(wShadowOAM)
	call hOAMDMA
	call CopyHud
.title
	ld hl, wFrame
	inc [hl]
	jr nz, .counted
	inc hl
	inc [hl]
.counted
	ei
	call MusicVBlank
	pop hl
	pop de
	pop bc
	pop af
	reti

; ---------------------------------------------------------------------------
; The joypad: hPad holds the buttons down now, hPadNew those pressed since
; the last read (PAD_* bits: the D-pad in bits 4-7).

ReadPad:
	ld a, JOYP_GET_CTRL_PAD
	ldh [rJOYP], a
	ldh a, [rJOYP]
	ldh a, [rJOYP]
	cpl
	and $0F
	swap a
	ld b, a
	ld a, JOYP_GET_BUTTONS
	ldh [rJOYP], a
	ldh a, [rJOYP]
	ldh a, [rJOYP]
	ldh a, [rJOYP]
	ldh a, [rJOYP]
	cpl
	and $0F
	or b
	ld b, a
	ldh a, [hPad]
	cpl
	and b
	ldh [hPadNew], a
	ld a, b
	ldh [hPad], a
	ld a, JOYP_GET_NONE
	ldh [rJOYP], a
	ret

IF DEF(GBCLI_GBC)
; ---------------------------------------------------------------------------
; GBC mode only, with the LCD off: the title's palette and attribute map
; (20 entries a row of the 32-wide map).

ColorTitle:
	ld a, BGPI_AUTOINC
	ldh [rBCPS], a
	ld hl, TitlePalettes
	ld b, TitlePalettesEnd - TitlePalettes
.bg
	ld a, [hli]
	ldh [rBCPD], a
	dec b
	jr nz, .bg
	ld a, 1
	ldh [rVBK], a
	ld hl, ATTRMAP0
	ld de, TitleAttrmap
	ld b, TITLE_HEIGHT
.row
	ld c, TITLE_WIDTH
.tile
	ld a, [de]
	inc de
	ld [hli], a
	dec c
	jr nz, .tile
	ld a, l
	add 32 - TITLE_WIDTH
	ld l, a
	jr nc, .rowDone
	inc h
.rowDone
	dec b
	jr nz, .row
	xor a
	ldh [rVBK], a               ; indexed art: its tiles serve both machines
	ret

; ---------------------------------------------------------------------------
; GBC mode only, with the LCD off: the road's palettes, attribute map and
; GBC tiles, the HUD font's palette (BG slot ROAD_PALETTES) and GBC tiles,
; and the object sheets' palettes and GBC tiles.

ColorGbc:
	ld a, BGPI_AUTOINC
	ldh [rBCPS], a
	ld hl, RoadPalettes
	ld b, RoadPalettesEnd - RoadPalettes
.bg
	ld a, [hli]
	ldh [rBCPD], a
	dec b
	jr nz, .bg
	; The font's palette follows the road's (autoincrement carries on).
	ASSERT FONT_PALETTES == 1
	ld hl, FontPalettes
	ld b, FontPalettesEnd - FontPalettes
.font
	ld a, [hli]
	ldh [rBCPD], a
	dec b
	jr nz, .font
	ld a, OBPI_AUTOINC
	ldh [rOCPS], a
	ld hl, ObjPalettesGbc
	ld b, ObjPalettesGbcEnd - ObjPalettesGbc
.obj
	ld a, [hli]
	ldh [rOCPD], a
	dec b
	jr nz, .obj

	ld a, 1
	ldh [rVBK], a
	ld hl, ATTRMAP0
	ld de, RoadAttrmap
	ld bc, RoadAttrmapEnd - RoadAttrmap
	call CopyMem
	ld hl, ATTRMAP1             ; the HUD row: the font's palette
	ld b, 32
	ld a, ROAD_PALETTES
.hud
	ld [hli], a
	dec b
	jr nz, .hud
	xor a
	ldh [rVBK], a
	ld hl, STARTOF(VRAM) + FONT_BASE * 16
	ld de, FontTilesGbc
	ld bc, FontTilesGbcEnd - FontTilesGbc
	call CopyMem
	; The road is indexed art: its tiles serve both machines. The sheets
	; are color art, with GBC tiles of their own.
	ld hl, STARTOF(VRAM) + CAR_BASE * 16
	ld de, CarTilesGbc
	ld bc, CarTilesGbcEnd - CarTilesGbc
	call CopyMem
	ld hl, STARTOF(VRAM) + RIVAL_BASE * 16
	ld de, RivalTilesGbc
	ld bc, RivalTilesGbcEnd - RivalTilesGbc
	call CopyMem
	ld hl, STARTOF(VRAM) + TREE_BASE * 16
	ld de, TreeTilesGbc
	ld bc, TreeTilesGbcEnd - TreeTilesGbc
	jp CopyMem

ENDC

; ---------------------------------------------------------------------------
; Memory helpers.

; Writes BC zero bytes from HL.
ClearMem::
	xor a
	ld [hli], a
	dec bc
	ld a, b
	or c
	jr nz, ClearMem
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

; Counts VBlanks since the LCD came on: `gbcli run --until 'u16[wFrame] >= 60'`.
wFrame:: dw

SECTION "Mode", HRAM
hInRace: db                     ; nonzero while the race's screen is up

SECTION "Pad", HRAM
hPad:: db
hPadNew:: db

IF DEF(GBCLI_GBC)
SECTION "GBC mode", HRAM
hGbc:: db
ENDC

SECTION "Shadow OAM", WRAM0, ALIGN[8]

; OAM DMA copies from a page boundary, so this is 256-byte aligned.
wShadowOAM:: ds OAM_SIZE
