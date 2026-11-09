# Example games

Original Game Boy games built entirely by AI agents with gbcli, as a test of the tool: the code, the pixel art, the music and the checks are all theirs. Each agent worked like any gbcli user, from `gbcli help` and this repo's README, never from gbcli's source. Each game is a vertical slice: one level or track, a title screen, music, and a way to win and lose. They run on the original Game Boy and, built with `--gbc`, in color on a Game Boy Color.

| Folder | Game | What it shows |
|---|---|---|
| `racer/` | Switchback: a pseudo-3D road race against the clock | Curves and hills drawn by changing the scroll registers on every scanline, in HBlank |
| `shooter/` | Vesper Swarm, a bullet-heavy shooter | 30+ sprites at once by multiplexing, and parallax from mid-frame scroll splits |

## Build and play one

You need gbcli and RGBDS (see the [README](../README.md)).

```sh
cd examples/racer
gbcli build                      # or: gbcli build --gbc, for color
gbcli run build/racer.gb --session play --frames 600 --shot shot.png
```

Each game's own README says how to play it and gives the gbcli commands that check its claims. The plan and the agents' developer-experience notes are in [project/](../project/) (`2026-11-08-example-games-plan.md`, `2026-11-08-dx-*.md`).
