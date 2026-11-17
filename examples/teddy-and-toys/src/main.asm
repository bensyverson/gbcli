; Teddy & Toys: start-up, the frame loop and the interrupt handlers.
;
; The cartridge is MBC5 with 8 KiB of battery-backed RAM (the best score).
; Runtime code reads only ROM0; ROMX holds the clock faces (switched in by
; the VBlank handler), the font and the images, which are read only with
; the LCD off and interrupts off.

include "hardware.inc"
include "layout.inc"
include "tuning.inc"
include "images.inc"
include "bank.inc"

; Cartridge header values hardware.inc 6 doesn't name.
DEF CART_MBC5_RAM_BATTERY EQU $1B
DEF CART_RAM_8K           EQU $02

DEF STATE_PLAY    EQU 0             ; wGameState: a round is on
DEF STATE_RESULTS EQU 1             ; the title or results screen
DEF STATE_PAUSED  EQU 2             ; a round, paused (src/pause.asm)
EXPORT STATE_PLAY, STATE_RESULTS, STATE_PAUSED
DEF RANDOM_SEED   EQU $ACE1         ; any nonzero start; START's frame mixes in

SECTION "Restart vectors", ROM0[$0000]
	ds $40, $FF                 ; rst $38 everywhere: a stray jump lands in a loop

SECTION "VBlank vector", ROM0[INT_HANDLER_VBLANK]
	jp VBlank

SECTION "Header", ROM0[$0100]
	nop
	jp Start
	ds $0147 - @, 0             ; rgbfix fills the logo, title and checksums
	db CART_MBC5_RAM_BATTERY    ; $0147
	db 0                        ; $0148 ROM size: rgbfix sets it
	db CART_RAM_8K              ; $0149
	ds $0150 - @, 0

SECTION "Start", ROM0

Start:
	di
	ld sp, $E000

IF DEF(GBCLI_GBC)
	; A GBC that runs this ROM in GBC mode hands A = BOOTUP_A_CGB and the
	; header's $80 flag is set (build --gbc); check both, before A changes.
	DEF HEADER_GBC_FLAG EQU $0143
	DEF HEADER_GBC EQU $80
	cp BOOTUP_A_CGB
	ld a, 0
	jr nz, .gbcKnown
	ld a, [HEADER_GBC_FLAG]
	and HEADER_GBC
.gbcKnown
	ldh [hGbc], a
ENDC

	; LCD off, only in VBlank.
.waitVBlank
	ldh a, [rLY]
	cp LY_VBLANK
	jr c, .waitVBlank
	xor a
	ldh [rLCDC], a

	xor a
	ld [wFrame], a
	ld [wFrame + 1], a
	ld [wResultsAfterRound], a
	ld [wNewBest], a
	ld [wScore], a
	ld [wScore + 1], a
	ld a, STATE_RESULTS
	ld [wGameState], a
	ld a, LOW(RANDOM_SEED)
	ld [wSeed], a
	ld a, HIGH(RANDOM_SEED)
	ld [wSeed + 1], a
	ld hl, wShadowOAM
	ld bc, OAM_SIZE
	call ClearMem
	call CopyOAMDMA
	ld a, BANK(ClockRotations)  ; any bank: VBlank restores hRomBank
	ldh [hRomBank], a
	call SoundInit
	SWITCH_BANK song_theme
	call MusicInit
	ld b, 0                     ; channel 1: the effects', never the music's
	ld c, 1
	call hUGE_mute_channel
	call SaveLoad

	ld a, BGP_GAME
	ldh [rBGP], a
	ASSERT ELLE_OBP == OBP_GAME, "Elle and the pieces share OBP0 (include/layout.inc)"
	ld a, OBP_GAME
	ldh [rOBP0], a
	xor a
	ldh [rSCY], a
	ldh [hJoyHeld], a
	ldh [hJoyPressed], a
	ld a, KEY_WX
	ldh [rWX], a
	xor a
	ldh [rWY], a
	ld a, STAT_LYC              ; the splits' interrupt, enabled in rounds
	ldh [rSTAT], a

	call ShowResults            ; the title: best score, press START

