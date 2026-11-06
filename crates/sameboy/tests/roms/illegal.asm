; Illegal-opcode test ROM for gbcli. Written for this project; same license as gbcli.
;
; Enables an interrupt, then reaches the illegal opcode $D3 at $0156. SameBoy
; would lock the CPU there (IE=0, halted); the core must stop before it.

SECTION "Header", ROM0[$100]
    nop
    jp Main
    ds $150 - @, 0

SECTION "Main", ROM0[$150]
Main:
    di              ; $0150
    ld a, $01       ; $0151
    ldh [$FFFF], a    ; $0153: IE = vblank
    nop             ; $0155
    db $D3          ; $0156: illegal
