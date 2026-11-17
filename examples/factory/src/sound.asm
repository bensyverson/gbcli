; PLACEHOLDER sound for the playtest build: one short noise burst for the
; buzz, so a refused press can be heard. Factory 3 replaces this with the
; real effects and music.

include "hardware.inc"

SECTION "Sound", ROM0

; Sound on, both speakers, full volume. Call once at start-up.
SoundInit::
	ld a, AUDENA_ON
	ldh [rNR52], a
	ld a, $FF
	ldh [rNR51], a
	ld a, $77
	ldh [rNR50], a
	ret

; The buzz: a low noise burst of about 0.1 s on channel 4.
SoundBuzz::
	ld a, 58                    ; length: (64 - 58) / 256 s
	ldh [rNR41], a
	ld a, $C1                   ; volume 12, fading
	ldh [rNR42], a
	ld a, $67                   ; a low, rough noise
	ldh [rNR43], a
	ld a, $C0                   ; trigger, stop at the length
	ldh [rNR44], a
	ret
