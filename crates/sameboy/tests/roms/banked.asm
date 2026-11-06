; Banked-memory test ROM for gbcli. Written for this project; same license as gbcli.
;
; MBC5 with 64 KiB ROM (4 banks) and 32 KiB cart RAM (4 banks). The main loop
; calls the routine at $4000 in ROM bank 1, then the one at $4000 in bank 2;
; each selects the cart RAM bank of the same number and writes its own value
; there and to WRAM, and reads WRAM.
;   bank 1: [$A000] (RAM bank 1) = $11, [$C123] = $11, [$E124] (echo of $C124) = $11
;   bank 2: [$A000] (RAM bank 2) = $22, [$C123] = $22
;   both:   read [$C200]

SECTION "Header", ROM0[$100]
    nop
    jp Main
    ds $147 - @, 0
    db $1A          ; $0147: MBC5+RAM
    db $01          ; $0148: 64 KiB ROM
    db $03          ; $0149: 32 KiB RAM
    ds $150 - @, 0  ; rgbfix fills in the logo and checksums

SECTION "Main", ROM0[$150]
Main:
    di
    ld sp, $FFFE
    ld a, $0A
    ld [$0000], a   ; enable cart RAM
.loop:
    ld a, 1
    ld [$2000], a   ; ROM bank 1
    call $4000
    ld a, 2
    ld [$2000], a   ; ROM bank 2
    call $4000
    jr .loop

SECTION "Bank1", ROMX[$4000], BANK[1]
Routine1:
    ld a, 1         ; $4000
    ld [$4000], a   ; $4002: RAM bank 1
    ld a, $11       ; $4005
    ld [$A000], a   ; $4007
    ld [$C123], a   ; $400A
    ld [$E124], a   ; $400D
    ld a, [$C200]   ; $4010
    ret

SECTION "Bank2", ROMX[$4000], BANK[2]
Routine2:
    ld a, 2         ; $4000
    ld [$4000], a   ; $4002: RAM bank 2
    ld a, $22       ; $4005
    ld [$A000], a   ; $4007
    ld [$C123], a   ; $400A
    nop             ; $400D
    nop
    nop
    ld a, [$C200]   ; $4010
    ret

SECTION "Bank3", ROMX[$4000], BANK[3]
    ret
