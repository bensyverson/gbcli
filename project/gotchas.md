# Gotchas

Project-specific traps that cost real time and that no general rule predicts. Read at session start; append when you hit one.

- **Fix at the source when you can.** A gotcha is a bug report on our tooling, not a permanent fact — if it can be fixed in code, file it in `job` and fix it instead of recording it here.
- **Delete anything that becomes obvious, gets fixed, or stops recurring.** Keep this list short; a long list is one nobody reads.
- **Feedback about `AGENTS.md` itself** — a rule that was wrong, misread, or cost time — goes here too, prefixed `rule:`. It is harvested when the shared rules are reviewed.

Format: one dated H2 headline, then one paragraph. If an entry needs more than that, it's a finding — write it as a dated doc under `project/` and link it from the paragraph.

---

## 2026-11-06 — git in the Bash sandbox needs `GIT_CONFIG_GLOBAL=/dev/null`

The sandbox can't read `~/.gitconfig`, so `git clone` and friends fail with "unable to access '/Users/ben/.gitconfig': Operation not permitted". Prefix git commands with `GIT_CONFIG_GLOBAL=/dev/null`; the warning about `~/.config/git/ignore` is harmless. That also drops `user.name`/`user.email`, so `git commit-tree` and `git merge --squash` fail with "Author identity unknown": set `GIT_AUTHOR_NAME`, `GIT_AUTHOR_EMAIL`, `GIT_COMMITTER_NAME` and `GIT_COMMITTER_EMAIL` for those calls (the real `git commit` runs with the sandbox off and reads `~/.gitconfig`). In an isolated agent worktree the harness refuses any git command carrying that variable ("injecting git configuration whose effect on where git writes can't be verified"); there, run the plain git call with the sandbox off for that one call (`project/agents/harness.md`; observed 2026-11-06, leaf eeTyfo).

## 2026-11-06 — subagents can't write report files

The harness refuses a subagent's Write to a findings file ("Subagents should return findings as text"). Brief research agents to return findings in their final message, and have the main thread save them.

## 2026-11-06 — Python downloads through the sandbox proxy end early, silently

`urllib` through the Bash sandbox's filtering proxy stopped short of `Content-Length` on most archives (e.g. 409,600 of 423,779 bytes, every attempt) with no exception, or raised `IncompleteRead`; `curl` through the same proxy got whole files. `scripts/corpus/fetch.py` checks the length and resumes with HTTP `Range`; anything else that downloads in Python needs the same.

## 2026-11-06 — Claude Code kills a background command at about 10 minutes

A `run_in_background` Bash command that ran past ~595 s died with `time: command terminated abnormally`, and a `cargo run` child it spawned kept running orphaned, writing the same output file as the next attempt. Start anything longer (the full corpus run) detached — `nohup … > log 2>&1 & disown` — and wait on its pid; check for orphans with `pgrep -fl` before re-running.

## 2026-11-06 — SameBoy publishes a blank frame after the LCD is turned on, after rendering it

On the first vblank after LCDC's enable bit is written, `GB_display_vblank` (display.c:235) fills `gb->screen` with the LCD-off colour *before* calling the vblank callback, while `frame_skip_state` is still `GB_FRAMESKIP_LCD_TURNED_ON` (it is cleared right after the callback returns, display.c:2213–2225). The frame's pixels are therefore the blank screen even though the PPU just rendered a full frame; anything captured during that render and published at the same vblank (as the layer tags are) describes pixels the caller will never see. Detect it from the callback with `gbcli_frame_scrubbed` and publish no side data for that frame. It shows up as an apparently one-frame lag between pixels and tags (observed 2026-11-06, leaf n2WX5r).

## 2026-11-06 — WRAM is random at power-on, so a test ROM's unset flag picks a branch by seed

SameBoy randomizes RAM at power-on (seeded, so it is the same every run), and `quick_dmg` does not clear it. `layers.gb` chose its quiet or VRAM-reading loop from `$C000`, and the test only wrote it for the busy variant, so the "quiet" variant silently ran the busy loop too and the batched renderer (`render_line`) was never tested. Write every flag a test ROM reads (`select_loop` in `crates/sameboy/tests/it/layers.rs`), or have the ROM clear it, and check a claimed code path with an observation (leaf XKaz0o).

## 2026-11-06 — in an isolated worktree, a path containing `git` reads as a git command

The worktree guard refused plain commands that only *mentioned* a path under `/Users/ben/git/...` inside a pipe, redirect or heredoc ("names git in a form too complex to verify"), including the session scratchpad (`/private/tmp/claude-501/-Users-ben-git-gbcli/...`), and `/usr/bin/time -p cargo test` ("cannot be shown not to be git"). Keep scratch files under the worktree's `target/` (gitignored), run scripts as `python3 -I <file>` rather than heredocs, and keep each call to one plain command (leaf 5ewkYZ).

## 2026-11-06 — a `--frames` run stops at the vblank, before the ROM has seen it

