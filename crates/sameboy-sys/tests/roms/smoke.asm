; Smoke-test ROM for gbcli. Written for this project; same license as gbcli.
;
; Writes once to echo RAM at $F000, which makes SameBoy log
; "Wrote 42 to f000 (RAM Mirror)", then spins forever. (SameBoy logs echo
; writes only in $F000-$FDFF; $E000-$EFFF goes through its plain RAM path.)
; The smoke test uses that log line to prove the ROM ran and that SameBoy's
; logging was routed away from stdout.

SECTION "Header", ROM0[$100]
    nop
    jp Main
    ds $150 - @, 0  ; header; rgbfix fills in the logo and checksums

SECTION "Main", ROM0[$150]
Main:
    di
    ld a, $42
    ld [$F000], a
.spin:
    jr .spin
