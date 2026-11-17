# Example games

Original Game Boy games built entirely by AI agents with gbcli, as a test of the tool: the code, the pixel art, the music and the checks are all theirs. Five are the agents' own designs; Teddy & Toys is Ben's, built by agents from his sketch and playtest notes. Each agent worked like any gbcli user, from `gbcli help` and this repo's README, never from gbcli's source. Each game is a vertical slice: one level or track, a title screen, music, and a way to win and lose (Teddy & Toys plays for a personal best instead). They run on the original Game Boy and, built with `--gbc`, in color on a Game Boy Color.

| Folder | Game | What it shows |
|---|---|---|
| `racer/` | Switchback: a pseudo-3D road race against the clock ([play: `switchback.gb`](racer/switchback.gb)) | Curves and hills drawn by changing the scroll registers on every scanline, in HBlank |
| `shooter/` | Vesper Swarm, a bullet-heavy shooter ([play: `vesper-swarm.gb`](shooter/vesper-swarm.gb)) | 30+ sprites at once by multiplexing, and parallax from mid-frame scroll splits |
| `rhythm/` | Lantern Drum, a rhythm game ([play: `lantern-drum.gb`](rhythm/lantern-drum.gb)) | Notes judged against the music driver's own tick, so the chart can't drift from the song; checked against the audio log |
| `rogue/` | Lanternfall, a turn-based dungeon crawl down five generated floors ([play: `lanternfall.gb`](rogue/lanternfall.gb)) | Dungeons generated in assembly from a seed, on a map bigger than the background, streamed in as it scrolls |
| `raycaster/` | Lantern, a first-person maze: find three keys before the lamp runs dry ([play: `lantern.gb`](raycaster/lantern.gb)) | A view raycast into a tile buffer and copied to VRAM in VBlank only, sped up 4.9x in profile-driven rounds, checked pixel for pixel against a host reference |
| `factory/` | Teddy & Toys: snap matching halves into toys between two factory belts before the clock runs out ([play: `teddy-and-toys.gb`](factory/teddy-and-toys.gb)) | Two belts scrolling opposite ways by mid-frame scroll splits, under a key column in the window that never scrolls; every gameplay number in one tuning file, rechecked by a 36-rule check in about a second |

## Play one

Each game folder has a ready-to-play ROM (linked above): one file that runs in color on a Game Boy Color and in four shades on the original Game Boy. Open it in any Game Boy emulator. `scripts/examples/build-roms.sh` rebuilds them all.

## Build one

You need gbcli and RGBDS (see the [README](../README.md)).

```sh
cd examples/racer
gbcli build                      # or: gbcli build --gbc, for color (build/gbc/racer.gbc)
gbcli run build/racer.gb --session play --frames 600 --shot shot.png
```

Each game's own README says how to play it and gives the gbcli commands that check its claims. The plan and the agents' developer-experience notes are in [project/](../project/) (`2026-11-08-example-games-plan.md`, `2026-11-0*-dx-*.md`).
