; The scanline road: per-line scroll tables, the HBlank interrupt that plays
; them, and the routine that fills them once a frame.
;
; The background map (assets/bg/road.png, drawn by tools/road.py) holds the
; sky and two sets of road strips, one per stripe phase, from the horizon
; (narrow) to the bottom of the screen (wide). Every screen line shows one
; BG row: SCY picks the strip (its depth), SCX shifts it for curves and
; steering. Above the horizon, the sky shows with one SCY and SCX.
;
; Tables: a table entry at index L holds the values for line L + 1, because
; the HBlank interrupt at the end of line L sets up the next line. Line 0's
; values sit at index 255, where the VBlank handler reads them. Two buffers:
; the interrupt plays the front one while ComputeRoad fills the back one,
; and VBlank swaps them only when the back one is complete.

include "hardware.inc"
include "road_geometry.inc"

; ---------------------------------------------------------------------------
; The HBlank interrupt: every drawn line's mode 0 raises it, and it writes
; SCX, then SCY, for the next line. Both writes must land in this HBlank,
; which is 376 - mode 3 dots: mode 3 is 172 dots, plus SCX % 8, plus 6-11
; per object on the line. So the handler does the writes first, from
; immediates (StatHandler in RAM, below), done 92 dots after the interrupt
; plus up to 20 dots for the instruction it interrupted: 112 at most. With
; at most 7 objects on a line (src/drive.asm, tools/sprites.py) HBlank lasts
; at least 376 - (172 + 7 + 77) = 120 dots. Then the handler patches the
; immediates with the line after next, from the table.
;
; The sky needs no writes: its lines all share line 0's SCX and SCY, set in
; VBlank. So VBlank turns the HBlank interrupt off (IE) and arms the timer
; to fire a few lines above the horizon, and the timer interrupt turns it
; back on. That saves the interrupt's cost on every sky line.

DEF OPCODE_NOP EQU $00
DEF OPCODE_CPL EQU $2F
DEF TIMER_LEAD EQU 4           ; lines above the horizon the timer aims for
DEF TIMER_TICK EQU 256          ; T-cycles per tick at 16384 Hz

SECTION "STAT vector", ROM0[INT_HANDLER_STAT]
	jp StatHandler

SECTION "Timer vector", ROM0[INT_HANDLER_TIMER]
	jp TimerHandler

SECTION "Timer handler", ROM0
TimerHandler:
	; Wait for a line's OAM scan (mode 2), so no HBlank starts while this
	; runs: one that did would be served late, past its HBlank. Patch the
	; next line's values into the HBlank handler, drop the HBlank requests
	; the sky lines raised (or one would fire at once, mid-line), let the
	; next HBlank in, and stop the timer.
	push af
	push hl
.wait
	ldh a, [rSTAT]
	and STAT_MODE
	cp STAT_OAM
	jr nz, .wait
	ldh a, [rLY]
	ld l, a
	ldh a, [hShow]
	ld h, a
	ld a, [hl]
	ld [StatScx + 1], a
	inc h
	ld a, [hl]
	ld [StatScy + 1], a
	ldh a, [rIF]
	and ~IF_STAT
	ldh [rIF], a
	ld a, IE_VBLANK | IE_STAT
	ldh [rIE], a
	xor a
	ldh [rTAC], a
	pop hl
	pop af
	reti

SECTION "Road", ROM0

; The VBlank handler's part, with interrupts off: show the finished back
; buffer (if ComputeRoad finished one), set line 0's scroll, and arm the
; timer for this frame's horizon.
RoadVBlank::
	ldh a, [hReady]
	and a
	jr z, .noSwap
	ldh a, [hShow]
	xor 2
	ldh [hShow], a
	ldh a, [hBackHorizon]
	ldh [hFrontHorizon], a
	xor a
	ldh [hReady], a
.noSwap
	call SetLineZero
	; No HBlank interrupts until the timer says the road is near.
	ld a, IE_VBLANK | IE_TIMER
	ldh [rIE], a
	; TIMA, so the timer overflows on line horizon - TIMER_LEAD.
	ldh a, [hFrontHorizon]
	ld l, a
	ld h, 0
	ld de, TimerStart
	add hl, de
	ld a, [hl]
	ldh [rTIMA], a
	ld a, TAC_START | TAC_16KHZ
	ldh [rTAC], a
	ret