MainLoop:
	ld a, [wFrame]
	ld b, a
.wait
	halt
	nop
	ld a, [wFrame]
	cp b
	jr z, .wait

	call ReadJoypad
	ld a, [wGameState]
	cp STATE_PAUSED
	jr z, .paused
	cp STATE_PLAY
	jr nz, .results
	; START pauses, anytime in a round (mid-turn too: the turn waits).
	ldh a, [hJoyPressed]
	and PAD_START
	jr z, .play
	call PauseStart
	jr MainLoop
.paused
	call PauseUpdate
	jr MainLoop
.play
	call BeltsUpdate
	call PlayUpdate
	call ClockTick
	jr nc, MainLoop
	call EndRound
	jr MainLoop
.results
	; START on the results screen goes back to the title; on the title, it
	; starts a round.
	ldh a, [hJoyPressed]
	and PAD_START
	jr z, MainLoop
	ld a, [wResultsAfterRound]
	and a
	jr z, .round
	xor a
	ld [wResultsAfterRound], a
	call ShowResults            ; the title
	jr MainLoop
.round
	call StartRound
	jr MainLoop

; Sets up a round with the LCD off, then plays.
StartRound:
	call LcdOff
	ld a, [wFrame]              ; when START was pressed varies the round
	call RandomMix
	call MusicNormal
	xor a
	ld [wPauseDraw], a
	call ScreenInit
IF DEF(GBCLI_GBC)
	; Before the belts: BeltsInit colors each slot it draws.
	ldh a, [hGbc]
	and a
	call nz, ColorGbc
ENDC
	call ClockInit
	call SupplyInit
	call PlayInit
	call BeltsInit
	call ElleDraw
	xor a
	ldh [rSCX], a
	ld a, SPLIT_BACK_LYC
	ldh [rLYC], a
	ld a, STATE_PLAY
	ld [wGameState], a
	ld a, LCDC_ON | LCDC_BG_ON | LCDC_OBJ_ON | LCDC_OBJ_16 | LCDC_BLOCK01 | LCDC_BG_9800 | LCDC_WIN_ON | LCDC_WIN_9C00
	ldh [rLCDC], a
	ld a, IE_VBLANK | IE_STAT
	ldh [rIE], a
	xor a
	ldh [rIF], a
	reti                        ; returns with interrupts on

; Quit from the pause menu: back to the title, with no results screen
; and the score not kept as a best.
QuitRound::
	ld a, $FF                   ; the music's channels back on (PauseStart)
	ldh [rNR51], a
	xor a
	ld [wResultsAfterRound], a
	ld [wNewBest], a
	ld [wPauseDraw], a
	ld a, STATE_RESULTS
	ld [wGameState], a
	jp ShowResults              ; the title

; Time's up: keep a new best, then the results screen.
EndRound:
	ld a, STATE_RESULTS
	ld [wGameState], a
	ld a, 1
	ld [wResultsAfterRound], a
	xor a
	ld [wNewBest], a
	; Is wScore > wBest? (BCD compares like binary.)
	ld a, [wBest + 1]
	ld b, a
	ld a, [wScore + 1]
	cp b
	jr c, .show
	jr nz, .new
	ld a, [wBest]
	ld b, a
	ld a, [wScore]
	cp b
	jr c, .show
	jr z, .show
.new
	ld a, [wScore]
	ld [wBest], a
	ld a, [wScore + 1]
	ld [wBest + 1], a
	call SaveBest
	ld a, 1
	ld [wNewBest], a
.show
	ld a, SFX_TIMEUP
	call SfxPlay
	jp ShowResults

