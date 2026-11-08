; Flash-save test ROM for gbcli. Written for this project; same license as gbcli.
;
; An MBC5+RAM+BATTERY cart of 8 banks (128 KiB) that saves as GB Studio's
; batteryless saves do (gbvm flasher_s.s): from WRAM, since a busy chip
; answers every ROM read with status, it maps bank 12, past the image, and
; programs $42 at $4000 with the swapped-data sequence ($0AAA<-$A9,
; $0555<-$56, $0AAA<-$A0), then polls until the byte reads back. wSaved
; counts the programs that read back. On a board this sequence misses, or
; on mask ROM, the poll never ends.

DEF SAVE_BANK EQU 12

SECTION "Header", ROM0[$100]
    nop
    jp Main
    ds $147 - @, 0      ; rgbfix fills in the logo and checksums
    db $1B              ; $0147: MBC5+RAM+BATTERY
    db $02              ; $0148: 128 KiB
    db $02              ; $0149: 8 KiB of cart RAM
    ds $150 - @, 0

SECTION "Main", ROM0[$150]
Main:
    di
    ld sp, $DFFF
    xor a
    ld [wSaved], a
    ld hl, Program
    ld de, wProgram
    ld b, ProgramEnd - Program
.copy:
    ld a, [hl+]
    ld [de], a
    inc de
    dec b
    jr nz, .copy
    call wProgram
    ld hl, wSaved
    inc [hl]
.done:
    jr .done

; Copied to WRAM and run there.
Program:
    ld a, SAVE_BANK
    ld [$2000], a
    ld a, $A9
    ld [$0AAA], a
    ld a, $56
    ld [$0555], a
    ld a, $A0
    ld [$0AAA], a
    ld a, $42
    ld [$4000], a
.poll:
    ld a, [$4000]
    cp $42
    jr nz, .poll
    ld a, 1
    ld [$2000], a
    ret
ProgramEnd:

SECTION "Last bank", ROMX, BANK[7]
    db $07

SECTION "WRAM", WRAM0[$C000]
wSaved: ds 1
wProgram: ds 64
