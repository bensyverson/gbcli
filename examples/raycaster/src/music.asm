; Music: hUGEDriver (src/hUGEDriver.asm) playing assets/music/*.song.
;
; `gbcli build` converts every song under assets/music to build/music/
; <name>.asm, whose label song_<name> is the song to hand MusicPlay. Both
; songs here tick once a frame (`timer off`), from the VBlank handler, after
; the VRAM work; and both leave channel 1 silent, which the driver is also
; told to leave alone, for the sound effects (src/sfx.asm).
;
; Cost: one tick takes about 2,500-5,900 T (4-8% of a frame).

include "hardware.inc"

; hUGEDriver writes wave RAM by hardware.inc 4's name for it, which the
; project's hardware.inc 6 calls AUD3WAVERAM; this gives the driver its name.
DEF _AUD3WAVERAM EQU AUD3WAVERAM
EXPORT _AUD3WAVERAM

DEF SFX_CHANNEL EQU 0           ; channel 1, as hUGE_mute_channel numbers them

SECTION "Music", ROM0

; Turns the sound on, every channel to both speakers at full volume.
SoundInit::
	ld a, AUDENA_ON
	ldh [rNR52], a
	ld a, $FF
	ldh [rNR51], a
	ld a, $77
	ldh [rNR50], a
	xor a
	ld [wMusicOn], a
	ret

; Starts the song at HL (a song_<name> label) from its beginning.
MusicPlay::
	xor a
	ld [wMusicOn], a            ; no tick while the driver is set up
	call hUGE_init
	ld b, SFX_CHANNEL
	ld c, 1
	call hUGE_mute_channel
	ld a, 1
	ld [wMusicOn], a
	ret

; One driver tick, from the VBlank handler. Keeps nothing; the handler
; saves every register.
MusicTick::
	ld a, [wMusicOn]
	and a
	ret z
	jp hUGE_dosound

SECTION "Music variables", WRAM0
wMusicOn: db
