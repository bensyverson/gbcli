; The program: set up the screen, then wait for VBlank forever.
; `gbcli build` assembles every .asm under src/ with include/ on the
; include path, and links them into build/<project>.gb with .sym and .map.

include "hardware.inc"                ; every register, plus PAD_* (and B_PAD_*) for a joypad byte with the D-pad in bits 4-7

; ---------------------------------------------------------------------------
; Restart vectors ($0000-$003F), claimed so rgblink can't place other code
; there: it fills any free gap, $0000 included, and a stray jump to 0 would
; then run it. All 64 bytes are $FF, `rst $38`, so a jump into them lands
; in a loop at $0038 you can break on. Put an `rst` routine here by
; replacing the fill up to its address.

SECTION "Restart vectors", ROM0[$0000]
	ds $40, $FF                 ; rst $38

; ---------------------------------------------------------------------------
; Interrupt vector. VBlank is enabled below; music.asm adds the timer when
; its song wants it.

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
	; `gbcli build --gbc` defines GBCLI_GBC, so only that build carries the
	; color code, and the same ROM must still run on a DMG: so check at run
	; time. The boot ROM leaves BOOTUP_A_CGB in A on a GBC, but a GBC hands
	; A=$11 to a ROM without the header's GBC flag too, and runs it as a
	; DMG; rgbfix writes that flag ($80 at $0143) under --gbc, and a build
	; made some other way may lack it, so the check takes both. A is read
	; now, before the wait below overwrites it.
	DEF HEADER_GBC_FLAG EQU $0143
	DEF HEADER_GBC EQU $80
	cp BOOTUP_A_CGB
	ld a, 0                     ; not a GBC (ld leaves the flags alone)
	jr nz, .gbcKnown
	ld a, [HEADER_GBC_FLAG]
	and HEADER_GBC
.gbcKnown
	ldh [hGbc], a
ENDC

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

IF DEF(GBCLI_GBC)
	; A GBC ignores BGP and OBP0 in GBC mode and colors from palette RAM
	; instead. A DMG must skip this: it has no VBK, so the attribute map
	; would land on the tilemap.
	ldh a, [hGbc]
	and a
	call nz, ColorGbc
ENDC

	; LCD on, BG and sprites on, tiles from block 0, map at $9800.
	ld a, LCDC_ON | LCDC_BG_ON | LCDC_OBJ_ON | LCDC_BLOCK01 | LCDC_BG_9800
	ldh [rLCDC], a

	; Enable the VBlank interrupt, dropping any request already pending.
	ld a, IE_VBLANK
	ldh [rIE], a
	xor a
	ldh [rIF], a
	call MusicInit              ; music.asm: start the song (and the timer, if it wants it)
	; The song uses all four channels: for a sound effect, drop the music
	; calls, or mute its channel here (`gbcli help music`: hUGE_mute_channel).
	ei

MainLoop:
	; Sleep until the VBlank handler has counted a frame. Other interrupts
	; (the music's timer) wake the halt too, so check the count.
	ld a, [wFrame]
	ld b, a
.wait
	halt
	nop
	ld a, [wFrame]
	cp b
	jr z, .wait
	; Game logic goes here: it runs once per frame, after the VBlank
	; handler, and moves sprites by writing wShadowOAM, never OAM.
	jr MainLoop

; ---------------------------------------------------------------------------
; Runs at the start of every VBlank: copy shadow OAM to OAM, count the frame,
; tick the music.

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
	call MusicVBlank            ; music.asm: one tick of the song
	pop hl
	pop af
	reti

IF DEF(GBCLI_GBC)
; ---------------------------------------------------------------------------
; GBC mode only, with the LCD off: palette RAM and VRAM are open to the CPU
; then. The face's tiles serve both machines unchanged, since a palette
; colors their four shades; art drawn in color has its own GBC tiles to
; copy instead (`gbcli help images`).

ColorGbc:
	; BG palettes, then OBJ palettes, from index 0 with autoincrement.
	ld a, BGPI_AUTOINC
	ldh [rBCPS], a
	ld hl, FaceBgPalettes
	ld b, FaceBgPalettes.end - FaceBgPalettes
.bg
	ld a, [hli]
	ldh [rBCPD], a
	dec b
	jr nz, .bg
	ld a, OBPI_AUTOINC
	ldh [rOCPS], a
	ld hl, FaceObjPalettes
	ld b, FaceObjPalettes.end - FaceObjPalettes
.obj
	ld a, [hli]
	ldh [rOCPD], a
	dec b
	jr nz, .obj

	; The attribute map lives in VRAM bank 1, behind the tilemap: palette 0
	; everywhere, then the faces through BG palettes 0-3 in turn. Sprites
	; take their palette from OAM's flags instead (0 here).
	ld a, 1
	ldh [rVBK], a
	ld hl, ATTRMAP0
	ld bc, $400
	call ClearMem
	ld hl, ATTRMAP0 + 8 * 32 + 4
	ld b, 12
.faces
	ld a, b
	and %11                     ; palettes 0-3
	ld [hli], a
	dec b
	jr nz, .faces
	; Back to bank 0, where the tiles and the tilemap are written.
	xor a
	ldh [rVBK], a
	ret

; One color in palette RAM: red, green and blue from 0 to 31.
MACRO rgb
	dw (\1) | (\2) << 5 | (\3) << 10
ENDM

; Four colors a palette, color 0 first: the face draws in 0 and 3. All
; four share color 0, the background's.
FaceBgPalettes:
	rgb 31, 31, 31
	rgb 20, 24, 31
	rgb 10, 14, 28
	rgb 4, 6, 20                ; blue
	rgb 31, 31, 31
	rgb 31, 22, 20
	rgb 28, 12, 10
	rgb 22, 2, 4                ; red
	rgb 31, 31, 31
	rgb 20, 30, 20
	rgb 8, 22, 10
	rgb 2, 14, 4                ; green
	rgb 31, 31, 31
	rgb 28, 20, 30
	rgb 20, 10, 24
	rgb 14, 2, 18               ; purple
.end

; An object's color 0 is transparent, so its entry is never drawn.
FaceObjPalettes:
	rgb 31, 31, 31
	rgb 31, 28, 16
	rgb 31, 20, 4
	rgb 26, 10, 0               ; orange
.end
ENDC

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

IF DEF(GBCLI_GBC)
SECTION "GBC mode", HRAM

; Nonzero in GBC mode: on a GBC, built with `gbcli build --gbc`. Test it
; before touching VBK, palette RAM or the attribute map.
hGbc:: db
ENDC

SECTION "Shadow OAM", WRAM0, ALIGN[8]

; OAM DMA copies from a page boundary, so this is 256-byte aligned.
wShadowOAM:: ds OAM_SIZE
