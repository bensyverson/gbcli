; Per-pixel layer-identity test ROM for gbcli. Written for this project; same
; license as gbcli.
;
; A static scene whose layers are known exactly, so the backend test can pin
; specific pixels. All OAM Y values are 16, so everything sits on line 0.
;   tile 0: color 0          (background where nothing else is)
;   tile 1: color 1          (background behind the BG-priority object)
;   tile 2: color 3          (object 0, opaque)
;   tile 3: color 0 then 3   (object 1; its leftmost pixel is transparent)
;   tile 4: color 3          (object 2; BG-priority flag set)
;   tile 5: color 1          (window)
;   $C000: 0 = quiet loop (the PPU may batch a whole line)
;          1 = VRAM read loop (forces the slow, per-pixel PPU path)
; Expected on line 0:
;   x=8   object 0 (opaque over background)
;   x=32  background (transparent object pixel)
;   x=33  object 1
;   x=56  background (object 2 hidden by BG priority)
;   x=89  window (WX=96 starts the window at x=WX-7)

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

    ; Tile 1: all color 1.
    ld hl, $8010
    ld b, 8
.tile1:
    ld a, $FF
    ld [hl+], a
    xor a
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

    ; OAM: clear, then the three objects.
    ld hl, $FE00
    ld b, 160
    xor a
.oam:
    ld [hl+], a
    dec b
    jr nz, .oam
    ld a, 16
    ld [$FE00], a       ; object 0: y
    ld a, 16
    ld [$FE01], a       ; x = 16 -> screen x = 8
    ld a, 2
    ld [$FE02], a       ; tile 2
    ld a, 16
    ld [$FE04], a       ; object 1: y
    ld a, 40
    ld [$FE05], a       ; x = 40 -> screen x = 32
    ld a, 3
    ld [$FE06], a       ; tile 3
    ld a, 16
    ld [$FE08], a       ; object 2: y
    ld a, 64
    ld [$FE09], a       ; x = 64 -> screen x = 56
    ld a, 4
    ld [$FE0A], a       ; tile 4
    ld a, $80
    ld [$FE0B], a       ; BG priority: the BG color 1 hides it

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

    ; LCD on, window on with its map at $9C00, BG and objects on, tiles at $8000.
    ld a, $F3
    ldh [$FF40], a      ; LCDC

.loop:
    ld a, [$C000]
    and a
    jr nz, .busy
    jr .loop
.busy:
    ld a, [$8000]       ; VRAM read: syncs the PPU, so mode 3 can't batch
    jr .loop
