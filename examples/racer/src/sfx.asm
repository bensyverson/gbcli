; Sound effects on channel 2, which the race song leaves empty: an engine
; drone whose pitch follows the speed, and short effects that interrupt it
; (countdown beeps, the start, a bump, a crash, the finish, time up). One
; call a frame from the main loop.
;
; An effect is a list of steps: frames, NR22 (volume and envelope), then
; the 11-bit frequency (f = 131072 / (2048 - x) Hz); a zero ends it. Each
; step retriggers the channel.

include "hardware.inc"
include "race.inc"

DEF ENGINE_DUTY EQU $00          ; 12.5%: buzzy
DEF ENGINE_VOLUME EQU $40        ; volume 4, no envelope
DEF ENGINE_BASE EQU 176          ; about 70 Hz at rest
DEF ENGINE_STEP EQU 14           ; per speed unit: about 134 Hz flat out
DEF EFFECT_DUTY EQU $80          ; 50%
DEF CH2 EQU 1                    ; hUGEDriver's number for channel 2

SECTION "Sfx", ROM0

; Takes channel 2 from the driver (call it after MusicPlayRace, whose
; hUGE_init unmutes every channel) and starts the engine.
SfxInit::
	ld b, CH2
	ld c, 1
	call hUGE_mute_channel
	xor a
	ld [wSfxLeft], a
	ld a, [wCrashes]
	ld [wSfxCrashes], a
	ld a, [wBumps]
	ld [wSfxBumps], a
	ld a, $FF
	ld [wSfxState], a
	jp EngineStart

SfxUpdate::
	call SfxTriggers
	ld a, [wSfxLeft]
	and a
	jr z, Engine
	dec a
	ld [wSfxLeft], a
	ret nz
	; The step is over: the next one, or back to the engine.
	ld hl, wSfxPtr
	ld a, [hli]
	ld h, [hl]
	ld l, a
	ld a, [hl]
	and a
	jr z, EngineStart
	jr PlayStep

; Starts an effect: HL = its steps.
SfxPlay:
	ld a, EFFECT_DUTY
	ldh [rNR21], a
PlayStep:
	ld a, [hli]
	ld [wSfxLeft], a
	ld a, [hli]
	ldh [rNR22], a
	ld a, [hli]
	ldh [rNR23], a
	ld a, [hli]
	or AUD2HIGH_RESTART
	ldh [rNR24], a
	ld a, l
	ld [wSfxPtr], a
	ld a, h
	ld [wSfxPtr + 1], a
	ret

EngineStart:
	xor a
	ld [wSfxLeft], a
	ld a, ENGINE_DUTY
	ldh [rNR21], a
	ld a, ENGINE_VOLUME
	ldh [rNR22], a
	call EngineFrequency
	ld a, l
	ldh [rNR23], a
	ld a, h
	or AUD2HIGH_RESTART
	ldh [rNR24], a
	ret

; Follows the speed without retriggering (no click).
Engine:
	call EngineFrequency
	ld a, l
	ldh [rNR23], a
	ld a, h
	ldh [rNR24], a
	ret

; HL = ENGINE_BASE + speed * ENGINE_STEP.
EngineFrequency:
	ld a, [wSpeed]
	ld e, a
	ld d, 0
	ld hl, ENGINE_BASE
	ld b, ENGINE_STEP
.mul
	add hl, de
	dec b
	jr nz, .mul
	ret

; What happened since the last frame, newest event winning.
SfxTriggers:
	; Countdown: a beep as each of 3, 2, 1 shows.
	ld a, [wState]
	cp STATE_READY
	jr nz, .notReady
	ld a, [wReadyTimer]
	cp 120
	jr z, .beep
	cp 80
	jr z, .beep
	cp 40
	jr nz, .changes
.beep
	ld hl, SfxBeep
	jp SfxPlay
.notReady
	; A state change: the start, the finish or time up.
	ld b, a
	ld a, [wSfxState]
	cp b
	jr z, .changes
	ld a, b
	ld [wSfxState], a
	ld hl, SfxGo
	cp STATE_RACE
	jp z, SfxPlay
	ld hl, SfxFinish
	cp STATE_WON
	jp z, SfxPlay
	ld hl, SfxTimeUp
	jp SfxPlay
.changes
	ld a, [wCrashes]
	ld hl, wSfxCrashes
	cp [hl]
	jr z, .noCrash
	ld [hl], a
	ld hl, SfxCrash
	jp SfxPlay
.noCrash
	ld a, [wBumps]
	ld hl, wSfxBumps
	cp [hl]
	ret z
	ld [hl], a
	ld hl, SfxBump
	jp SfxPlay

MACRO step                      ; frames, NR22, frequency
	db \1, \2, LOW(\3), HIGH(\3)
ENDM

SfxBeep:
	step 10, $A2, 1750          ; A4
	db 0
SfxGo:
	step 24, $F3, 1899          ; A5
	db 0
SfxBump:
	step 3, $C1, 900
	step 5, $81, 700
	db 0
SfxCrash:
	step 4, $F1, 600
	step 4, $D1, 500
	step 4, $B1, 400
	step 10, $91, 300
	db 0
SfxFinish:
	step 6, $D2, 1797           ; C5
	step 6, $D2, 1849           ; E5
	step 6, $D2, 1881           ; G5
	step 24, $F3, 1923          ; C6
	db 0
SfxTimeUp:
	step 10, $C2, 1750
	step 10, $C2, 1650
	step 24, $C3, 1546
	db 0

SECTION "Sfx variables", WRAM0
wSfxLeft: db                    ; frames left in this step; 0: the engine
wSfxPtr: dw                     ; the next step
wSfxCrashes: db
wSfxBumps: db
wSfxState: db
