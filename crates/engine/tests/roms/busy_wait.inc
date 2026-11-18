; Busy-wait frame-budget test ROMs for gbcli. Written for this project; same license as gbcli.
;
; A loop that waits for each vblank in HALT with the LCD on. At frame 4 it
; spends a whole frame polling LY for the next VBlank with interrupts off,
; never halting, the way a game waits to turn the LCD off safely. Then, at
; the start of the next frame:
;   - THEN_OFF = 1 (busy_wait_off): turns the LCD off at once, works about
;     two frames' worth without halting, turns it back on and halts again;
;   - THEN_OFF = 0 (busy_wait_halt): halts again with the LCD on.
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
    ld a, $91
    ldh [$FF40], a      ; LCD on (a quick boot ROM leaves it off)
    xor a
    ldh [$FF0F], a      ; IF: nothing pending
    inc a
    ldh [$FFFF], a      ; IE = VBlank
    ei
HaltLoop:
    halt
    nop
    ld a, [wFrame]
    cp 4
    jr nz, HaltLoop
    ; In VBlank, right after the handler: poll out this VBlank, then the
    ; whole frame up to the next one.
    di
.leave:
    ldh a, [$FF44]
    cp 144
    jr nc, .leave
.wait:
    ldh a, [$FF44]
    cp 144
    jr c, .wait
IF THEN_OFF
    xor a
    ldh [$FF40], a      ; LCD off, at the start of the next frame
    ld bc, 5000         ; about 35,000 M-cycles: two frames
.busy:
    dec bc
    ld a, b
    or c
    jr nz, .busy
    ld a, $91
    ldh [$FF40], a      ; LCD on
ENDC
    ld a, 5
    ld [wFrame], a      ; past 4, so the loop doesn't wait again
    xor a
    ldh [$FF0F], a      ; IF: forget the vblank polled past
    ei
    jr HaltLoop

VBlank:
    push af
    ld a, [wFrame]
    inc a
    ld [wFrame], a
    pop af
    reti

SECTION "WRAM", WRAM0[$C000]
wFrame:: db