; Per horizon line: 256 - the timer ticks from early in line 144 to line
; horizon - TIMER_LEAD (the 10 VBlank lines, then the drawn ones).
TimerStart:
FOR Y, 144
	REDEF TICKS = (10 * 456 + (Y - TIMER_LEAD) * 456) / TIMER_TICK
	IF TICKS < 1
		REDEF TICKS = 1
	ENDC
	db 256 - TICKS
ENDR

; Fills both buffers, for the first frame. Call it with the LCD off.
InitRoad::
	ld a, HIGH(wScrollTables)
	ldh [hShow], a
	ld hl, RoadLoopsRom
	ld de, RoadLoops
	ld bc, RoadLoopsRomEnd - RoadLoopsRom
.copy
	ld a, [hli]
	ld [de], a
	inc de
	dec bc
	ld a, b
	or c
	jr nz, .copy
	call ComputeRoad            ; fills the back buffer
	ldh a, [hShow]
	xor 2
	ldh [hShow], a
	call ComputeRoad            ; and the other
	xor a
	ldh [hReady], a
	ld a, [wHorizon]
	ldh [hFrontHorizon], a
	ret

; Sets SCX and SCY for line 0 from the front buffer. The VBlank handler calls
; it after any swap.
SetLineZero::
	ldh a, [hShow]
	ld h, a
	ld l, $FF
	ld a, [hl]
	ldh [rSCX], a
	inc h
	ld a, [hl]
	ldh [rSCY], a
	ret

; Fills the back buffer from wHill, wCurve, wPlayerX, wPos and wSkyX, then
; marks it ready for the next VBlank to show.
ComputeRoad::
	ldh a, [hShow]
	xor 2
	ld [wBack], a

	; The horizon: the first line the road reaches at this hill level.
	ld a, [wHill]
	ld e, a
	ld d, 0
	ld hl, HillHorizon
	add hl, de
	ld a, [hl]
	ld [wHorizon], a

	; The stripe phase, from depth (wPos / 16) mod PHASES, picks the table
	; that maps each strip to its BG row: its page goes in B, its offset is
	; patched into the vertical loop.
	ld a, [wPos]
	swap a
	and $0F
	ld c, a
	ld a, [wPos + 1]
	swap a
	and $F0
	or c
	and PHASES - 1
	add a
	ld e, a
	ld d, 0
	ld hl, StripePtrs
	add hl, de
	ld a, [hli]
	ld [VPatchBase + 1], a
	ld b, [hl]
	push bc

	; DE: the hill profile's byte for the horizon line.
	ld a, [wHill]
	call LevelOffset
	ld de, HillProfiles - FIRST_LINE
	add hl, de
	ld a, [wHorizon]
	ld e, a
	ld d, 0
	add hl, de
	ld d, h
	ld e, l

	; The sky's SCY, on line 0 (index 255) and down to the horizon: the
	; sky's bottom row lands on the line above the road.
	ld a, [wBack]
	inc a
	ld h, a
	ld a, [wHorizon]
	ld c, a
	ld a, SKY_ROWS
	sub c
	ld l, $FF
	ld [hl], a
	ld l, 0
	dec c
.skyY
	ld [hli], a
	dec c
	jr nz, .skyY
	pop bc
	call VLoop                  ; the road's SCY, horizon to bottom

	; The sky's SCX, likewise.
	ld a, [wBack]
	ld h, a
	ld a, [wHorizon]
	ld c, a
	ld a, [wSkyX + 1]
	ld l, $FF
	ld [hl], a
	ld l, 0
	dec c
.skyX
	ld [hli], a
	dec c
	jr nz, .skyX

	; The road's SCX, bottom to horizon: the curve's offset for the line,
	; plus the steering offset, which starts at -wPlayerX on the bottom line
	; and closes by wPlayerX * 3 / 256 a line, near zero at the horizon.
	ld a, [wHorizon]
	sub 2
	ld [HPatchStop + 1], a
	ld a, [wPlayerX]
	ld l, a
	add a
	sbc a
	ld h, a
	ld d, h
	ld e, l
	add hl, hl
	add hl, de
	ld a, l
	ld [HPatchDpLo + 1], a
	ld a, h
	ld [HPatchDpHi + 1], a
	; The curve: |level - FLAT| picks the profile; a left bend mirrors it.
	ld a, [wCurve]
	sub FLAT
	ld b, OPCODE_NOP
	ld c, ROAD_SCX0 + 1
	jr nc, .right
	cpl
	inc a
	ld b, OPCODE_CPL
	ld c, ROAD_SCX0
