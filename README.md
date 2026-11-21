# gbcli

A tool that helps AI assistants build and test Game Boy games.

gbcli runs a Game Boy emulator in the background with no window, so an AI agent can load a game, save and restore moments in it, pause on any instruction, read every byte of memory, measure exactly how long code takes, and look at the screen or listen to the sound. It also builds games from RGBDS assembly and teaches an agent the Game Boy basics through its own help. It aims to match the original Game Boy as faithfully as possible, and it supports the Game Boy Color too.

**Status: early (0.1).** The emulator session, debugger, checkpoints, screenshots, audio views, cycle profiler, A/B equivalence check, the RGBDS build loop and hUGETracker songs as editable text (`gbcli song`, `gbcli help music`) work today; onboarding content and the agent eval come next. The plan is [project/2026-11-06-plan.md](project/2026-11-06-plan.md), the design is [project/2026-11-06-design.md](project/2026-11-06-design.md), and the research behind them is [project/2026-11-06-backend-evaluation.md](project/2026-11-06-backend-evaluation.md).

## Install

```sh
brew install bensyverson/tap/gbcli
```

This installs gbcli and [RGBDS](https://rgbds.gbdev.io/), the assembler it builds games with, on macOS and Linux. Apple silicon Macs on macOS 15 or later get a prebuilt binary; other machines build it from source, which Homebrew does for you. gbcli needs RGBDS 1.0.4, and Homebrew installs its current release, so run `gbcli doctor` to check.

To build from source yourself: `cargo build --release`, and the binary is `target/release/gbcli`. SameBoy is included and compiled by Cargo; building games needs [RGBDS 1.0.4](https://github.com/gbdev/rgbds/releases/tag/v1.0.4) on `PATH`. Setting up a checkout for development: [docs/development.md](docs/development.md).

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
