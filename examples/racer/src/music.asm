; Music: hUGEDriver (src/hUGEDriver.asm) playing the songs in assets/music/
; (written by tools/songs.py): song_title on the title screen, song_race in
; the race. The race song leaves channel 2 empty for the sound effects
; (src/sfx.asm mutes it in the driver).
;
; The timer vector is the road's (src/road.asm), so every song here must
; tick once a frame, from VBlank. The VBlank handler calls MusicVBlank with
; interrupts on, so the road's HBlank interrupts keep their timing while
; the driver works.
;
; Cost: the driver is about 1.9 KB of ROM0 and 100 bytes of WRAM, and one
; tick takes about 2,500-5,900 cycles (4-8% of a frame), more on busy rows.

include "hardware.inc"

; hUGEDriver writes wave RAM by hardware.inc 4's name for it, which the
; project's hardware.inc 6 calls AUD3WAVERAM; this gives the driver its name.
DEF _AUD3WAVERAM EQU AUD3WAVERAM
EXPORT _AUD3WAVERAM

SECTION "Music", ROM0

; Turns the sound on and starts the title song.
MusicPlayTitle::
	ld hl, song_title
	jr MusicPlay

; Turns the sound on and starts the race song.
MusicPlayRace::
	ld hl, song_race
	; fall through

; Turns the sound on and starts the song at HL.
MusicPlay:
	ld a, AUDENA_ON
	ldh [rNR52], a
	ld a, $FF                   ; every channel to both speakers
	ldh [rNR51], a
	ld a, $77                   ; full volume, left and right
	ldh [rNR50], a
	call hUGE_init
	ASSERT song_title_TIMER == 0 && song_race_TIMER == 0, "songs must tick from VBlank: the timer vector is the road's"
	ret

; One driver tick. Keeps every register but AF and HL, which the caller saves.
MusicVBlank::
	push bc
	push de
	call hUGE_dosound
	pop de
	pop bc
	ret
