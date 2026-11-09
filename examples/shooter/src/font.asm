; Text and the status bar, on the window (map $9C00).
;
; During play the window shows only its row 0, the status bar, from line
; 136; the title and result screens raise it to line 0 so it covers the
; screen. On a GBC every text cell takes BG palette PAL_TEXT.

include "hardware.inc"
include "charmap.inc"

DEF HUD_W EQU 20
DEF HUD_SCORE EQU 2                   ; column of the score's first digit
DEF HUD_BOSS EQU 10                   ; "B" and two digits of boss HP
DEF HUD_LIVES EQU 16                  ; "^" "x" and one digit

SECTION "Status bar", WRAM0

wHud:: ds HUD_W                       ; the bar's tiles, copied in VBlank
wHudDirty:: db

SECTION "Text code", ROM0

; With the LCD off: fills the window map's 18 rows with the space (and, on
; a GBC, the text palette) and raises the window over the whole screen.
TextScreen::
	ld hl, TILEMAP1
	ld bc, 32 * 18
	ld a, T_SPACE
	call FillMem
	ld a, PAL_TEXT
	call WindowAttrs
	xor a
	ldh [rWY], a
	ret

; With the LCD off, on a GBC only: every window attribute to A.
WindowAttrs::
IF DEF(GBCLI_GBC)
	ld b, a
	ldh a, [hGbc]
	and a
	ret z
	ld a, 1
	ldh [rVBK], a
	ld a, b
	ld hl, TILEMAP1
	ld bc, 32 * 18
	call FillMem
	xor a
	ldh [rVBK], a
ENDC
	ret

; With the LCD off: writes the string at DE to the window map at row B,
; column C, in the text palette. Leaves HL after its last character.
TextAt::
	ld h, 0
	ld l, b
	add hl, hl
	add hl, hl
	add hl, hl
	add hl, hl
	add hl, hl                        ; row * 32
	ld a, c
	add l
	ld l, a
	ld a, h
	adc HIGH(TILEMAP1)
	ld h, a
.char
	ld a, [de]
	inc de
	cp TEXT_END
	ret z
	ld [hl], a
	call TextAttr
	inc hl
	jr .char

; With the LCD off: the text palette for the map cell at HL (GBC only).
; Keeps every register but A.
TextAttr::
IF DEF(GBCLI_GBC)
	ldh a, [hGbc]
	and a
	ret z
	ld a, 1
	ldh [rVBK], a
	ld [hl], PAL_TEXT
	xor a
	ldh [rVBK], a
ENDC
	ret

; With the LCD off: the play layout, the window down to the status bar.
HudScreen::
	call TextScreen
	ld a, BAR_LINE
	ldh [rWY], a
	ld hl, wHud
	ld de, HudTemplate
	ld bc, HUD_W
	call CopyMem
	ld a, 1
	ld [wHudDirty], a
	ret

; In VBlank: the status bar to the window's row 0, if it changed.
HudVBlank::
	ld a, [wHudDirty]
	and a
	ret z
	xor a
	ld [wHudDirty], a
	push bc
	push de
	ld hl, TILEMAP1
	ld de, wHud
	ld b, HUD_W
.copy
	ld a, [de]
	inc de
	ld [hli], a
	dec b
	jr nz, .copy
	pop de
	pop bc
	ret

; The score (3 BCD bytes, most significant first, at wScore) as six digit
; tiles from HL.
ScoreDigits::
	ld de, wScore
	ld b, 3
.digits
	ld a, [de]
	inc de
	ld c, a
	swap a
	and $0F
	add T_DIGITS
	ld [hli], a
	ld a, c
	and $0F
	add T_DIGITS
	ld [hli], a
	dec b
	jr nz, .digits
	ret

; The score into the bar.
HudScore::
	ld hl, wHud + HUD_SCORE
	call ScoreDigits
	jr HudChanged

; The lives count (wLives, 0-9) into the bar.
HudLives::
	ld a, [wLives]
	add T_DIGITS
	ld [wHud + HUD_LIVES + 2], a
	jr HudChanged

; The boss's HP (wBossHp, 0-99) into the bar as "B" and two digits.
HudBoss::
	ld a, [wBossHp]
	ld b, -1
.tens
	inc b
	sub 10
	jr nc, .tens
	add 10 + T_DIGITS
	ld [wHud + HUD_BOSS + 2], a
	ld a, b
	add T_DIGITS
	ld [wHud + HUD_BOSS + 1], a
	ld a, 'B'
	ld [wHud + HUD_BOSS], a
	jr HudChanged

; Blanks the boss's HP.
HudBossOff::
	ld a, T_SPACE
	ld hl, wHud + HUD_BOSS
	ld [hli], a
	ld [hli], a
	ld [hl], a
HudChanged:
	ld a, 1
	ld [wHudDirty], a
	ret

HudTemplate:
	db "SC000000        ^x3 "
