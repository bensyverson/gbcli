; Music: hUGEDriver (src/hUGEDriver.asm) playing assets/music/*.song, which
; `gbcli build` converts to song_title and song_stage.
;
; Both songs tick once a frame from the VBlank handler (neither turns the
; timer on). The stage song leaves channel 4 silent, and MusicStage mutes it
; in the driver too, so the sound effects (sfx.asm) own it during play.
;
; Cost: one tick takes about 2,500-5,900 T (4-8% of a frame).

include "hardware.inc"

; hUGEDriver writes wave RAM by hardware.inc 4's name for it, which the
; project's hardware.inc 6 calls AUD3WAVERAM; this gives the driver its name.
DEF _AUD3WAVERAM EQU AUD3WAVERAM
EXPORT _AUD3WAVERAM

DEF SFX_CHANNEL EQU 3                 ; hUGE's numbering: channel 4, noise
EXPORT SFX_CHANNEL

SECTION "Music state", WRAM0

wMusicOn: db                          ; nonzero once a song has started

SECTION "Music", ROM0

; Turns the sound on. Call it once, before the first song.
MusicInit::
	ld a, AUDENA_ON
	ldh [rNR52], a
	ld a, $FF                         ; every channel to both speakers
	ldh [rNR51], a
	ld a, $77                         ; full volume, left and right
	ldh [rNR50], a
	xor a
	ld [wMusicOn], a
	ret

; Starts the title theme on all four channels.
MusicTitle::
	ld hl, song_title
	jr MusicStart

; Starts the stage theme and hands channel 4 to the sound effects.
MusicStage::
	ld hl, song_stage
	call MusicStart
	ld b, SFX_CHANNEL
	ld c, 1
	jp hUGE_mute_channel

MusicStart:
	call hUGE_init                    ; also unmutes every channel
	ld a, 1
	ld [wMusicOn], a
	ret

; One driver tick. Called by the VBlank handler; keeps every register but
; AF and HL, which the handler saves.
MusicVBlank::
	ld a, [wMusicOn]
	and a
	ret z
	push bc
	push de
	call hUGE_dosound
	pop de
	pop bc
	ret
