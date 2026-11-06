; HALT-forever test ROM for gbcli. Written for this project; same license as gbcli.
;
; Disables every interrupt source in IE, then halts: nothing can wake the CPU.

SECTION "Header", ROM0[$100]
    nop
    jp Main
    ds $150 - @, 0  ; rgbfix fills in the logo and checksums

SECTION "Main", ROM0[$150]
Main:
    di              ; $0150
    xor a           ; $0151
    ldh [$FFFF], a  ; $0152: IE = 0
    halt            ; $0154
    nop             ; $0155
    jr Main
