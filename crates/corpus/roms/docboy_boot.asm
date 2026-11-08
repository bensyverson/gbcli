; A DMG boot ROM that hands over in docboy's post-boot state. Written for this
; project; same license as gbcli. Test-only: gbcli-corpus runs docboy's cases
; through it (crates/corpus/src/docboy.rs); no session uses it.
;
; docboy runs its tests with no boot ROM, from a hand-coded hand-over state
; (docboy 01e04d3: the `if_bootrom_else` reset values in cpu.cpp, timers.cpp,
; ppu.cpp, apu.cpp, interrupts.h, hram.cpp, and vram0.cpp's VRAM image). That
; state is a real DMG boot ROM's: DIV's 16-bit counter at $ABCC, the PPU 395
; dots into line 153, channel 1 triggered long enough ago to have faded out,
; the logo in VRAM. SameBoy always runs a boot ROM, so this one reaches that
; state with four timed writes (APU on, sound trigger, LCD on, DIV reset)
; instead of the logo animation.
;
; DIV and LCD are calibrated against the boot tests docboy passes (mooneye's
; boot_div, GBMicrotest's poweron_div/ly/stat/vram edges): each passes at
; exactly one value. APU is derived (SameBoy's frame-sequencer counter at
; 1 mod 8, docboy's div_apu = 18) and checked: docboy's ch1 length-timer tests
; pass at every even step and fail at every odd one. See
; project/2026-11-06-docboy-baseline.md and crates/corpus/tests/it/docboy_boot.rs.
; Assembled to docboy_boot.bin (256 bytes, unpadded) by scripts/build-roms.sh.

; M-cycles from each timed write to the boot ROM's last write (FF50).
DEF APU_TO_HANDOVER EQU 805613   ; DIV reset + APU on: 393 frame-sequencer steps (1 mod 8) to hand-over
DEF SOUND_TO_HANDOVER EQU 786432 ; 0.75 s: the envelope (F, down, pace 3) is at 0 after 0.70 s
DEF LCD_TO_HANDOVER EQU 17540    ; the PPU 395 dots into line 153
DEF DIV_TO_HANDOVER EQU 10994    ; DIV's counter at $ABCC

; Spends exactly \1 M-cycles (at least 9, or under 9 as NOPs). Clobbers A, BC.
MACRO delay
    REDEF DELAY_LEFT = \1
    REPT 8
        IF DELAY_LEFT >= 9
            REDEF DELAY_N = (DELAY_LEFT - 2) / 7
            IF DELAY_N > 65535
                REDEF DELAY_N = 65535
            ENDC
            ld bc, DELAY_N          ; 3
.loop\@
            dec bc                  ; 2
            ld a, b                 ; 1
            or c                    ; 1
            jr nz, .loop\@          ; 3 taken, 2 not
            REDEF DELAY_LEFT = DELAY_LEFT - (7 * DELAY_N + 2)
        ENDC
    ENDR
    ASSERT DELAY_LEFT < 9
    REPT DELAY_LEFT
        nop
    ENDR
ENDM

SECTION "Boot", ROM0[$0]
Start:
    ld sp, $FFFE

    ld hl, $8000                    ; VRAM is not cleared at power-on
    xor a
.clearVram
    ld [hl+], a
    bit 5, h
    jr z, .clearVram
    ldh [$FFFF], a                  ; IE

    ld a, $FC
    ldh [$FF47], a                  ; BGP

    ; The cartridge's logo, each bit doubled, each row twice, from tile 1.
    ld de, $0104
    ld hl, $8010
.logo
    ld a, [de]
    ld b, a
    call WriteNibble
    call WriteNibble
    inc de
    ld a, e
    cp $34
    jr nz, .logo

    ld de, Registered               ; tile $19, after the logo's 24
    ld c, 8
.registered
    ld a, [de]
    inc de
    ld [hl+], a
    inc hl
    dec c
    jr nz, .registered

    ; Tile map: 1-12 at $9904, $19 at $9910, 13-24 at $9924.
    ld a, $19
    ld [$9910], a
    ld hl, $992F
    ld c, 12
.map
    dec a
    jr z, .mapDone
    ld [hl-], a
    dec c
    jr nz, .map
    ld l, $0F
    jr .map
.mapDone

    ; The stack bytes a real boot leaves, and $FFFE's power-on value (docboy's
    ; hram.cpp). Nothing below uses the stack, so they survive to hand-over.
    ld hl, $FFFA
    ld a, $39
    ld [hl+], a
    ld a, $01
    ld [hl+], a
    ld a, $2E
    ld [hl+], a
    xor a
    ld [hl+], a
    ld [hl], $4C

    ; From here on every cycle is counted: an ldh write lands in its last
    ; M-cycle, and each delay is measured from the previous timed write.
    ldh [$FF04], a                  ; 3: DIV's counter to 0, so the APU powers on
    ld a, $80                       ; 2  with DIV's bit 12 clear (no skipped event)
    ldh [$FF26], a                  ; 3: NR52: APU on
    ldh [$FF11], a                  ; 3: NR11: 50% duty
    ld a, $F3                       ; 2
    ldh [$FF12], a                  ; 3: NR12: volume F, down, pace 3
    ldh [$FF25], a                  ; 3: NR51
    ld a, $77                       ; 2
    ldh [$FF24], a                  ; 3: NR50
    ld a, $C1                       ; 2
    ldh [$FF13], a                  ; 3: NR13
    delay APU_TO_HANDOVER - SOUND_TO_HANDOVER - 26
    ld a, $87                       ; 2
    ldh [$FF14], a                  ; 3: NR14: trigger channel 1
    delay SOUND_TO_HANDOVER - LCD_TO_HANDOVER - 5
    ld a, $91                       ; 2
    ldh [$FF40], a                  ; 3: LCD on, BG on, tiles at $8000
    delay LCD_TO_HANDOVER - DIV_TO_HANDOVER - 3
    ldh [$FF04], a                  ; 3: DIV's counter to 0
    delay DIV_TO_HANDOVER - 22
    ld a, $88                       ; 2
    add a, $78                      ; 2: F=Z-HC ($B0) without touching the stack
    ld a, $01                       ; 2
    ld bc, $0013                    ; 3
    ld de, $00D8                    ; 3
    ld hl, $014D                    ; 3
    jp Handover                     ; 4

; Writes B's high nibble with each bit doubled to two rows at HL (bitplane 1
; left 0), then shifts B left by four.
WriteNibble:
    ld c, 0
    ld a, 4
.bit
    sla c
    sla c
    sla b
    jr nc, .zero
    inc c
    inc c
    inc c
.zero
    dec a
    jr nz, .bit
    ld a, c
    ld [hl+], a
    inc hl
    ld [hl+], a
    inc hl
    ret

Registered:
    db $3C, $42, $B9, $A5, $B9, $A5, $42, $3C

    ds $FE - @, 0
Handover:                           ; $00FE: writing FF50 unmaps this ROM, so PC lands on $0100
    ldh [$FF50], a                  ; 3