.right
	ld e, a
	ld a, b
	ld [HPatchMirror], a
	ld a, c
	ld [HPatchK + 1], a
	ld a, e
	call LevelOffset
	ld de, CurveProfiles + (143 - FIRST_LINE)
	add hl, de
	ld d, h
	ld e, l
	ld a, [wPlayerX]
	cpl
	inc a
	ld b, a
	ld c, 0
	ld a, [wBack]
	ld h, a
	ld l, 142
	call HLoop

	ld a, [wHorizon]
	ldh [hBackHorizon], a
	ld a, 1
	ldh [hReady], a
	ret

; HL = A * PROFILE_LINES: a level's offset into HillProfiles or CurveProfiles.
LevelOffset:
	ASSERT PROFILE_LINES == 128
	srl a
	ld h, a
	ld a, 0
	rra
	ld l, a
	ret

; Code run from WRAM so it can patch its own immediates: the HBlank
; handler, and the two per-line loops, whose constants ComputeRoad patches
; once a frame instead of spending a register on each.
RoadLoopsRom:
LOAD "Road loops", WRAM0
RoadLoops:

; The HBlank handler (see the top of this file). The writes come first;
; then the values for the line after next (table index LY + 1) go into the
; two immediates.
StatHandler:
	push af
StatScx:
	ld a, 0
StatScxWrite::
	ldh [rSCX], a
StatScy:
	ld a, 0
StatScyWrite::
	ldh [rSCY], a
	push hl
	ldh a, [rLY]
	ld l, a
	inc l
	ldh a, [hShow]
	ld h, a
	ld a, [hl]
	ld [StatScx + 1], a
	inc h                       ; the SCY table is the next page
	ld a, [hl]
	ld [StatScy + 1], a
	pop hl
	pop af
	reti

; SCY for lines from the horizon down: DE walks the hill profile (strip per
; line), BC the stripe table (BG row - 1 per strip), HL the SCY table from
; the horizon's index to 142. SCY = BG row - line = (BG row - 1) - index.
VLoop:
	ld a, [de]
	inc de
VPatchBase:
	add a, 0
	ld c, a
	ld a, [bc]
	sub l
	ld [hli], a
	ld a, l
	cp 143
	jr nz, VLoop
	ret

; SCX for lines from the bottom up: DE walks the curve profile backward, B.C
; is the steering offset in 8.8, HL the SCX table from 142 down.
; SCX = ROAD_SCX0 - (curve + steering). A left bend reads the mirrored
; right bend's profile: ComputeRoad patches a cpl in (curve = -profile - 1)
; and the constant down by one to match.
HLoop:
	ld a, [de]
	dec de
HPatchMirror:
	nop
	add b
	cpl
HPatchK:
	add ROAD_SCX0 + 1
	ld [hld], a
	ld a, c
HPatchDpLo:
	add a, 0
	ld c, a
	ld a, b
HPatchDpHi:
	adc a, 0
	ld b, a
	ld a, l
HPatchStop:
	cp 0
	jr nz, HLoop
	ret
ENDL
RoadLoopsRomEnd:

; ---------------------------------------------------------------------------

SECTION "Scroll tables", WRAM0, ALIGN[8]
; Four pages: SCX and SCY for buffer A, then for buffer B.
wScrollTables:: ds 4 * 256

SECTION "Road variables", WRAM0
wBack:: db                      ; the back buffer's SCX page
wHorizon:: db                   ; the first road line this frame
wHill:: db                      ; hill level, 0-31: FLAT is level, higher climbs
wCurve:: db                     ; curve level, 0-31: FLAT is straight, higher bends right
wPlayerX:: db                   ; the car's offset from the road's center, signed pixels
wPos:: ds 3                     ; distance driven, 24 bits; depth units are wPos / 16
wSkyX:: dw                      ; the sky's SCX, in 8.8

SECTION "Road HRAM", HRAM
hShow:: db                      ; the front buffer's SCX page
hReady:: db                     ; nonzero when the back buffer is complete
hBackHorizon: db                ; the back buffer's first road line
hFrontHorizon: db               ; the front buffer's
