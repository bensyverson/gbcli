; Per-pixel layer-identity test ROM for gbcli (CGB priorities). Written for this
; project; same license as gbcli.
;
; A static scene for the CGB's own priority rules; no window. Palettes are left
; alone: the test reads layer tags, not colors.
;   tile 0: color 0          tile 1: color 2          tile 2: color 3 (objects)
; Background map: tile 0, except tile 1 at x=16-23 and x=48-55 (row 0).
; BG attributes: 0, except the priority bit (7) at x=16-23 (over color 2) and
; at x=32-39 (over color 0).
;
; Expected on lines 0-7 with LCDC.0 on:
;   x=16-23  background (the BG attribute priority bit hides object 0)
;   x=32-39  object 1 (attribute priority, but the BG is color 0)
;   x=48-55  background (object 2's own BG-priority flag, BG color 2)
; With LCDC.0 off (CGB master priority), objects 0, 1 and 2 all win.
; Expected on lines 8-15, CGB priority by OAM index alone:
;   x=20-23  object 4
;   x=24-31  object 3 (it beats object 4, whose X is lower, where they overlap)

SECTION "Header", ROM0[$100]
    nop
    jp Main
    ds $143 - @, 0
    db $80              ; CGB flag: CGB features, DMG compatible
    ds $150 - @, 0      ; rgbfix fills in the logo and checksums

SECTION "Main", ROM0[$150]
Main:
    di
    ld sp, $FFFE

    ; Some boot ROMs (SameBoy's fast CGB boot) hand over with the LCD on, and
    ; VRAM and OAM writes in modes 2 and 3 are dropped: turn it off, which is
    ; only allowed in vblank. LY reads 0 while the LCD is off, so check first.
    ldh a, [$FF40]      ; LCDC
    bit 7, a
    jr z, .lcdOff
.waitVBlank:
    ldh a, [$FF44]      ; LY
    cp 144
    jr c, .waitVBlank
    xor a
    ldh [$FF40], a      ; LCDC: LCD off
.lcdOff:

    xor a
    ldh [$FF4F], a      ; VBK: bank 0

    ; Tile 0: all color 0.
    ld hl, $8000
    ld b, 16
.tile0:
    ld [hl+], a
    dec b
    jr nz, .tile0

    ; Tile 1: all color 2.
    ld b, 8
.tile1:
    xor a
    ld [hl+], a
    ld a, $FF
    ld [hl+], a
    dec b
    jr nz, .tile1

    ; Tile 2: all color 3.
    ld b, 16
.tile2:
    ld [hl+], a
    dec b
    jr nz, .tile2

    ; Background map, bank 0: tile 0 everywhere, tile 1 in two cells.
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
    ld [$9802], a       ; row 0, column 2: x = 16
    ld [$9806], a       ; row 0, column 6: x = 48

    ; Attributes, bank 1: zero, then the priority bit in two cells.
    ld a, 1
    ldh [$FF4F], a      ; VBK: bank 1
    ld hl, $9800
    ld bc, 1024
.attrs:
    xor a
    ld [hl+], a
    dec bc
    ld a, b
    or c
    jr nz, .attrs
    ld a, $80
    ld [$9802], a       ; x = 16: priority over color 2
    ld [$9804], a       ; x = 32: priority over color 0
    xor a
    ldh [$FF4F], a      ; VBK: bank 0

    ; OAM: clear, then copy the objects.
    ld hl, $FE00
    ld b, 160
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

    xor a
    ldh [$FF42], a      ; SCY
    ldh [$FF43], a      ; SCX

    ; LCD on, BG and objects on, tiles at $8000, no window.
    ld a, $93
    ldh [$FF40], a      ; LCDC

.loop:
    jr .loop

Objects:                ; y, x, tile, flags
    db 16, 24, 2, $00   ; 0: x = 16, under an attribute-priority BG cell
    db 16, 40, 2, $00   ; 1: x = 32, attribute priority over color 0
    db 16, 56, 2, $80   ; 2: x = 48, its own BG-priority flag
    db 24, 32, 2, $00   ; 3: x = 24, lines 8-15
    db 24, 28, 2, $00   ; 4: x = 20, lower X but higher index than object 3
.end:
