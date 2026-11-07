# {{name}}

A Game Boy program in RGBDS assembly, made with `gbcli new`.

## Build and run

```sh
gbcli build
gbcli run build/{{name}}.gb --session {{name}} --frames 120 --shot first.png
```

`gbcli doctor` checks that the pinned RGBDS is on `PATH`.

## How `gbcli build` builds it

There is no build file: the layout is the build.

- Every `.asm` under `src/` is assembled by rgbasm, with `include/` on the
  include path, into `build/obj/`.
- rgblink links the objects into `build/{{name}}.gb`, with symbols in
  `build/{{name}}.sym` and the memory map in `build/{{name}}.map`.
- rgbfix fixes the header: logo, checksums, padding with `$FF`.
- PNGs in `assets/bg/` and `assets/sprites/` are checked against the
  hardware's limits and converted by rgbgfx into `build/images/`, with
  labels for the program in `images.inc`: `gbcli help images`.
- Every song in `assets/music/` (gbcli song text `.song`, or a hUGETracker
  `.uge`) becomes `build/music/<name>.asm`, hUGEDriver data with the label
  `song_<name>`, assembled with the rest.

Errors come back with their file and line. `gbcli build --help` lists the
flags that change the layout (`--src`, `--include`, `--out`).

## The edit loop

```sh
gbcli run --session {{name}} --frames 60 --checkpoint start   # keep a state
# … edit src/ …
gbcli build --session {{name}} --restore start               # rebuild, reload
gbcli measure VBlank..VBlank.done --session {{name}}         # cycles per call
```

`--restore` loads the checkpoint into the rebuilt ROM only when nothing it
was running has moved; otherwise it says what moved.

## What is here

- `src/main.asm`: the header, the VBlank handler and the program.
- `src/oam_dma.asm`: the OAM DMA routine, copied to HRAM at startup.
- `include/hardware.inc`: gbdev's hardware.inc v6.0.0 (CC0).
- `assets/music/theme.song`: the song, as text (`gbcli help music`).
- `src/music.asm`: starts the song and ticks the driver, once a frame or
  from the timer when the song turns it on. To play another song, change
  `song_theme` there.
- `src/hUGEDriver.asm`, `include/hUGE.inc`, `include/hUGE_note_table.inc`:
  hUGEDriver (public domain), the player hUGETracker songs are written for.
  It costs about 1.9 KB of ROM0, 100 bytes of WRAM and 2,500-5,900 cycles a
  tick.

## Hear the music

```sh
gbcli run build/{{name}}.gb --session {{name}} --frames 600 --audio-log --piano roll.png
gbcli run build/{{name}}.gb --session {{name}} --frames 600 --wav music.wav --channels each
```
