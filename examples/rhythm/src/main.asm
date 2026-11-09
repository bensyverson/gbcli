; Lantern Drum: notes fall down four lanes toward a hit line, in time with
; the song; press a lane's button (LEFT, RIGHT, B, A) as its note reaches
; the line. Startup, the frame loop, the VBlank handler and input live here;
; play.asm judges and draws, results.asm grades, music.asm keeps the clock.

include "hardware.inc"
include "game.inc"
include "images.inc"

; The restart vectors are unused: claim them, so the linker can't float a
; ROM0 section to $0000, where a stray `rst $00` or jump to 0 would run it.
; $FF is `rst $38`, which traps at $38.
SECTION "RST vectors", ROM0[$0000]
	ds INT_HANDLER_VBLANK, $FF

SECTION "VBlank vector", ROM0[INT_HANDLER_VBLANK]
VBlankVector:
	jp VBlank

SECTION "Header", ROM0[$0100]
	nop
	jp Start
	ds $0150 - @, 0

; ---------------------------------------------------------------------------

SECTION "Start", ROM0

Start:
IF DEF(GBCLI_GBC)
	; The same ROM runs on both machines: hGbc says which. The boot ROM
	; leaves BOOTUP_A_CGB in A on a GBC, which also hands A=$11 to a ROM
	; without the header's GBC flag and runs it as a DMG, so check both.
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
	; fall through

; Sets up the machine once: tiles, palettes, interrupts; then the title.
Init:
	di
	ld sp, $E000
	call LcdOffSpin
	xor a
	ldh [rNR52], a

	ld hl, STARTOF("Variables")
	ld bc, SIZEOF("Variables")
	call ClearMem
	ld hl, wShadowOAM
	ld bc, OAM_SIZE
	call ClearMem
	call CopyOAMDMA

	ld hl, TILEBLOCK0
	ld de, PlayTiles
	ld bc, PlayTilesEnd - PlayTiles
	call CopyMem
	call LoadFont
	ld hl, TILEBLOCK1
	ld de, TitleTiles
	ld bc, TitleTilesEnd - TitleTiles
	call CopyMem

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

	ld a, IE_VBLANK
	ldh [rIE], a
	xor a
	ldh [rIF], a
	ei
	call ShowTitle

MainLoop:
	ld a, [wFrame]
	ld b, a
.wait
	halt
	nop
	ld a, [wFrame]
	cp b
	jr z, .wait

	call ReadPad
	ld a, [wState]
	cp STATE_PLAY
	jr nz, .menu

	call PlayFrame
	ld a, [wLife]
	and a
	jr z, .failed
	ld a, [wSongOver]
	and a
	jr z, MainLoop
	ld a, [wHead]
	cp CHART_LEN
	jr c, MainLoop
	ld a, 1                     ; cleared
	call ShowResults
	jr MainLoop
.failed
	call MusicStop
	xor a
	call ShowResults
	jr MainLoop

	; The title and the results wait for START: the title starts the song,
	; the results go back to the title.
.menu
	ld b, a
	ld a, [wPressed]
	and PAD_START
	jr z, MainLoop
	ld a, b
	cp STATE_TITLE
	jr z, .play
	call ShowTitle
	jr MainLoop
.play
	call StartPlay
	jr MainLoop

; The title screen, with its music. Leaves the LCD on.
ShowTitle:
	call LcdOff
	ld hl, wShadowOAM
	call HideFrom
	ld hl, TILEMAP0
	ld bc, $400
	call ClearMem
	; The title's map, each byte moved up to TITLE_BASE.
	ld de, TitleTilemap
	ld hl, TILEMAP0
	ld c, TITLE_HEIGHT
.row
	ld b, TITLE_WIDTH
.col
	ld a, [de]
	add TITLE_BASE
	ld [hli], a
	inc de
	dec b
	jr nz, .col
	ld a, l
	add TILEMAP_WIDTH - TITLE_WIDTH
	ld l, a
	adc h
	sub l
	ld h, a
	dec c
	jr nz, .row
	ld hl, TILEMAP0 + 15 * 32 + 4
	ld de, StrPressStart
	call PrintStr
	ld a, STATE_TITLE
	ld [wState], a
	call MusicPlayTitle
	jp LcdOn

; A new play: the playfield, a fresh score, the song from its preroll.
StartPlay:
	call LcdOff
	call DrawPlayfield
	call PlayInit
	ld a, STATE_PLAY
	ld [wState], a
	call MusicPlayGame
	jp LcdOn

StrPressStart: db "PRESS START", 0

; ---------------------------------------------------------------------------
; VBlank: copy shadow OAM and the HUD, count the frame, tick the clock.

VBlank:
	push af
	push bc
	push de
	push hl
	ld a, HIGH(wShadowOAM)
	call hOAMDMA
	ld a, [wState]
	cp STATE_PLAY
	call z, CopyHud
	ld hl, wFrame
	inc [hl]
	jr nz, .done
	inc hl
	inc [hl]
.done
	call MusicVBlank
	pop hl
	pop de
	pop bc
	pop af
	reti

; ---------------------------------------------------------------------------
; The LCD.

; Sleeps until the VBlank handler has run (interrupts on), so a screen
; change that follows starts in VBlank instead of spinning a frame on LY.
WaitVBlank::
	ld a, [wFrame]
	ld b, a
