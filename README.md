# gbcli

A tool that helps AI assistants build and test Game Boy games.

gbcli runs a Game Boy emulator in the background with no window, so an AI agent can load a game, save and restore moments in it, pause on any instruction, read every byte of memory, measure exactly how long code takes, and look at the screen or listen to the sound. It also builds games from RGBDS assembly and teaches an agent the Game Boy basics through its own help. It aims to match the original Game Boy as faithfully as possible, and it supports the Game Boy Color too.

**Status: early.** The design is settled and the Rust workspace builds, but there are no commands yet. The plan is [project/2026-11-06-plan.md](project/2026-11-06-plan.md), the design is [project/2026-11-06-design.md](project/2026-11-06-design.md), and the research behind them is [project/2026-11-06-backend-evaluation.md](project/2026-11-06-backend-evaluation.md).

## Install

Not yet available. To build from source, run `cargo build`. You don't need RGBDS, SDL or make; SameBoy is vendored and compiled by Cargo. To fetch the test ROMs, which aren't committed, run `python3 -I scripts/corpus/fetch.py` (about 190 MB). gbcli will be a single Rust binary for macOS and Linux, and it will need [RGBDS](https://rgbds.gbdev.io/) at a pinned version for building games.

## Quick start

Planned, per the design doc:

```sh
gbcli new mygame && cd mygame   # a starter project that builds and runs
gbcli build                     # assemble, link and fix the header
gbcli                           # the primer: what the tool does and where to start
```

## Learn more

- [project/](project/) — plans, findings and decisions
- [Pan Docs](https://gbdev.io/pandocs/) — the Game Boy hardware reference

## Credits and license

By Ben Syverson. MIT license; see [LICENSE](LICENSE). gbcli builds on [SameBoy](https://github.com/LIJI32/SameBoy) by Lior Halphon (Expat license).
