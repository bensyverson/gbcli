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
;
; The tempo: a row lasts the song's ticks_per_row ticks (frames), which
; hUGE_init reads from the song. When the clock turns red, MusicFast asks
; for MUSIC_RED_TICKS_PER_ROW (tuning.inc) instead, and MusicNormal puts
; the song's back. The driver compares its tick count with
; hUGE_ticks_per_row for equality, so a shorter row may only start while
; the count is below it; MusicVBlank waits for that.

include "hardware.inc"
include "tuning.inc"

	ASSERT MUSIC_RED_TICKS_PER_ROW >= 1 && MUSIC_RED_TICKS_PER_ROW <= 255, "MUSIC_RED_TICKS_PER_ROW is 1-255"

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
	ld a, [hUGE_ticks_per_row]
	ld [wSongTicks], a
	xor a
	ld [wMusicFast], a
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

; Asks for the faster tempo (MUSIC_RED_TICKS_PER_ROW); MusicVBlank
; switches as soon as the driver allows.
MusicFast::
	ld a, 1
	ld [wMusicFast], a
	ret

; The song's own tempo again, now (a longer row is always safe).
MusicNormal::
	xor a
	ld [wMusicFast], a
	ld a, [wSongTicks]
	ld [hUGE_ticks_per_row], a
	ret

; One driver tick, unless the timer drives the song. Called by the VBlank
; handler; keeps every register but AF and HL, which the handler saves.
; (This song ticks from VBlank; a timer-driven one would switch tempo in
; MusicTimer instead.)
MusicVBlank::
	ld a, song_theme_TIMER
	and a
	ret nz
	ld a, [wMusicFast]
	and a
	jr z, .tick
	ld a, [hUGE_tick]
	cp MUSIC_RED_TICKS_PER_ROW
	jr nc, .tick                ; not yet: this row ends at the old tempo
	ld a, MUSIC_RED_TICKS_PER_ROW
	ld [hUGE_ticks_per_row], a
	xor a
	ld [wMusicFast], a
.tick
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

SECTION "Music variables", WRAM0
wSongTicks: db                     ; the song's own ticks_per_row
wMusicFast: db                     ; 1: switch to MUSIC_RED_TICKS_PER_ROW when the driver allows
