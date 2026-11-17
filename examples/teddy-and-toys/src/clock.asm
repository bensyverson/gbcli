; The round's clock: one hand that sweeps once a round.
;
; tools/gen_clock.py pre-draws CLOCK_STEPS faces (src/gen/clock.asm), one
; per 256-byte page in ROMX. The face on screen is 4x4 tiles, drawn from
; one of two tile buffers (A and B). When the hand moves, the VBlank
; handler copies the next face into the hidden buffer, 4 tiles a frame,
; and on the fifth frame points the 16 map entries at it: the hand never
; tears, and no VBlank copies more than 64 bytes.

include "hardware.inc"
include "layout.inc"
include "tuning.inc"
include "bank.inc"

DEF ROUND_FRAMES      EQU ROUND_SECONDS * 60
DEF FRAMES_PER_STEP   EQU ROUND_FRAMES / CLOCK_STEPS
	ASSERT ROUND_FRAMES % CLOCK_STEPS == 0, "CLOCK_STEPS must divide ROUND_SECONDS * 60"
	ASSERT CLOCK_STEPS <= 64, "the faces must fit in one ROMX bank"
DEF RED_FRAMES        EQU CLOCK_RED_SECONDS * 60

DEF CLOCK_TILES       EQU 16
DEF COPY_TILES        EQU 4         ; tiles copied a VBlank
DEF COPY_IDLE         EQU $FF       ; wClockCopy: no copy under way
DEF COPY_FLIP         EQU CLOCK_TILES ; wClockCopy: copied, flip next VBlank

; wClockPalette requests, served in VBlank on a GBC.
DEF PAL_NONE   EQU 0
DEF PAL_NORMAL EQU 1
DEF PAL_RED    EQU 2


SECTION "Clock", ROM0

; Starts a round: the hand at 12, the full time left. With the LCD off it
; also draws face 0 into buffer A and the map; with it on, call
; ClockRestart instead.
ClockInit::
	call ClockRestart
	SWITCH_BANK ClockRotations
	ld de, ClockRotations
	ld hl, TILEBLOCK0 + T_CLOCK_A * 16
	ld bc, CLOCK_TILES * 16
	call CopyMem
	xor a
	ld [wClockShown], a
	ld a, COPY_IDLE
	ld [wClockCopy], a
	jp ClockDrawMap

; Resets the round's time and starts face 0 copying (LCD on is fine).
ClockRestart::
	ld a, LOW(ROUND_FRAMES)
	ld [wRoundLeft], a
	ld a, HIGH(ROUND_FRAMES)
	ld [wRoundLeft + 1], a
	ld a, LOW(FRAMES_PER_STEP)
	ld [wStepLeft], a
	ld a, HIGH(FRAMES_PER_STEP)
	ld [wStepLeft + 1], a
	xor a
	ld [wClockStep], a
	ld [wClockCopy], a          ; copy face 0 in
	ld [wTickLeft], a
	ld a, PAL_NORMAL
	ld [wClockPalette], a
	ret

; Once a frame, from the main loop. Out: carry set when the round ends
; (wRoundLeft reached 0 this frame).
ClockTick::
	; The round's time.
	ld hl, wRoundLeft
	ld a, [hli]
	or [hl]
	jr z, .over                 ; already over: stay over
	dec hl
	ld a, [hl]
	sub 1
	ld [hli], a
	ld a, [hl]
	sbc 0
	ld [hl], a
	; The red zone starts at exactly RED_FRAMES left.
	dec hl
	ld a, [hli]
	cp LOW(RED_FRAMES)
	jr nz, .notRed
	ld a, [hl]
	cp HIGH(RED_FRAMES)
	jr nz, .notRed
	ld a, PAL_RED
	ld [wClockPalette], a
	ld a, SFX_WARN              ; time is running out
	call SfxPlay
	ld a, 60
	ld [wTickLeft], a
	jr .hand
.notRed
	; In the red zone, a tick each second.
	ld a, [wTickLeft]
	and a
	jr z, .hand
	dec a
	ld [wTickLeft], a
	jr nz, .hand
	ld a, 60
	ld [wTickLeft], a
	ld a, SFX_TICK
	call SfxPlay
