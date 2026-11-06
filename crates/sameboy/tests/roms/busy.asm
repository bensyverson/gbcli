; Busy-work test ROM for gbcli. Written for this project; same license as gbcli.
;
; Turns the LCD on and runs vblank and timer interrupts while the main loop
; counts in WRAM and writes VRAM, so PPU, timer, interrupt and memory state all
; change from instruction to instruction. Used by save/load and inspection tests.
;   $C000-$C001  main-loop counter (16-bit)
;   $C002        vblank count
;   $FF80        timer interrupt count

SECTION "VBlank", ROM0[$40]
    jp VBlank
SECTION "Timer", ROM0[$50]
    jp Timer

SECTION "Header", ROM0[$100]
    nop
    jp Main
    ds $150 - @, 0  ; header; rgbfix fills in the logo and checksums

SECTION "Main", ROM0[$150]
Main:
    di
    ld sp, $FFFE
    xor a
    ld [$C000], a
    ld [$C001], a
    ld [$C002], a
    ldh [$FF80], a
    ldh [$FF06], a    ; TMA
    ld a, $05
    ldh [$FF07], a    ; TAC: on, 262144 Hz
    ld a, $E4
    ldh [$FF47], a    ; BGP
    ld a, $05
    ldh [$FFFF], a    ; IE: vblank and timer
    ld a, $91
    ldh [$FF40], a    ; LCDC: on, BG on
    ei
.loop:
    ld hl, $C000
    inc [hl]
    jr nz, .noCarry
    inc hl
    inc [hl]
.noCarry:
    ld a, [$C000]
    ld l, a
    ld h, $80
    ld [hl], a      ; tile data; dropped while the PPU blocks VRAM
    jr .loop

VBlank:
    push af
    ld a, [$C002]
    inc a
    ld [$C002], a
    pop af
    reti

Timer:
    push af
    ldh a, [$FF80]
    inc a
    ldh [$FF80], a
    pop af
    reti
