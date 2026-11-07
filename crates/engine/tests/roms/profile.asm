; Cycle-profiler test ROM for gbcli. Written for this project; same license as gbcli.
;
; Hand-written routines whose costs the tests compute from the opcode table
; (crates/engine/data/gb-opcodes). Interrupts stay off in the main loop, so
; nothing else runs there. It calls a leaf, a routine that calls the leaf,
; and two routines that leave by unusual paths:
;   - Discard drops its own return address (`pop hl`) and jumps back;
;   - Trampoline pushes an address and `ret`s to it, a computed jump.
; HaltLoop waits for each vblank in HALT, and the vblank handler counts
; frames in wFrames: a game that finishes its work in time.
; SpeedSwitch (CGB only) switches to double speed between two labels; the
; tests jump to it by setting PC, as they do to Key1Poke.

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
MainLoop:
    call Leaf
    call Outer
    call Discard
.back:
    call Trampoline
    jr MainLoop

Leaf:
    ld a, 5
    ret

Outer:
    call Leaf
    nop
    ret

; Leaves with its return address popped off the stack, not by RET.
Discard:
    pop hl
    jp MainLoop.back

; Calls nothing: the RET here jumps to Target.
Trampoline:
    ld hl, Target
    push hl
    ret
Target:
    ret

SpeedSwitch:
    ld a, 1
    ldh [$FF4D], a
.before:
    stop
    nop
.after:
    jr SpeedSwitch.after

; Writes KEY1 without arming a switch, so `measure`'s KEY1 watch stops the
; run right before .after, an arrival it must still see.
Key1Poke:
    xor a
    ldh [$FF4D], a
.after:
    jr Key1Poke

HaltLoop:
    ld a, $91
    ldh [$FF40], a  ; LCD on: vblank interrupts need it
    ld a, 1
    ldh [$FFFF], a  ; IE = vblank
    xor a
    ldh [$FF0F], a  ; IF = 0
    ei
.wait:
    halt
    nop
    jr HaltLoop.wait

VBlank:
    ld hl, wFrames
    inc [hl]
    reti

SECTION "Frames", WRAM0
wFrames: ds 1
