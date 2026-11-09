# Lantern Drum

A Game Boy rhythm game: paper lanterns fall toward four drums, and you hit each drum as its lantern lands, in time with the song.

One original 75-second song, written for the game, with 165 notes to hit, a score and combo, a life meter that empties on misses, and a results screen with a grade from S to F. It runs on the original Game Boy in four shades and, built with `--gbc`, in color on a Game Boy Color (a color for each lane). Built by an agent with gbcli, as a user; the developer-experience notes are in [project/2026-11-09-dx-rhythm.md](../../project/2026-11-09-dx-rhythm.md).

Play the ROM: [`lantern-drum.gb`](lantern-drum.gb), in any Game Boy emulator.

## How to play

- **START** on the title screen starts the song.
- Four lanes, left to right: **LEFT**, **RIGHT**, **B**, **A**. Press a lane's button as its lantern reaches the drum.
- Within 2 frames of the beat is PERFECT (300 points, and a little life back); within 6 is GOOD (100); a lantern 7 frames past its drum is a MISS (the combo breaks and the meter drops a cell). Six misses in a row from a full meter fail the song.
- The grade: S is no miss and at least 90% perfect; then by points (2 a perfect, 1 a good, out of 2 a note): A 90%, B 75%, C 60%, else D. A failed song is F.
- **START** on the results screen goes back to the title.

## What's technically notable

- **The game keeps time by the music driver's own tick, not a separate timer.** `src/music.asm` counts the frames since a play began; after a 64-tick preroll (while the first lanterns fall into view) each one calls hUGEDriver exactly once. So the count is the driver's tick, and the chart (`assets/music/lantern.chart`, in driver ticks) is judged against the tick the driver actually played. Nothing is ever converted from frames or rounded, so the chart can't drift from the audio. The song keeps `timer off` (asserted when it assembles): with the timer on, a driver tick is not a whole number of frames.
- **The chart is made from the song.** `tools/chart.py` reads the song text (gbcli song text) and makes one note for each lead (channel 1) onset, giving it a lane from the melody's shape: up a step moves right, down moves left. Change the song and the chart follows.
- **The sound effects have a channel to themselves.** Both songs leave channel 2 empty and the driver is told to leave it alone, so a hit or miss sound never cuts a note of the music, whenever it lands.
- **Everything is text and regenerates byte for byte**: the songs (`tools/compose.py`), the chart (`tools/chart.py`), the font (`tools/font.py`), and the playfield tiles and title screen (`tools/art.py`), all as gbcli pixel text in `assets/`.

## The claims, and the commands that check them

From this folder, with gbcli and RGBDS 1.0.4 on `PATH` (`gbcli doctor`). Each run is deterministic, so you get these numbers exactly.

**Both builds are clean** (no warnings):

```sh
gbcli build                                  # images ... errors=0 warnings=0, build ... errors=0 warnings=0
gbcli build --gbc --out build/rhythm-gbc.gb  # the same, target=gbc
```

**Scripted runs.** The game's clock starts in frame 63 when START is pressed on frame 60:

```sh
gbcli run build/rhythm.gb --session cal --from boot --keys 'START@60 -START@63' --until 'u16[wNow] == 0'
#   stop ... frame=63
python3 -I tools/keys.py --start 63 --shift 0 > build/perfect.keys   # every note on its beat
python3 -I tools/keys.py --start 63 --shift 4 > build/late.keys      # every note 4 frames late
```

**A perfect run scores every note perfect and grades S, with no lag frames:**

```sh
gbcli run build/rhythm.gb --session clear --from boot --frames 4700 --keys-file build/perfect.keys \
    --frame-budget --read '[wCleared]' --read '[wGrade]' --read '[wPerfect]'
#   wCleared=1 wGrade=0 (S) wPerfect=165
#   frame_budget frames=4700 lag=0 ... active_max=20232
```

The same on a Game Boy Color: `gbcli build --gbc --out build/rhythm-gbc.gb`, then the run with `build/rhythm-gbc.gb --session clear-gbc --model gbc` in place of `build/rhythm.gb --session clear`: wCleared=1, wGrade=0, wPerfect=165, `lag=0`.

**A late run scores every note good; no input fails the song:**

```sh
gbcli run build/rhythm.gb --session late --from boot --frames 4700 --keys-file build/late.keys \
    --read '[wPerfect]' --read '[wGood]' --read '[wMiss]' --read '[wGrade]'
#   wPerfect=0 wGood=165 wMiss=0 wGrade=4 (D)
gbcli run build/rhythm.gb --session fail --from boot --frames 1500 --keys 'START@60 -START@63' \
    --read '[wCleared]' --read '[wGrade]' --read '[wMiss]' --read '[wMusicOff]'
#   wCleared=0 wGrade=5 (F) wMiss=6 wMusicOff=1
```

**No drift: every note the music plays lands on its chart tick, to the last one:**

```sh
gbcli run build/rhythm.gb --session sync --from boot --frames 4700 --keys-file build/perfect.keys \
    --audio-log --audio-channel 1 --json > build/sync.json
python3 -I tools/sync.py build/sync.json --from-frame 63
#   chart notes=165 heard onsets=165 ...
#   residual min=-16 max=+16 elapsed ticks (spread 0.0002 frames, tolerance 0.125)
#   last note: #164 tick 4256 (order 09 row 32) residual -16 elapsed ticks
#   sync ok
```

`sync.py` takes every channel-1 trigger from the audio log and checks its pitch, and its time against the first note's time plus its distance in ticks, at a frame (140,448 of the log's 2^23 Hz ticks) a tick. The worst error over the whole song is 16 of those ticks, 8 CPU cycles.

**The effects are on channel 2 alone, from one routine:**

```sh
python3 -I tools/keys.py --start 63 --skip 5 > build/skip5.keys      # miss every fifth note
gbcli run build/rhythm.gb --session fx --from boot --frames 1100 --keys-file build/skip5.keys \
    --audio-summary --audio-channel 2 --read '[wPerfect]' --read '[wMiss]'
#   wPerfect=18 wMiss=4
#   audio_writer pc=$00D7 writes=22 registers=NR21  (and NR22, NR23, NR24: 22 each, one per sound)
#   audio_writer pc=$0421 writes=2 registers=NR22   (the driver silencing channel 2 as each song starts)
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

`python3 -I tools/windows.py` plays the whole chart 2, 3, 6 and 7 frames early and late, to show the judgment windows' edges: ±2 all perfect, ±3 and ±6 all good, ±7 six misses and a failed song.

## The files

- `src/main.asm`: startup, the frame loop, the VBlank handler, input, the title screen.
- `src/music.asm`: the clock and the two songs. `src/play.asm`: judging, scoring, the lanterns, the HUD. `src/results.asm`: the grade and the results screen. `src/sfx.asm`: the hit and miss sounds. `src/text.asm`: text and numbers.
- `src/chart.asm`, `include/chart.inc`, `assets/music/lantern.chart`: made by `tools/chart.py`; don't edit them.
- `assets/music/*.song`: by `tools/compose.py`. `assets/tiles/*.pixels`, `assets/bg/title.pixels`: by `tools/font.py` and `tools/art.py`. `assets/palettes.txt`: the shades.
- `src/hUGEDriver.asm`, `include/hUGE*.inc`: hUGEDriver (public domain); `include/hardware.inc`: gbdev's hardware.inc (CC0); `src/oam_dma.asm`: from `gbcli new`'s starter.
