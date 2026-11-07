; Music: hUGEDriver (src/hUGEDriver.asm) playing assets/music/theme.song.
;
; `gbcli build` converts every song under assets/music (gbcli song text,
; or a hUGETracker .uge) to build/music/<name>.asm, whose label
; song_<name> is the song to hand hUGE_init. To play another song, change
; song_theme below to its label; `gbcli build` prints each song's label.
;
; The song says how often the driver must tick: once a frame (from the
; VBlank handler, MusicVBlank) or, when the song turns the timer on, from
; the timer interrupt at 4096 / (256 - TMA) Hz. The converted song exports
; song_<name>_TIMER (1 or 0) and song_<name>_TMA for this.
;
; Cost: the driver is about 1.9 KB of ROM0 and 100 bytes of WRAM, and one
; tick takes about 2,500-5,900 cycles (4-8% of a frame), more on busy rows.

include "hardware.inc"

; hUGEDriver writes wave RAM by hardware.inc 4's name for it, which the
; project's hardware.inc 6 calls AUD3WAVERAM; this gives the driver its name.
DEF _AUD3WAVERAM EQU AUD3WAVERAM
EXPORT _AUD3WAVERAM

SECTION "Timer vector", ROM0[INT_HANDLER_TIMER]
	jp MusicTimer

SECTION "Music", ROM0

; Turns the sound on and starts the song. Call it with interrupts off, after
; IE is set: it adds the timer interrupt when the song wants it.
MusicInit::
	ld a, AUDENA_ON
	ldh [rNR52], a
	ld a, $FF                   ; every channel to both speakers
	ldh [rNR51], a
	ld a, $77                   ; full volume, left and right
	ldh [rNR50], a
	ld hl, song_theme
	call hUGE_init
	ld a, song_theme_TIMER
	and a
	ret z
	ld a, song_theme_TMA
	ldh [rTMA], a
	ldh [rTIMA], a
	ld a, TAC_START | TAC_4KHZ
	ldh [rTAC], a
	ldh a, [rIE]
	or IE_TIMER
	ldh [rIE], a
	ret

; One driver tick, unless the timer drives the song. Called by the VBlank
; handler; keeps every register but AF and HL, which the handler saves.
MusicVBlank::
	ld a, song_theme_TIMER
	and a
	ret nz
	push bc
	push de
	call hUGE_dosound
	pop de
	pop bc
	ret

MusicTimer:
	push af
	push bc
	push de
	push hl
	call hUGE_dosound
	pop hl
	pop de
	pop bc
	pop af
	reti
