; Per-pixel layer-identity test ROM for gbcli (DMG). Written for this project;
; same license as gbcli.
;
; A scene whose layers are known exactly, so the backend test can pin specific
; pixels. Each layer has its own shade (identity palettes), so a test can also
; check the whole layer map against the pixels:
;   shade 0 or 2: background    shade 1: window    shade 3: an object
;
;   tile 0: color 0          (background where nothing else is)
;   tile 1: color 2          (background behind the BG-priority object 2)
;   tile 2: color 3          (opaque objects)
;   tile 3: color 3, but the leftmost pixel of row 0 is color 0 (object 1)
;   tile 4: color 3          (object 2; BG-priority flag set)
;   tile 5: color 1          (window)
;   $C000: 0 = quiet loop (the PPU may batch a whole line)
;          1 = VRAM read loop (forces the slow, per-pixel PPU path)
;
; Expected on lines 0-7 (objects 0-3, OAM Y = 16):
;   x=8-15   object 0 (opaque over background)
;   x=32     background (object 1's transparent pixel, row 0 only)
;   x=33-39  object 1
;   x=56-63  background (object 2 has BG priority and the BG is color 2)
;   x=72-79  object 3 (BG priority, but the BG there is color 0: the object wins)
;   x=89-    window (WX=96 starts the window at x=WX-7)
; Expected on lines 8-15 (objects 4-7, OAM Y = 24), DMG priority by X then index:
;   x=16-23  object 5 (X 16 beats object 4's X 20 where they overlap, x=20-23)
;   x=24-27  object 4
;   x=40-47  object 6 (objects 6 and 7 share X; the lower index wins)
; Line 100: an LY=LYC interrupt clears LCDC.5 partway through the window, so the
; line is background, window from x=89, then background again; lines 101-143
; have no window. The vblank interrupt sets LCDC.5 again. Both handlers change
; only that bit, so a test may write the others (LCDC.1, objects) from outside.

SECTION "VBlank", ROM0[$40]
    jp VBlank
SECTION "Stat", ROM0[$48]
    jp Stat

SECTION "Header", ROM0[$100]
    nop
    jp Main
    ds $150 - @, 0  ; header; rgbfix fills in the logo and checksums

SECTION "Main", ROM0[$150]
Main:
    di
    ld sp, $FFFE

    ; Tile 0: all color 0.
    ld hl, $8000
    ld b, 16
    xor a
.tile0:
    ld [hl+], a
    dec b
    jr nz, .tile0

    ; Tile 1: all color 2 (low plane 0, high plane 1).
    ld hl, $8010
    ld b, 8
.tile1:
    xor a
    ld [hl+], a
    ld a, $FF
    ld [hl+], a
    dec b
    jr nz, .tile1

    ; Tiles 2 and 4: all color 3.
    ld hl, $8020
    ld b, 16
    ld a, $FF
.tile2:
    ld [hl+], a
    dec b
    jr nz, .tile2
    ld hl, $8040
    ld b, 16
.tile4:
    ld [hl+], a
    dec b
    jr nz, .tile4

    ; Tile 3: all color 3, except the leftmost pixel of row 0 (color 0).
    ld hl, $8030
    ld b, 16
.tile3:
    ld [hl+], a
    dec b
    jr nz, .tile3
    ld a, $7F
    ld [$8030], a       ; lower row 0: bit 7 = 0
    ld [$8031], a       ; upper row 0: bit 7 = 0

    ; Tile 5: all color 1.
    ld hl, $8050
    ld b, 8
.tile5:
    ld a, $FF
    ld [hl+], a
    xor a
    ld [hl+], a
    dec b
    jr nz, .tile5

    ; Background map: tile 0 everywhere, tile 1 in the cell under object 2.
    ld hl, $9800
    ld bc, 1024
.bgmap:
    xor a
    ld [hl+], a
    dec bc
    ld a, b
    or c
    jr nz, .bgmap
    ld a, 1
    ld [$9807], a       ; row 0, column 7: x = 56

    ; Window map: tile 5 everywhere.
    ld hl, $9C00
    ld bc, 1024
.winmap:
    ld a, 5
    ld [hl+], a
    dec bc
    ld a, b
    or c
    jr nz, .winmap

    ; OAM: clear, then copy the objects.
    ld hl, $FE00
    ld b, 160
    xor a
.oam:
    ld [hl+], a
    dec b
    jr nz, .oam
    ld hl, Objects
    ld de, $FE00
    ld b, Objects.end - Objects
.copy:
    ld a, [hl+]
    ld [de], a
    inc de
    dec b
    jr nz, .copy

    ; Palettes (identity) and scroll.
    ld a, $E4
    ldh [$FF47], a      ; BGP
    ldh [$FF48], a      ; OBP0
    ldh [$FF49], a      ; OBP1
    xor a
    ldh [$FF42], a      ; SCY
    ldh [$FF43], a      ; SCX
    ldh [$FF4A], a      ; WY = 0
    ld a, 96
    ldh [$FF4B], a      ; WX = 96 -> the window starts at x = 89

    ; Interrupts: vblank, and STAT on LY = LYC = 100.
    ld a, 100
    ldh [$FF45], a      ; LYC
    ld a, $40
    ldh [$FF41], a      ; STAT: LY = LYC interrupt only
    ld a, $03
    ldh [$FFFF], a      ; IE: vblank and STAT
    xor a
    ldh [$FF0F], a      ; IF

    ; LCD on, window on with its map at $9C00, BG and objects on, tiles at $8000.
    ld a, $F3
    ldh [$FF40], a      ; LCDC
    ei

.loop:
    ld a, [$C000]
    and a
    jr nz, .busy
    jr .loop
.busy:
    ld a, [$8000]       ; VRAM read: syncs the PPU, so mode 3 can't batch
    jr .loop

; Line 100: wait into mode 3, past the window's start, then turn the window off.
Stat:
    push af
    push bc
    ld b, 7
.wait:
    dec b
    jr nz, .wait
    ldh a, [$FF40]
    and $DF
    ldh [$FF40], a      ; LCDC.5 off, mid-line
    pop bc
    pop af
    reti

VBlank:
    push af
    ldh a, [$FF40]
    or $20
    ldh [$FF40], a      ; LCDC.5 on for the next frame
    pop af
    reti

Objects:                ; y, x, tile, flags
    db 16, 16, 2, $00   ; 0: x = 8
    db 16, 40, 3, $00   ; 1: x = 32, leftmost pixel of row 0 transparent
    db 16, 64, 4, $80   ; 2: x = 56, BG priority over BG color 2
    db 16, 80, 2, $80   ; 3: x = 72, BG priority over BG color 0
    db 24, 28, 2, $00   ; 4: x = 20, lines 8-15
    db 24, 24, 2, $00   ; 5: x = 16, overlaps object 4 with the lower X
    db 24, 48, 2, $00   ; 6: x = 40
    db 24, 48, 2, $00   ; 7: x = 40, same X as object 6
.end:
