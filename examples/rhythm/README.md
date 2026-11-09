# Lantern Drum

A Game Boy rhythm game: paper lanterns fall toward four drums, and you hit each drum as its lantern lands, in time with the song.

One original 75-second song, written for the game, with 184 notes to hit, a score and combo, a life meter that empties on misses, and a results screen with a grade from S to F. It opens slowly, one button at a time, and builds to chords and off-beat notes by the end. It runs on the original Game Boy in four shades and, built with `--gbc`, in color on a Game Boy Color (a color for each lane). Built by an agent with gbcli, as a user; the developer-experience notes are in [project/2026-11-09-dx-rhythm.md](../../project/2026-11-09-dx-rhythm.md).

Play the ROM: [`lantern-drum.gb`](lantern-drum.gb), in any Game Boy emulator.

## How to play

- **START** on the title screen starts the song.
- Four lanes, left to right: **LEFT**, **RIGHT**, **B**, **A**. Each lane's button is drawn under its drum (an arrow, B, A). Press it as the lane's lantern reaches the drum.
- The song opens with a lesson: eight single lanterns, two beats apart, two in each lane from left to right. A miss there costs no life. Then the notes follow the melody across the lanes and get closer together; chords (two lanterns at once, always one arrow lane and one button lane, a thumb each) start in the first chorus, and off-beat notes in the second verse.
- Within 3 frames of the beat (50 ms) is PERFECT: 300 points and 2 life back. Within 8 frames (133 ms) is GOOD: 100 points and 1 life back. A lantern 9 frames past its drum is a MISS: the combo breaks and the meter loses 2 of its 48.
- The grade: S is no miss and at least 90% perfect; then by points (2 a perfect, 1 a good, out of 2 a note): A 90%, B 75%, C 60%, else D. A song failed (the meter empty) is F.
- **START** on the results screen goes back to the title.

### Why it forgives this much

Play-testing showed a first-time player was failed within seconds. Three things changed, each for a reason:

- **Wider windows.** The old ones (2 and 6 frames) are tight for someone still learning the buttons. 50 ms for a perfect and 133 ms for a good are generous but still reward timing, and a good window never reaches the next note of its lane: the closest two notes in one lane are 14 frames apart (`tools/chart.py` refuses a chart with closer ones).
- **A meter that comes back.** Before, only a perfect gave life back, and six misses in a row failed the song. Now a miss costs 2 of 48, a good gives 1 back and a perfect 2. So two goods pay back a miss, and **a player who hits two notes in three, however early or late, clears the song** (the sloppy run below). Someone who stops pressing still fails, after 24 misses in a row, and so does someone who hits only half the notes (about 47 seconds in).
- **A lesson that can't fail you.** The first eight notes teach the four buttons; missing them breaks the combo and costs nothing else.

## What's technically notable

- **The game keeps time by the music driver's own tick, not a separate timer.** `src/music.asm` counts the frames since a play began; after a 64-tick preroll (while the first lanterns fall into view) each one calls hUGEDriver exactly once. So the count is the driver's tick, and the chart (`assets/music/lantern.chart`, in driver ticks) is judged against the tick the driver actually played. Nothing is ever converted from frames or rounded, so the chart can't drift from the audio. The song keeps `timer off` (asserted when it assembles): with the timer on, a driver tick is not a whole number of frames.
- **The chart is made from the song, and every note is one the song plays.** `tools/chart.py` reads the song text (gbcli song text) and takes the lead's (channel 1) note onsets, and for a chord the bass's (channel 3) onset on the same tick. A table of stages, one per section of the song, says which onsets to take: every second beat at first, then every beat, then every note; chords on a bar's first beat, then every second beat, then every note of the ending. A note's lane follows the melody (up a step moves right, down moves left). Change the song and the chart follows. `python3 -I tools/chart.py --density` prints the ramp:

  ```
  order notes chords shortest_gap_ticks
      0     8      0                 56
      1     8      0                 56
      2    16      0                 28
      3    17      4                 28
      4    21      4                 14
      5    29      4                 14
      6    20      8                 28
      7    24      4                 14
      8    33      8                 14
      9     8      4                 56
  ```

  Order 0 is the lesson, a lane at a time; the first chords come in order 3 (the first chorus), off-beat eighths in order 4, the bridge (order 6) is a breather with more chords, the 3-row syncopation arrives in order 7, and in the ending every note is a chord. A gap of 56 ticks is almost a second.
- **The sound effects have a channel to themselves.** Both songs leave channel 2 empty and the driver is told to leave it alone, so a hit or miss sound never cuts a note of the music, whenever it lands.
- **Everything is text and regenerates byte for byte**: the songs (`tools/compose.py`), the chart (`tools/chart.py`), the font and the lane labels' arrows (`tools/font.py`), and the playfield tiles and title screen (`tools/art.py`), all as gbcli pixel text in `assets/`.