.hand
	; The hand.
	ld hl, wStepLeft
	ld a, [hl]
	sub 1
	ld [hli], a
	ld a, [hl]
	sbc 0
	ld [hl], a
	dec hl
	or [hl]
	jr nz, .checkOver
	ld a, LOW(FRAMES_PER_STEP)
	ld [hli], a
	ld [hl], HIGH(FRAMES_PER_STEP)
	ld a, [wClockStep]
	inc a
	cp CLOCK_STEPS
	jr c, .step
	xor a                       ; the sweep is whole: back at 12
.step
	ld [wClockStep], a
	xor a
	ld [wClockCopy], a
.checkOver
	ld hl, wRoundLeft
	ld a, [hli]
	or [hl]
	ret nz                      ; carry is clear after or
.over
	scf
	ret

; From the VBlank handler: the copy and flip, and a palette change.
; Clobbers af, bc, de, hl.
ClockVBlank::
	call ClockPaletteVBlank
	ld a, [wClockCopy]
	cp COPY_IDLE
	ret z
	cp COPY_FLIP
	jr z, .flip
	; Source: the face's page, tile wClockCopy. Destination: the hidden
	; buffer, same tile.
	swap a                      ; * 16 (wClockCopy < 16)
	ld e, a
	ld l, a
	ld a, [wClockStep]
	add HIGH(ClockRotations)
	ld d, a
	ld a, [wClockShown]
	and a
	ld h, HIGH(TILEBLOCK0 + T_CLOCK_B * 16)
	jr z, .dest
	ld h, HIGH(TILEBLOCK0 + T_CLOCK_A * 16)
.dest
	ld a, BANK(ClockRotations)  ; the VBlank handler puts hRomBank's back
	ld [rROMB0], a
	ld c, COPY_TILES * 16
.copy
	ld a, [de]
	ld [hli], a
	inc e
	dec c
	jr nz, .copy
	ld a, [wClockCopy]
	add COPY_TILES
	ld [wClockCopy], a
	ret
.flip
	ld a, [wClockShown]
	xor 1
	ld [wClockShown], a
	ld a, COPY_IDLE
	ld [wClockCopy], a
	; fall through

; Points the clock's 16 map entries at the shown buffer.
ClockDrawMap:
	ld a, [wClockShown]
	and a
	ld a, T_CLOCK_A
	jr z, .base
	ld a, T_CLOCK_B
.base
	ld hl, TILEMAP0 + CLOCK_ROW * 32 + CLOCK_COL
	ld de, 32 - 4
	ld c, 4
.row
	REPT 4
	ld [hli], a
	inc a
	ENDR
	add hl, de
	dec c
	jr nz, .row
	ret

ClockPaletteVBlank:
	ld a, [wClockPalette]
	and a
	ret z
	ld b, a
	xor a
	ld [wClockPalette], a
IF DEF(GBCLI_GBC)
	ldh a, [hGbc]
	and a
	ret z
	ld a, BGPI_AUTOINC | (PAL_CLOCK * 8)     ; src/colors.asm
	ldh [rBCPS], a
	ld hl, ClockNormalColors
	ld a, b
	cp PAL_RED
	jr nz, .load
	ld hl, ClockRedColors
.load
	ld c, 8
.color
	ld a, [hli]
	ldh [rBCPD], a
	dec c
	jr nz, .color
ENDC
	ret

SECTION "Clock variables", WRAM0

wRoundLeft:: dw                    ; frames left in the round
wStepLeft: dw                      ; frames until the hand moves
wClockStep:: db                    ; the hand's position, 0 (12 o'clock) to CLOCK_STEPS - 1
wClockCopy: db                     ; next tile to copy, or COPY_IDLE, or COPY_FLIP
wClockShown: db                    ; 0: buffer A on screen, 1: buffer B
wClockPalette: db                  ; a PAL_ request for VBlank, or PAL_NONE
wTickLeft: db                      ; in the red zone, frames to the next second's tick; else 0
