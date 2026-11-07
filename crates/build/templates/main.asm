; The program: set up the screen, then wait for VBlank forever.
; `gbcli build` assembles every .asm under src/ with include/ on the
; include path, and links them into build/<project>.gb with .sym and .map.

include "hardware.inc"

; ---------------------------------------------------------------------------
; Interrupt vector. Only VBlank is enabled (IE_VBLANK below).

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

	; Turn the LCD off, but only during VBlank: turning it off mid-frame
	; can damage real DMG hardware.
.waitVBlank
	ldh a, [rLY]
	cp LY_VBLANK
	jr c, .waitVBlank
	xor a
	ldh [rLCDC], a

	; Silence the APU; the boot ROM leaves it on.
	ldh [rNR52], a

	; WRAM and HRAM hold garbage at power-on: clear every variable we read.
	ld [wFrame], a
	ld [wFrame + 1], a
	ld hl, wShadowOAM
	ld b, OAM_SIZE
.clearOAM
	ld [hli], a
	dec b
	jr nz, .clearOAM

	call CopyOAMDMA

	; Tiles: tile 0 stays blank, tile 1 is a face. Both live in block 0
	; ($8000), which sprites always use and the BG uses with LCDC_BLOCK01.
	ld hl, TILEBLOCK0
	ld bc, 16
	call ClearMem
	ld de, FaceTile
	ld bc, FaceTile.end - FaceTile
	call CopyMem

	; Background: tile 0 everywhere, then a row of faces across the middle.
	ld hl, TILEMAP0
	ld bc, $400
	call ClearMem
	ld hl, TILEMAP0 + 8 * 32 + 4
	ld b, 12
	ld a, 1
.faces
	ld [hli], a
	dec b
	jr nz, .faces

	; One sprite: the face, near the top. Shadow OAM is copied to OAM by
	; the VBlank handler, so the program only ever writes wShadowOAM.
	ld hl, wShadowOAM
	ld a, 32 + OAM_Y_OFS
	ld [hli], a                 ; Y
	ld a, 76 + OAM_X_OFS
	ld [hli], a                 ; X
	ld a, 1
	ld [hli], a                 ; tile
	xor a
	ld [hli], a                 ; flags

	; Palettes: darkest shade for color 3, lightest for color 0.
	ld a, %11_10_01_00
	ldh [rBGP], a
	ldh [rOBP0], a
	xor a
	ldh [rSCX], a
	ldh [rSCY], a

	; LCD on, BG and sprites on, tiles from block 0, map at $9800.
	ld a, LCDC_ON | LCDC_BG_ON | LCDC_OBJ_ON | LCDC_BLOCK01 | LCDC_BG_9800
	ldh [rLCDC], a

	; Enable only the VBlank interrupt, dropping any request already pending.
	ld a, IE_VBLANK
	ldh [rIE], a
	xor a
	ldh [rIF], a
	ei

MainLoop:
	halt                        ; sleep until the next interrupt
	nop
	; Game logic goes here: it runs once per frame, after the VBlank
	; handler, and moves sprites by writing wShadowOAM, never OAM.
	jr MainLoop

; ---------------------------------------------------------------------------
; Runs at the start of every VBlank: copy shadow OAM to OAM, count the frame.

VBlank:
	push af
	push hl
	ld a, HIGH(wShadowOAM)
	call hOAMDMA
	ld hl, wFrame
	inc [hl]
	jr nz, .done
	inc hl
	inc [hl]
.done
	pop hl
	pop af
	reti

; ---------------------------------------------------------------------------
; Memory helpers.

; Writes BC zero bytes from HL.
ClearMem:
	xor a
	ld [hli], a
	dec bc
	ld a, b
	or c
	jr nz, ClearMem
	ret

; Copies BC bytes from DE to HL.
CopyMem:
	ld a, [de]
	ld [hli], a
	inc de
	dec bc
	ld a, b
	or c
	jr nz, CopyMem
	ret

FaceTile:
	dw `00333300
	dw `03000030
	dw `30300303
	dw `30000003
	dw `30300303
	dw `30033003
	dw `03000030
	dw `00333300
.end

; ---------------------------------------------------------------------------

SECTION "Variables", WRAM0

; Counts VBlanks since the LCD came on: `gbcli run --until 'u16[wFrame] >= 60'`.
wFrame:: dw

SECTION "Shadow OAM", WRAM0, ALIGN[8]

; OAM DMA copies from a page boundary, so this is 256-byte aligned.
wShadowOAM:: ds OAM_SIZE