## The claims, and the commands that check them

From this folder, with gbcli and RGBDS 1.0.4 on `PATH` (`gbcli doctor`). Each run is deterministic, so you get these numbers exactly. A run that plays the whole song is 4,700 frames; on a heavily loaded machine gbcli's 60-second wall guard can stop one early (`wall_guard`, not reproducible). Split it at a checkpoint and replay the same key file from boot: `--frames 2400 --checkpoint half`, then `gbcli run --session NAME --at half --frames 2300 --keys-file FILE --keys-since boot` with the same reads.

**Both builds are clean** (no warnings):

```sh
gbcli build                                  # images ... errors=0 warnings=0, build ... errors=0 warnings=0
gbcli build --gbc --out build/rhythm-gbc.gb  # the same, target=gbc
```

**Every lane shows its button, under its drum, on both machines:**

```sh
gbcli run build/rhythm.gb --session labels --from boot --frames 160 --keys 'START@60 -START@63' \
    --shot build/labels.png --crop 56,112,96,24 --read '[$9A08]' --read '[$9A0B]' --read '[$9A0E]' --read '[$9A11]'
#   [$9A08]=74 ('<')  [$9A0B]=75 ('>')  [$9A0E]=44 ('B')  [$9A11]=43 ('A'): map row 16, under the drums
gbcli run build/rhythm-gbc.gb --session labels-gbc --model gbc --from boot --frames 160 \
    --keys 'START@60 -START@63' --shot build/labels-gbc.png --crop 56,112,96,24
```

`build/labels.png` shows a black arrow, arrow, B and A under the four drums on white; `build/labels-gbc.png` shows each in its lane's lantern color (amber, teal, rose, leaf) on the night-sky blue.

**Scripted runs.** The game's clock starts in frame 63 when START is pressed on frame 60:

```sh
gbcli run build/rhythm.gb --session cal --from boot --keys 'START@60 -START@63' --until 'u16[wNow] == 0'
#   stop ... frame=63
python3 -I tools/keys.py --start 63 --shift 0 > build/perfect.keys   # every note on its beat
python3 -I tools/keys.py --start 63 --shift 6 > build/late.keys      # every note 6 frames late
python3 -I tools/keys.py --start 63 --skip 3 --jitter 6,-4,8,4,-5,7,-4,5 > build/sloppy.keys
#   every third note missed, the rest 4 to 8 frames early or late: never a perfect
```

**A perfect run scores every note perfect and grades S, with no lag frames:**

```sh
gbcli run build/rhythm.gb --session clear --from boot --frames 4700 --keys-file build/perfect.keys \
    --frame-budget --read '[wCleared]' --read '[wGrade]' --read '[wPerfect]' --read '[wState]'
#   wCleared=1 wGrade=0 (S) wPerfect=184 wState=1 (the results screen)
#   frame_budget frames=4700 lag=0 ... active_max=22568
```

The same on a Game Boy Color: the run with `build/rhythm-gbc.gb --session clear-gbc --model gbc` in place of `build/rhythm.gb --session clear`: wCleared=1, wGrade=0, wPerfect=184, wState=1, `lag=0`.

**A sloppy run still clears; a late run scores every note good:**

```sh
gbcli run build/rhythm.gb --session sloppy --from boot --frames 4700 --keys-file build/sloppy.keys \
    --read '[wCleared]' --read '[wState]' --read '[wPerfect]' --read '[wGood]' --read '[wMiss]' --read '[wLife]' --read '[wGrade]'
#   wCleared=1 wState=1 (the results screen) wPerfect=0 wGood=123 wMiss=61 wLife=46 wGrade=4 (D)
gbcli run build/rhythm.gb --session late --from boot --frames 4700 --keys-file build/late.keys \
    --read '[wPerfect]' --read '[wGood]' --read '[wMiss]' --read '[wGrade]'
#   wPerfect=0 wGood=184 wMiss=0 wGrade=4 (D)
```

**Hitting only half the notes, or none, still fails:**

```sh
python3 -I tools/keys.py --start 63 --skip 2 --shift 6 > build/half.keys
gbcli run build/rhythm.gb --session half --from boot --frames 4700 --keys-file build/half.keys \
    --until '[wMusicOff] == 1' --read '[wGood]' --read '[wMiss]' --read '[wHead]'
#   stop ... frame=2936   wGood=53 wMiss=52 wHead=105 (failed at note 105 of 184)
gbcli run build/rhythm.gb --session fail --from boot --frames 1500 --keys 'START@60 -START@63' \
    --read '[wCleared]' --read '[wGrade]' --read '[wMiss]' --read '[wMusicOff]'
#   wCleared=0 wGrade=5 (F) wMiss=32 wMusicOff=1   (8 free misses in the lesson, then 24)
```