.wait
	halt
	nop
	ld a, [wFrame]
	cp b
	jr z, .wait
	ret

; Turns the LCD off in VBlank (never mid-frame: that can damage a DMG).
; Interrupts on: it first sleeps to the VBlank handler, so a screen change
; doesn't spin most of a frame on LY (a lag frame).
LcdOff::
	ldh a, [rLCDC]
	bit B_LCDC_ENABLE, a
	ret z
	call WaitVBlank
	; fall through

; The same with interrupts off (at startup): waits on LY for VBlank.
LcdOffSpin:
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

; LCD on: BG and 8x8 sprites, tiles from block 0, map at $9800.
LcdOn::
	ld a, LCDC_ON | LCDC_BG_ON | LCDC_OBJ_ON | LCDC_BLOCK01 | LCDC_BG_9800
	ldh [rLCDC], a
	ret

; ---------------------------------------------------------------------------
; Input.

; Reads the joypad into wPad (1 = held; D-pad in bits 4-7) and the buttons
; newly pressed since last frame into wPressed.
ReadPad:
	ld a, JOYP_GET_CTRL_PAD
	ldh [rP1], a
	ldh a, [rP1]
	ldh a, [rP1]
	cpl
	and $0F
	swap a
	ld b, a
	ld a, JOYP_GET_BUTTONS
	ldh [rP1], a
	ldh a, [rP1]
	ldh a, [rP1]
	ldh a, [rP1]
	ldh a, [rP1]
	ldh a, [rP1]
	cpl
	and $0F
	or b
	ld b, a
	ld a, JOYP_GET_NONE
	ldh [rP1], a
	ld a, [wPad]
	cpl
	and b
	ld [wPressed], a
	ld a, b
	ld [wPad], a
	ret

; ---------------------------------------------------------------------------
; The playfield.

; The lanes, the hit line and the HUD's labels, on a clear map. LCD off.
DrawPlayfield:
	ld hl, TILEMAP0
	ld bc, $400
	call ClearMem
	ld c, 0                     ; lane
.lane
	ld hl, LaneX
	ld a, c
	add l
	ld l, a
	adc h
	sub l
	ld h, a
	ld a, [hl]
	sub OAM_X_OFS
	srl a
	srl a
	srl a                       ; tile column
	ld hl, TILEMAP0
	add l
	ld l, a
	adc h
	sub l
	ld h, a
	ld b, SCREEN_HEIGHT
	ld de, TILEMAP_WIDTH
.row
	ld a, b
	cp SCREEN_HEIGHT - HIT_Y / 8
	ld a, TILE_LANE
	jr nz, .put
	ld a, TILE_DRUM
.put
	ld [hl], a
	add hl, de
	dec b
	jr nz, .row
	inc c
	ld a, c
	cp 4
	jr nz, .lane
	ld hl, TILEMAP0 + 0 * 32
	ld de, StrScore
	call PrintStr
	ld hl, TILEMAP0 + 3 * 32
	ld de, StrCombo
	call PrintStr
	ld hl, TILEMAP0 + 8 * 32
	ld de, StrLife
	jp PrintStr

StrScore: db "SCORE", 0
StrCombo: db "COMBO", 0
StrLife:  db "LIFE", 0

IF DEF(GBCLI_GBC)
; GBC mode only, LCD off: palette RAM, and palette 0 in the attribute map.
ColorGbc:
	ld a, BGPI_AUTOINC
	ldh [rBCPS], a
	ld hl, BgPalette
	ld b, 8
.bg
	ld a, [hli]
	ldh [rBCPD], a
	dec b
	jr nz, .bg
	ld a, OBPI_AUTOINC
	ldh [rOCPS], a
	ld hl, ObjPalettes
	ld b, ObjPalettes.end - ObjPalettes
.obj
	ld a, [hli]
	ldh [rOCPD], a
	dec b
	jr nz, .obj
	ld a, 1
	ldh [rVBK], a
	ld hl, ATTRMAP0
	ld bc, $400
	call ClearMem
	xor a
	ldh [rVBK], a
	ret

MACRO rgb
	dw (\1) | (\2) << 5 | (\3) << 10
ENDM

BgPalette:
	rgb 4, 4, 10
	rgb 10, 10, 18
	rgb 20, 18, 26
	rgb 31, 28, 20
; One a lane (its lanterns), then the bursts'. Color 0 is never drawn.
ObjPalettes:
	rgb 0, 0, 0
	rgb 31, 28, 12
	rgb 31, 16, 4
	rgb 18, 4, 2                ; amber
	rgb 0, 0, 0
	rgb 16, 31, 28
	rgb 4, 22, 22
	rgb 2, 10, 14               ; teal
	rgb 0, 0, 0
	rgb 31, 22, 28
	rgb 28, 8, 18
	rgb 14, 2, 10               ; rose
	rgb 0, 0, 0
	rgb 22, 31, 14
	rgb 10, 24, 6
	rgb 4, 12, 2                ; leaf
	rgb 0, 0, 0
	rgb 31, 31, 31
	rgb 31, 31, 16
	rgb 31, 24, 8               ; burst
.end
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

wFrame:: dw
wPad:: db
wPressed:: db
wState:: db

IF DEF(GBCLI_GBC)
SECTION "GBC mode", HRAM
hGbc:: db
ENDC

SECTION "Shadow OAM", WRAM0, ALIGN[8]
wShadowOAM:: ds OAM_SIZE
