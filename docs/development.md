# Development

How to set up a checkout, run the tests, keep the suite fast, and release.

## Set up a checkout

1. **Rust 1.99.1** (`rustup`), and **RGBDS 1.0.4** on `PATH` (`brew install rgbds` while Homebrew's release is 1.0.4). The build-loop tests and the opcode lookup's rgbasm cross-checks need RGBDS; without it they fail and name the install command.
2. **FPC 3.2.2** (`brew install fpc`), for hUGETracker's own song export.
3. **Enable the pre-commit hook:** `git config core.hooksPath scripts/hooks`. It runs `cargo fmt --check`, clippy with `-D warnings` and `cargo test`, and needs the Bash sandbox off, because cargo's registry cache is outside it. Without this setting, commits run no checks at all.
4. **Fetch the test-ROM corpus** (gitignored; about 190 MB): `python3 -I scripts/corpus/fetch.py`, or at least `--only blargg --only mooneye --only gbmicrotest --only dmg-acid2 --only cgb-acid-hell --only hugetracker-samples`. Then `scripts/music/export-reference.sh`, which builds hUGETracker 1.0.11's `uge2source` and exports the fetched songs as the reference the emitter is compared against. Without them, `cargo test` fails and names the command. Licenses and the manifest: [project/2026-11-06-test-rom-corpus.md](../project/2026-11-06-test-rom-corpus.md).
5. **Optional, for the pre-release flash tests:** a batteryless export of GB Studio's default project (MBC5, "Save game data to ROM") at `local/demo/build/rom/demo.gb` with its `.sym` (gitignored), or named with `GBCLI_GBSTUDIO_BATTERYLESS_ROM`. Without it, `crates/engine/tests/it/flash_save.rs` fails under `--ignored` and gives the build steps.

6. **For the wasm check** (the web-target ruling asks that library crates keep building for `wasm32`): `rustup target add --toolchain stable wasm32-wasip1`, and wasi-sdk 34 for SameBoy's C, unpacked under `~/.local/wasi` (`curl -sSL https://github.com/WebAssembly/wasi-sdk/releases/download/wasi-sdk-34/wasi-sdk-34.0-arm64-macos.tar.gz | tar -xz -C ~/.local/wasi`). Then `scripts/web/check-wasm.sh` checks the protocol, bundle, server and CLI crates for `wasm32-wasip1`. Homebrew's Rust can't add targets, so the script calls rustup's toolchain directly. It catches only what doesn't compile: wasip1's `std` compiles threads, processes and file locks that then fail at run time ([project/2026-11-09-web-spike-findings.md](../project/2026-11-09-web-spike-findings.md)).

`gbcli doctor` checks RGBDS and a project directory.

## The test suites

- **Warm suite:** `cargo test --workspace`. **It must stay under 30 s.** The hook times it: on a quiet machine (load average at most half the CPUs) a run past 30 s warns; under load it warns only past 60 s. Neither fails the commit: the budget is a target. `python3 -I scripts/bench/test-times.py` finds the slow tests; wall time under load (parallel agents, low-power mode) is noise, so compare CPU seconds and check the load average it prints.
- **Pre-release suite:** a test too slow to keep warm gets `#[ignore = "pre-release: <why>"]`; run them all with `cargo test --workspace -- --ignored`. Policy and figures: [project/2026-11-06-test-suite-speed.md](../project/2026-11-06-test-suite-speed.md).
- **One integration-test binary per crate:** `crates/<crate>/tests/it/main.rs` (the `gbcli` crate's is `tests/cli/`), each test file a module of it and each helper (`common/`, …) a module once, so a crate's tests link once and share one process. Run one crate's with `cargo test -p <crate> --test it`. A test that needs a process of its own (it redirects stdout, counts the process's threads, or holds a wall-clock bound) is a file beside `it/` and says why at its top.
- The dev profile builds at `opt-level = 1` (`Cargo.toml`), because the tests drive the emulator for thousands of frames.

## Costs

- Build cost (`scripts/bench/workspace-build/run.sh`) and CLI latency (`scripts/bench/cli-latency/run.py`, about 12 ms a request against a live session): figures in [scripts/bench/README.md](../scripts/bench/README.md).

## Releasing

gbcli ships through Ben's Homebrew tap (`brew install bensyverson/tap/gbcli`), released the same way as his other tools. With [homebrew-tap](https://github.com/bensyverson/homebrew-tap) checked out beside this repo, `scripts/release` runs the tap's release script: it runs the pre-commit hook as its check, tags `v<version>`, points `Formula/gbcli.rb` at the tag, builds and uploads an Apple-silicon bottle for macOS 15 and later, then installs, tests and audits the formula. `scripts/release.conf` says how; `scripts/release --help` lists the steps, and `--dry-run` prints the plan. It needs the Bash sandbox off.

- **`scripts/release <X.Y.Z>` bumps the version for you.** The version is `version` under `[workspace.package]` in `Cargo.toml`, and every crate's entry in `Cargo.lock` repeats it. The release check's cargo build refreshes the lock, and `version_also=Cargo.lock` commits it with the bump. The formula builds with `cargo install --locked`, so a stale lock would fail every source install; the tap's script refuses to commit a bump the check didn't refresh the lock for.
- **Run the pre-release suite first:** `cargo test --workspace -- --ignored`. The release check runs only the warm suite.
- **The formula's test** runs `gbcli new`, `build` and a 60-frame `run` with a screenshot, so a release that can't build its own starter fails `brew test`.