**No drift: every note the music plays lands on its tick, to the last one, and so does every chart note:**

```sh
gbcli run build/rhythm.gb --session sync --from boot --frames 4700 --keys-file build/perfect.keys \
    --audio-log --audio-channel 1 --audio-channel 3 --json > build/sync.json
python3 -I tools/sync.py build/sync.json --from-frame 63
#   song lead onsets=173 heard=173 tick=140448.0 elapsed (1.0000 frames)
#   residual min=+0 max=+32 elapsed ticks (spread 0.0002 frames, tolerance 0.125)
#   drift slope=+0.0002 elapsed/tick, +0.0000 frames over 4256 ticks
#   last lead onset: #172 tick 4256 (order 09 row 32) residual +0 elapsed ticks
#   chart notes=184 heard at their ticks=184 (lead 148, bass 36); worst residual 3248 elapsed ticks
#   sync ok
```

`sync.py` takes every channel-1 and channel-3 trigger from the audio log. It checks each of the song's 173 lead notes, pitch and time, against the first note's time plus its distance in ticks, at a frame (140,448 of the log's 2^23 Hz ticks) a tick; the worst error over the whole song is 32 of those ticks, 16 CPU cycles. Then it finds each of the chart's 184 notes, lead or bass, heard on its channel at its tick; the worst is 3,248 ticks, the driver reaching the bass channel after the lead within the same tick (under a fortieth of a frame).

**The windows sit exactly on the driver's tick:**

```sh
python3 -I tools/windows.py
#   shift perfect good miss cleared
#      -9       0    0   32       0
#      -8       0  184    0       1
#      -4       0  184    0       1
#      -3     184    0    0       1
#      +0     184    0    0       1
#      +3     184    0    0       1
#      +4       0  184    0       1
#      +8       0  184    0       1
#      +9       0    0   32       0
```

It plays the whole chart 3, 4, 8 and 9 frames early and late, one gbcli run each, stopped at the results screen. The edges are symmetric to the frame, so a press is judged against the very tick the driver played.

**The effects are on channel 2 alone, from one routine:**

```sh
python3 -I tools/keys.py --start 63 --skip 5 > build/skip5.keys      # miss every fifth note
gbcli run build/rhythm.gb --session fx --from boot --frames 1100 --keys-file build/skip5.keys \
    --audio-summary --audio-channel 2 --read '[wPerfect]' --read '[wMiss]'
#   wPerfect=16 wMiss=3
#   audio_writer pc=$00D7 symbol=PlaySfx+$F writes=19 registers=NR21  (and NR22, NR23, NR24: 19 each, one per sound)
#   audio_writer pc=$0421 symbol=note_cut+$A writes=2 registers=NR22  (and $0427, NR24: the driver silencing channel 2 as each song starts)
gbcli sym 'PlaySfx*' --session fx                                    # PlaySfx at $00C8
```

**Title to song to results and back:**

```sh
gbcli run build/rhythm.gb --session loop --from boot --frames 4750 --keys-file build/perfect.keys \
    --keys 'START@4720 -START@4723' --read '[wState]'
#   wState=2 (the title again, after the results)
```

**The assets regenerate byte-identical** (each exits 0):

```sh
python3 -I tools/compose.py --check && python3 -I tools/chart.py --check \
    && python3 -I tools/font.py --check && python3 -I tools/art.py --check
```

## The files

- `src/main.asm`: startup, the frame loop, the VBlank handler, input, the title screen, the playfield and its lane labels.
- `src/music.asm`: the clock and the two songs. `src/play.asm`: judging, scoring, the lanterns, the HUD. `src/results.asm`: the grade and the results screen. `src/sfx.asm`: the hit and miss sounds. `src/text.asm`: text and numbers.
- `src/chart.asm`, `include/chart.inc`, `assets/music/lantern.chart`: made by `tools/chart.py`; don't edit them.
- `assets/music/*.song`: by `tools/compose.py`. `assets/tiles/*.pixels`, `assets/bg/title.pixels`: by `tools/font.py` and `tools/art.py`. `assets/palettes.txt`: the shades.
- `tools/keys.py` writes key files that play the chart; `tools/sync.py` and `tools/windows.py` check the timing.
- `src/hUGEDriver.asm`, `include/hUGE*.inc`: hUGEDriver (public domain); `include/hardware.inc`: gbdev's hardware.inc (CC0); `src/oam_dma.asm`: from `gbcli new`'s starter.
