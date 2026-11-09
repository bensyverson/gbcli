; Vesper Swarm: the header, start-up, the VBlank handler and the main loop.
; `gbcli build` assembles every .asm under src/ with include/ on the
; include path, and links them into build/shooter.gb with .sym and .map.
;
; Each frame: VBlank copies shadow OAM to OAM and resets the parallax
; splits; the main loop then moves the scene, and the multiplexer
; (sprites.asm) packs the frame's sprite list into shadow OAM, rotated so
; that sprites past the hardware's limits flicker instead of vanishing.

include "hardware.inc"

; ---------------------------------------------------------------------------
; Interrupt vector. VBlank and STAT (the parallax splits, parallax.asm) are
; enabled below; music.asm adds the timer when its song wants it.

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
	; a DMG, so check at run time (see the starter's notes in README.md).
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
	ldh [rNR52], a              ; silence the APU; the boot ROM leaves it on

	ld [wFrame], a
	ld [wFrame + 1], a
	ld [wFrameDone], a
	ld hl, wShadowOAM
	ld b, OAM_SIZE
.clearOAM
	ld [hli], a
	dec b
	jr nz, .clearOAM

	call CopyOAMDMA
	call SprInit
	call MusicInit              ; sound on; each screen starts its song
	call ParallaxInit           ; tiles, maps and the split tables
	call GameInit               ; font, sprites, the title screen


	ld a, LCDC_ON | LCDC_BG_ON | LCDC_OBJ_ON | LCDC_OBJ_8 | LCDC_BLOCK01 | LCDC_BG_9800 | LCDC_WIN_ON | LCDC_WIN_9C00
	ldh [rLCDC], a

	ld a, IE_VBLANK | IE_STAT
	ldh [rIE], a
	xor a
	ldh [rIF], a
	ei

MainLoop:
	; Sleep until VBlank has counted a frame; the STAT splits and the
	; music's timer also wake the halt, so check the flag.
.wait
	halt
	nop
	ld a, [wFrameDone]
	and a
	jr z, .wait
	xor a
	ld [wFrameDone], a

	call ReadPad
	call ParallaxUpdate
	call SprBegin
	call GameUpdate             ; the state's frame: adds this frame's sprites
	call SprFlush               ; packs the list into shadow OAM
	jr MainLoop

; ---------------------------------------------------------------------------
; Runs at the start of every VBlank: shadow OAM to OAM, the parallax reset
; for the next frame, the frame count, the music.

VBlank:
	push af
	push hl
	ld a, HIGH(wShadowOAM)
	call hOAMDMA
	call ParallaxVBlank
	call HudVBlank
	ld hl, wFrame
	inc [hl]
	jr nz, .counted
	inc hl
	inc [hl]
.counted
	ld a, 1
	ld [wFrameDone], a
	call MusicVBlank
	pop hl
	pop af
	reti


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

; Counts VBlanks since the LCD came on: `gbcli run --until 'u16[wFrame] >= 60'`.
wFrame:: dw
; Set by VBlank, cleared by the main loop: one game frame per VBlank.
wFrameDone:: db

IF DEF(GBCLI_GBC)
SECTION "GBC mode", HRAM
hGbc:: db
ENDC

SECTION "Shadow OAM", WRAM0, ALIGN[8]

; OAM DMA copies from a page boundary, so this is 256-byte aligned.
wShadowOAM:: ds OAM_SIZE
