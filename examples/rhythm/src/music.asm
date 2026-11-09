; Music and the game's clock: hUGEDriver playing assets/music/lantern.song.
;
; The clock IS the driver's tick count. wNow counts VBlanks from the start of
; a play; the first PREROLL of them are silent (the first notes fall into
; view), and every later one calls hUGE_dosound exactly once, until the
; song's last tick. So clock time wNow is driver tick wNow - PREROLL, the
; chart (src/chart.asm) is in the same units, and a press is judged against
; the tick the driver actually played: nothing is ever rounded to frames.
;
; The song keeps `timer off`: with the timer on, a tick is not a whole
; number of frames, and a timer tick that lands just before VBlank holds the
; VBlank handler (and its OAM DMA) back (`gbcli help music`).

include "hardware.inc"
include "game.inc"

; hUGEDriver writes wave RAM by hardware.inc 4's name for it, which the
; project's hardware.inc 6 calls AUD3WAVERAM; this gives the driver its name.
DEF _AUD3WAVERAM EQU AUD3WAVERAM
EXPORT _AUD3WAVERAM

ASSERT song_lantern_TIMER == 0, "the clock needs a VBlank-ticked song: `timer off`"
ASSERT song_title_TIMER == 0, "MusicVBlank ticks the title loop once a frame: `timer off`"

SECTION "Music", ROM0

; Starts the title loop. LCD off (so no VBlank tick lands mid-init).
MusicPlayTitle::
	ld hl, song_title
	ld a, MUSIC_TITLE
	jr MusicPlay

; Starts the game's song and resets the clock to just before tick 0 of the
; preroll. LCD off.
MusicPlayGame::
	ld hl, song_lantern
	ld a, MUSIC_PLAY
	; fall through

; Turns the sound on and starts song HL in mode A (MUSIC_PLAY or
; MUSIC_TITLE), with channel 2 muted in the driver for the effects.
MusicPlay:
	ld [wMusicMode], a
	ld a, AUDENA_ON
	ldh [rNR52], a
	ld a, $FF                   ; every channel to both speakers
	ldh [rNR51], a
	ld a, $77                   ; full volume, left and right
	ldh [rNR50], a
	call hUGE_init
	ld b, 1                     ; channel 2
	ld c, 1                     ; muted: the effects' (src/sfx.asm)
	call hUGE_mute_channel
	ld a, $FF                   ; the first VBlank's increment makes it 0
	ld [wNow], a
	ld [wNow + 1], a
	xor a
	ld [wSongOver], a
	ld [wMusicOff], a
	ret

; Stops the song and the clock where they are (a failed play).
MusicStop::
	ld a, 1
	ld [wMusicOff], a
	xor a
	ldh [rNR51], a              ; no channel reaches a speaker
	ret

; One clock tick: called by the VBlank handler once a frame. Plays driver
; tick wNow - PREROLL when the song is under way, and silences the sound
; after its last tick. Keeps every register but AF and HL.
MusicVBlank::
	ld a, [wMusicOff]
	and a
	ret nz
	ld a, [wMusicMode]
	cp MUSIC_TITLE
	jr nz, .clock
	push bc
	push de
	call hUGE_dosound           ; the title loop: no clock, no end
	pop de
	pop bc
	ret
.clock
	ld hl, wNow
	inc [hl]
	ld a, [hli]
	jr nz, .noCarry
	inc [hl]
.noCarry
	; a = low byte; [hl] = high byte. Before PREROLL: silent.
	ld l, a
	ld a, [wNow + 1]
	ld h, a
	ld a, l
	sub LOW(PREROLL)
	ld l, a
	ld a, h
	sbc HIGH(PREROLL)
	ret c                       ; still in the preroll
	ld h, a                     ; hl = driver tick
	ld a, l
	sub LOW(SONG_END_TICK)
	ld a, h
	sbc HIGH(SONG_END_TICK)
	jr nc, .over
	push bc
	push de
	call hUGE_dosound
	pop de
	pop bc
	ret
.over
	ld a, [wSongOver]
	and a
	ret nz
	inc a
	ld [wSongOver], a
	xor a
	ldh [rNR51], a              ; no channel reaches a speaker
	ret

SECTION "Clock", WRAM0

; Clock time of the tick just played: PREROLL + the driver's tick.
wNow:: dw
; Nonzero once the song's last tick has played.
wSongOver:: db
; Nonzero once MusicStop has stopped the song and the clock.
wMusicOff:: db
; MUSIC_PLAY (the clock runs) or MUSIC_TITLE (the loop runs free).
wMusicMode:: db
