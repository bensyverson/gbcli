# gbcli

A tool that helps AI assistants build and test Game Boy games.

gbcli runs a Game Boy emulator in the background with no window, so an AI agent can load a game, save and restore moments in it, pause on any instruction, read every byte of memory, measure exactly how long code takes, and look at the screen or listen to the sound. It also builds games from RGBDS assembly and teaches an agent the Game Boy basics through its own help. It aims to match the original Game Boy as faithfully as possible, and it supports the Game Boy Color too.

**Status: early, usable from source.** The emulator session, debugger, checkpoints, screenshots, audio views, cycle profiler, A/B equivalence check, the RGBDS build loop and hUGETracker songs as editable text (`gbcli song`, `gbcli help music`) work today; onboarding content and the agent eval come next. The plan is [project/2026-11-06-plan.md](project/2026-11-06-plan.md), the design is [project/2026-11-06-design.md](project/2026-11-06-design.md), and the research behind them is [project/2026-11-06-backend-evaluation.md](project/2026-11-06-backend-evaluation.md).

## Install

Build from source with `cargo build --release`; the binary is `target/release/gbcli`. Building gbcli itself needs no RGBDS, SDL or make; SameBoy is vendored and compiled by Cargo. To fetch the test ROMs, which aren't committed, run `python3 -I scripts/corpus/fetch.py` (about 190 MB). gbcli will be a single Rust binary for macOS and Linux. Building games (and running `cargo test`) needs [RGBDS](https://rgbds.gbdev.io/) 1.0.4 on `PATH`: the [1.0.4 release](https://github.com/gbdev/rgbds/releases/tag/v1.0.4), or `brew install rgbds` while Homebrew's current release is 1.0.4; `gbcli doctor` checks it.

## Quick start

```sh
gbcli doctor                    # checks RGBDS 1.0.4 and the project directory
gbcli new game                  # a starter project that builds and runs
cd game                         # its own .gbcli/ keeps the sessions you run here
gbcli build                     # assemble, link and fix the header; errors come back with file and line
gbcli run build/game.gb --session game --frames 120 --shot first.png
gbcli                           # the primer: what the tool does and where to go next
```

## Learn more

- [examples/](examples/README.md) — games built entirely by agents with gbcli
- [docs/architecture.md](docs/architecture.md) — which crate owns what
- [docs/development.md](docs/development.md) — setting up a checkout and running the tests
- [project/](project/) — plans, findings and decisions
- [Pan Docs](https://gbdev.io/pandocs/) — the Game Boy hardware reference

## Credits and license

By Ben Syverson. MIT license; see [LICENSE](LICENSE). gbcli builds on [SameBoy](https://github.com/LIJI32/SameBoy) by Lior Halphon (Expat license).
