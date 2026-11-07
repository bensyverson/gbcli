; Trap demo for `gbcli help traps`: an OAM DMA routine run from ROM.
; Written for this project; same license as gbcli.
;
; During OAM DMA a DMG CPU can read only HRAM; anything else it fetches is
; the byte the DMA is copying. The same routine (start DMA, wait 160
; M-cycles, ret) runs twice:
;   - from HRAM, where it belongs: it returns to AfterHramDma;
;   - from ROM: its own instructions are replaced by shadow OAM bytes, so it
;     never reaches its `ret` and AfterRomDma is never reached.
; `gbcli run --to AfterHramDma` arrives; `gbcli run --to AfterRomDma` then
; fails with budget_exhausted.

SECTION "Header", ROM0[$100]
    nop
    jp Main
    ds $150 - @, 0  ; rgbfix fills in the logo and checksums

SECTION "Main", ROM0[$150]
Main:
    di
    ld sp, $DFFF
    ld hl, wShadowOam
    ld b, 160
    xor a
.clear
    ld [hli], a
    dec b
    jr nz, .clear
    ld hl, DmaSource
    ld de, hDma
    ld b, DmaSourceEnd - DmaSource
.copy
    ld a, [hli]
    ld [de], a
    inc de
    dec b
    jr nz, .copy
    call hDma
AfterHramDma:
    call DmaSource
AfterRomDma:
    jr AfterRomDma

; The routine's bytes in ROM, assembled for HRAM; jr is relative, so the
; same bytes run from either place.
SECTION "DMA routine", ROM0[$1000]
DmaSource:
LOAD "DMA in HRAM", HRAM
hDma:
    ld a, HIGH(wShadowOam)
    ldh [$FF46], a  ; start OAM DMA
    ld a, 40        ; 40 * 4 M-cycles
.wait
    dec a
    jr nz, .wait
    ret
ENDL
DmaSourceEnd:

SECTION "Shadow OAM", WRAM0, ALIGN[8]
wShadowOam: ds 160
