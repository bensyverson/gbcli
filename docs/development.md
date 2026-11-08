# Development

How to set up a checkout, run the tests, and keep the suite fast.

## Set up a checkout

1. **Rust 1.99.1** (`rustup`), and **RGBDS 1.0.4** on `PATH` (`brew install rgbds` while Homebrew's release is 1.0.4). The build-loop tests and the opcode lookup's rgbasm cross-checks need RGBDS; without it they fail and name the install command.
2. **FPC 3.2.2** (`brew install fpc`), for hUGETracker's own song export.
3. **Enable the pre-commit hook:** `git config core.hooksPath scripts/hooks`. It runs `cargo fmt --check`, clippy with `-D warnings` and `cargo test`, and needs the Bash sandbox off, because cargo's registry cache is outside it. Without this setting, commits run no checks at all.
4. **Fetch the test-ROM corpus** (gitignored; about 190 MB): `python3 -I scripts/corpus/fetch.py`, or at least `--only blargg --only mooneye --only gbmicrotest --only dmg-acid2 --only cgb-acid-hell --only hugetracker-samples`. Then `scripts/music/export-reference.sh`, which builds hUGETracker 1.0.11's `uge2source` and exports the fetched songs as the reference the emitter is compared against. Without them, `cargo test` fails and names the command. Licenses and the manifest: [project/2026-11-06-test-rom-corpus.md](../project/2026-11-06-test-rom-corpus.md).
5. **Optional, for the pre-release flash tests:** a batteryless export of GB Studio's default project (MBC5, "Save game data to ROM") at `local/demo/build/rom/demo.gb` with its `.sym` (gitignored), or named with `GBCLI_GBSTUDIO_BATTERYLESS_ROM`. Without it, `crates/engine/tests/flash_save.rs` fails under `--ignored` and gives the build steps.

`gbcli doctor` checks RGBDS and a project directory.

## The test suites

- **Warm suite:** `cargo test --workspace`. **It must stay under 30 s.** The hook times it and warns past that. `python3 -I scripts/bench/test-times.py` finds the slow tests; wall time under load (parallel agents, low-power mode) is noise, so compare CPU seconds and check the load average it prints.
- **Pre-release suite:** a test too slow to keep warm gets `#[ignore = "pre-release: <why>"]`; run them all with `cargo test --workspace -- --ignored`. Policy and figures: [project/2026-11-06-test-suite-speed.md](../project/2026-11-06-test-suite-speed.md).
- The dev profile builds at `opt-level = 1` (`Cargo.toml`), because the tests drive the emulator for thousands of frames.

## Costs

- Build cost (`scripts/bench/workspace-build/run.sh`) and CLI latency (`scripts/bench/cli-latency/run.py`, about 12 ms a request against a live session): figures in [scripts/bench/README.md](../scripts/bench/README.md).