; hJoyHeld and hJoyPressed (newly down this frame): PAD_* bits.
ReadJoypad:
	ld a, JOYP_GET_CTRL_PAD
	ldh [rP1], a
	ldh a, [rP1]
	ldh a, [rP1]
	cpl
	and $0F
	swap a
	ld b, a
	ld a, JOYP_GET_BUTTONS
	ldh [rP1], a
	REPT 6                      ; let the lines settle
	ldh a, [rP1]
	ENDR
	cpl
	and $0F
	or b
	ld b, a
	ldh a, [hJoyHeld]
	xor b
	and b
	ldh [hJoyPressed], a
	ld a, b
	ldh [hJoyHeld], a
	ld a, JOYP_GET_NONE
	ldh [rP1], a
	ret

; ---------------------------------------------------------------------------

VBlank:
	push af
	push bc
	push de
	push hl
	; The HUD band has no scroll, and the first split is armed again.
	xor a
	ldh [rSCX], a
	ld a, SPLIT_BACK_LYC
	ldh [rLYC], a
	ld a, HIGH(wShadowOAM)
	call hOAMDMA
	ld a, [wGameState]
	cp STATE_PAUSED
	jr z, .paused
	cp STATE_PLAY
	jr nz, .sound
	call PauseVBlank            ; closing the menu: before the score's digits
	call BeltsVBlank
	call ScoreVBlank
	call ClockVBlank
	jr .sound
.paused
	call PauseVBlank
	call BeltsVBlank            ; what was queued before the pause
	call ClockVBlank            ; a face copy under way finishes
	call SfxVBlank              ; the pause sound; the music holds its place
	jr .bank
.sound
	; Sound, on every screen: the effects, then a tick of the music, whose
	; data is in ROMX. Then the main code's bank back (the clock and the
	; music replaced it).
	call SfxVBlank
	ld a, BANK(song_theme)
	ld [rROMB0], a
	call MusicVBlank
.bank
	ldh a, [hRomBank]
	ld [rROMB0], a
	ld hl, wFrame
	inc [hl]
	jr nz, .done
	inc hl
	inc [hl]
.done
	pop hl
	pop de
	pop bc
	pop af
	reti

; Redraws the score box's three digits if the score changed.
ScoreVBlank:
	ld a, [wScoreDirty]
	and a
	ret z
	xor a
	ld [wScoreDirty], a
	ld hl, TILEMAP0 + (SCORE_ROW + 1) * 32 + SCORE_COL + 1
	ld a, [wScore + 1]
	and $0F
	add T_DIGITS
	ld [hli], a
	ld a, [wScore]
	swap a
	and $0F
	add T_DIGITS
	ld [hli], a
	ld a, [wScore]
	and $0F
	add T_DIGITS
	ld [hl], a
	ret

; ---------------------------------------------------------------------------
; Memory helpers.

; Writes BC zero bytes from HL.
ClearMem::
	xor a
	; fall through

; Writes BC bytes of A from HL.
FillMem::
	ld d, a
.loop
	ld a, d
	ld [hli], a
	dec bc
	ld a, b
	or c
	jr nz, .loop
	ret

; Copies BC bytes from DE to HL.
CopyMem::
	ld a, [de]
	ld [hli], a
	inc de
	dec bc
	ld a, b
	or c
	jr nz, CopyMem
	ret

; ---------------------------------------------------------------------------

SECTION "Variables", WRAM0

wFrame:: dw                        ; VBlanks since start-up
wGameState:: db                    ; STATE_PLAY or STATE_RESULTS

SECTION "Joypad", HRAM
hJoyHeld:: db                      ; PAD_* bits held now
hJoyPressed:: db                   ; PAD_* bits newly pressed this frame

SECTION "Bank", HRAM
hRomBank:: db                      ; the ROMX bank main code selected (bank.inc)

IF DEF(GBCLI_GBC)
SECTION "GBC mode", HRAM
hGbc:: db                          ; nonzero in GBC mode
ENDC

SECTION "Shadow OAM", WRAM0, ALIGN[8]
wShadowOAM:: ds OAM_SIZE