`run --frames N` ends on the core's vblank stop, which comes before the game's own vblank code runs. So anything a ROM does "at vblank" (sampling JOYP, logging a frame counter) for the stop's vblank happens at the start of the *next* run, as that run's frame 0. A test that assumed the next run's first sample was its first full frame was off by one (leaf bHJhLp, `crates/server/src/handle/keys_tests.rs`).

## 2026-11-06 — the CLI goldens mask every ` at=` and `"at":` as wall-clock time

`crates/gbcli/tests/cli/common.rs` (`mask`) replaces the value after ` at=` and `"at":` in every transcript, because event rows carry host time there. A new field named `at` holding emulated time is silently masked, so its golden pins nothing. Name emulated times something else (the audio log's `elapsed`, leaf vd3ulF), or narrow the mask.

## 2026-11-07 — the worktree guard reads a file named `SOURCE` as the shell's `source`

In an isolated worktree, `cat crates/engine/data/gb-opcodes/SOURCE` was refused ("runs a string through SOURCE, which can't be verified"), as was a heredoc piped into a command next to `cargo`. Read such files with the Read tool, and keep each Bash call to one plain command (leaf 7qzbqc).

## 2026-11-07 — the hook's suite time is noise under low-power mode or parallel agents

The pre-commit hook warned 103–142 s against the 30 s budget all session, while the Mac was in low-power mode and several agents (and other sessions' Go builds, load average 48–167) were compiling. No single test was slow (largest about 6 s alone). Don't chase a hook warning taken under load; re-time with `python3 -I scripts/bench/test-times.py` on an idle machine, out of low-power mode (`pmset -g | grep lowpowermode`), before moving tests to the pre-release suite.

## 2026-11-07 — `git add -A` after a squash conflict commits the markers in files you didn't resolve

A `git merge --squash` that conflicted in two files was resolved in one (crates/server/src/handle/mod.rs), then `git add -A` staged the other (project/2026-11-06-design.md) with its `<<<<<<<` markers, and the hook passed because nothing compiles Markdown; commit e3ca2be shipped them. After resolving, run `git diff --name-only --diff-filter=U` and `git grep -n '^<<<<<<< '` before staging.

## 2026-11-07 — a worktree has no corpus, and `GBCLI_TEST_ROMS` doesn't reach every test

`tests/corpus/fetched/` is gitignored, so a fresh worktree's `cargo test --workspace` fails on missing corpus files. `GBCLI_TEST_ROMS=/Users/ben/git/gbcli/tests/corpus/fetched` fixes the CLI's song tests, but `crates/corpus/tests/it/docboy_boot.rs` reads `<repo>/tests/corpus/fetched` directly and ignores the variable. A symlink from the worktree's `tests/corpus/fetched` to the main checkout's (remove it afterwards) makes the whole suite run (leaf 3KQ8fO).

## 2026-11-07 — the permission check refuses to run uge2source until Ben approves it

`scripts/music/export-reference.sh` builds hUGETracker's `uge2source` from its source. Claude Code's auto-mode check refused the first run of that binary as "code from external" until Ben approved it in the session; a later agent ran the script without a prompt. If it is refused, ask Ben, and don't work around it. Two more traps on the same path. The sandbox refuses to extract a tarball's `.gitmodules` ("Operation not permitted"), so the script extracts only `src/`. And parallel agents share `$TMPDIR`: a reviewer's scratch folder replaced the hand-built binary, so build through the script, which keeps its build under `target/uge2source`.

## 2026-11-07 — this checkout has no toolchain and no hook, and commits still pass

In the eleventh session, `git config core.hooksPath` was unset and the machine had no `cargo`, `rustc` or `rgbasm` (no `~/.cargo`, `~/.rustup`, or Homebrew rust), no `tests/corpus/fetched` and no `local/`. So `git commit` ran no hook and succeeded silently (36a03b3 went in untested), and agents briefed "cargo needs the sandbox off" found no cargo at all. Before committing or dispatching, check `command -v cargo rgbasm` and `git config core.hooksPath` (the AGENTS.md head now says so); setup is in docs/development.md.

## 2026-11-07 — in a worktree, run cargo by its full path

Rust was installed with rustup this session, and `~/.cargo/bin` isn't on agents' `PATH`. In a worktree, the guard refused `. ~/.cargo/env; cargo …` and any command that put a `/Users/ben/git/...` path next to `cargo`. Run `/Users/ben/.cargo/bin/cargo` directly from the worktree, one plain command per call, with the sandbox off. The main thread can source `~/.cargo/env` (leaves DkhKgi, a1393ec).

## 2026-11-07 — `.jobs/log/*.jsonl` is tracked, so a `git reset --hard` erases every uncommitted `job` write

The tracker's event log lives in git, and `job` rebuilds its cache from it on open. In the twelfth session, undoing a staged squash with `git reset --hard HEAD` reverted `.jobs/log/Z7rGfP.jsonl` too. Every leaf, note, label and close since the last commit was gone with no error, and the leaves had to be recreated under new ids that running agents had to be told. To undo a squash, unstage and restore only the agent's paths (`git restore --staged --worktree -- <paths>`), and check `git status --short .jobs` before any reset or checkout.
