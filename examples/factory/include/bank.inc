; ROM banks. Main code switches with SWITCH_BANK, which records the bank in
; hRomBank; the VBlank handler, which switches to the clock's faces, puts
; hRomBank's bank back before it returns.

IF !DEF(BANK_INC)
DEF BANK_INC EQU 1

; Selects the ROMX bank holding label \1. Clobbers a.
MACRO SWITCH_BANK
	ld a, BANK(\1)
	ldh [hRomBank], a
	ld [rROMB0], a
ENDM

ENDC
