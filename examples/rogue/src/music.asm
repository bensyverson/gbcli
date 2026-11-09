; Music: hUGEDriver (src/hUGEDriver.asm) playing assets/music/title.song on
; the title screen and assets/music/dungeon.song below. `gbcli build`
; converts each to build/music/<name>.asm, label song_<name>. Both songs tick
; once a frame (`timer off`), from the VBlank handler.
;
; Both songs leave channel 2 empty, and channel 2 is muted in the driver so
; the sound effects (sfx.asm) have it to themselves.
;
; Cost: about 2,500-5,900 cycles a tick (4-8% of a frame).

include "hardware.inc"

; hUGEDriver writes wave RAM by hardware.inc 4's name for it.
DEF _AUD3WAVERAM EQU AUD3WAVERAM
EXPORT _AUD3WAVERAM

DEF SFX_CHANNEL EQU 1               ; hUGE_mute_channel counts from 0

SECTION "Music", ROM0

; Turns the sound on and starts the title theme. Interrupts off.
MusicInit::
	ld a, AUDENA_ON
	ldh [rNR52], a
	ld a, $FF                   ; every channel to both speakers
	ldh [rNR51], a
	ld a, $77                   ; full volume, left and right
	ldh [rNR50], a
	ld hl, song_title
	; fall through

; Starts the song at HL, giving channel 2 back to the sound effects.
MusicPlay::
	push hl
	ld hl, wMusicOn
	ld [hl], 0                  ; no tick while the driver resets
	pop hl
	call hUGE_init
	ld b, SFX_CHANNEL
	ld c, 1
	call hUGE_mute_channel
	ld a, 1
	ld [wMusicOn], a
	ret

; Starts the dungeon theme.
MusicDungeon::
	ld hl, song_dungeon
	jr MusicPlay

; Starts the title theme.
MusicTitle::
	ld hl, song_title
	jr MusicPlay

; One driver tick. Called by the VBlank handler; keeps every register but AF
; and HL, which the handler saves.
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

SECTION "Music variables", WRAM0
wMusicOn: db
