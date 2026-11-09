; LCD-off frame-budget test ROM for gbcli. Written for this project; same license as gbcli.
;
; A game's two deliberate LCD-off stretches, around a loop that waits for
; each vblank in HALT (so its frames are never lag):
;   - startup: waits for VBlank, turns the LCD off, works for about three
;     frames' worth of cycles without halting, then turns it back on;
;   - a screen change at frame 8: after the HALT that frame's vblank ends,
;     the LCD goes off for about two frames' worth of busy work.
; The vblank handler counts frames in wFrame.

SECTION "VBlank vector", ROM0[$40]
VBlankVector:
    jp VBlank

SECTION "Header", ROM0[$100]
    nop
    jp Main
    ds $150 - @, 0  ; rgbfix fills in the logo and checksums

SECTION "Main", ROM0[$150]
Main:
    di
    ld sp, $DFFF
    xor a
    ld [wFrame], a
    call LcdOff
    ld bc, 7500         ; about 52,500 M-cycles: three frames
    call Busy
    call LcdOn
    xor a
    ldh [$FF0F], a      ; IF: nothing pending
    inc a
    ldh [$FFFF], a      ; IE = VBlank
    ei
HaltLoop:
    halt
    nop
    ld a, [wFrame]
    cp 8
    jr nz, HaltLoop
    ; The screen change: in VBlank, right after the handler.
    xor a
    ldh [$FF40], a      ; LCD off
    ld bc, 5000         ; about 35,000 M-cycles: two frames
    call Busy
    call LcdOn
    jr HaltLoop

; Waits for VBlank (LY >= 144), then turns the LCD off; returns at once if
; it is off already (as a quick boot ROM leaves it).
LcdOff:
    ldh a, [$FF40]
    bit 7, a
    ret z
.wait:
    ldh a, [$FF44]
    cp 144
    jr c, .wait
    xor a
    ldh [$FF40], a
    ret

LcdOn:
    ld a, $91
    ldh [$FF40], a
    ret

; Spends 7 M-cycles per count in BC.
Busy:
    dec bc
    ld a, b
    or c
    jr nz, Busy
    ret

VBlank:
    push af
    ld a, [wFrame]
    inc a
    ld [wFrame], a
    pop af
    reti

SECTION "WRAM", WRAM0[$C000]
wFrame:: db
