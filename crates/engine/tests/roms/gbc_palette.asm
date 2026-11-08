; A dual-mode ($80) test ROM for gbcli: the whole screen in BG color 0, with
; BG palette 0's color 0 set to orange ($021F: red 31, green 16, blue 0)
; through palette RAM. Written for this project; same license as gbcli.
;
; Expected: on a CGB in CGB mode, every pixel is that orange; on a DMG, which
; has no palette RAM, every pixel is BGP's shade 0 (white after the boot ROM).

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

    ; The LCD may only be turned off in vblank; LY reads 0 while it's off.
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

    ; Clear VRAM bank 1 (the CGB's attributes; on a DMG, VBK is ignored and
    ; this clears bank 0 too), then bank 0: all tiles color 0, map tile 0.
    ld a, 1
    ldh [$FF4F], a      ; VBK: bank 1
    call ClearVram
    xor a
    ldh [$FF4F], a      ; VBK: bank 0
    call ClearVram

    ; BG palette 0, color 0: $021F, low byte first, through BCPS/BCPD.
    ld a, $80           ; index 0, autoincrement
    ldh [$FF68], a      ; BCPS
    ld a, $1F
    ldh [$FF69], a      ; BCPD
    ld a, $02
    ldh [$FF69], a

    ld a, $91           ; LCD on, tile data at $8000, BG on
    ldh [$FF40], a
.idle:
    jr .idle

ClearVram:
    ld hl, $8000
    ld bc, $2000
.loop:
    xor a
    ld [hl+], a
    dec bc
    ld a, b
    or c
    jr nz, .loop
    ret
