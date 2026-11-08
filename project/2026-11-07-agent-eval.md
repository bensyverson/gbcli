# Agent eval: a fresh Sonnet agent builds and optimizes with gbcli

Leaf k22J86 (thirteenth session). The definition of done for the CLI and onboarding, and evidence for the bespoke-core decision (qfXJij).

## Method

Each run gives a fresh Sonnet agent (the Agent tool, `model: sonnet`, no worktree) the release binary `gbcli 0.1.0 (SameBoy ea667ad+patches.b0f50798)`, built with `cargo build --release` at the run's commit and copied to a scratch directory, plus RGBDS 1.0.4 on `PATH`. The agent may learn gbcli only from the binary: no args, `help`, `recipes`, `VERB --help` and its errors. It may not read gbcli's source, anything under `/Users/ben` or the web. It works in an empty directory.

The task, DMG only:
1. A sprite moves with the D-pad at 1 px per frame, proven with gbcli.
2. A sound effect plays on A, proven with gbcli's audio views.
3. The main loop's cycles per frame fall by at least 30%, measured with `measure` or `--profile` and proven unchanged with `equiv`.

The agent reports every stumble with the command, the output, and what would have prevented it, tagged gbcli-docs, gbcli-error, gbcli-verb, gbcli-bug, my-mistake or environment. The main thread checks load-bearing claims by reproducing them before it files a fix.

## Run 1 (binary at 6ba54c4)

**Result:** all three parts done in about 40 gbcli calls (26 tool calls, 2.3 min).
1. **Sprite:** with RIGHT and DOWN held for 30 frames, shadow OAM went from (Y 48, X 84) to (78, 114).
2. **Sound:** `--audio-log` decoded the five NR1x writes.
3. **Cycles:** `run --keys … --measure Update..Update.done` fell from 1,988 T to 408 T, a cut of 79%. `equiv v2.gb --frames 200 --keys-file walk.txt --ram 0xC000,4 --ram 0xC106,2` exited 0. A patched negative control diverged at frame 11 and exited 1.

**Stumbles and fixes** (the fixes are agent eval-fix1's, on the leaf):

| # | Stumble | Tag | Fix |
|---|---|---|---|
| 1 | Needed a named routine with a `.done` label to `measure`; guessed it from `help optimize`'s `Work..Work.done`. | docs (minor) | none needed; the recipe hinted it |
| 2 | `run --at start` without `--session`: `no_session`. The remedy was clear. | error (minor) | remedy names `GBCLI_SESSION` |
| 3 | `measure` takes no `--keys`, so it can't measure the held-input path; worked around with `run --keys … --measure`. | verb | `measure --keys`/`--keys-file`, or point at the workaround |
| 4 | Rebuilt over `build/game.gb`; the baseline session then measured the optimized ROM. The agent reported "no warning", but the main thread reproduced the warning: `warning: the ROM changed but no live symbol moved (73 checked)` prints on every op. The real flaw is `help optimize` itself: step 4 rebuilds over the session's ROM, and step 5's `equiv fast.gb` names a ROM nobody built. | docs | the recipe freezes the baseline (`build --out build/fast.gb`); the warning says the session now runs the rebuilt file |
| 5 | `unknown_symbol "Update"` for a ROM copied without its .sym; the remedy didn't say the session had no symbols. | error | the error says when no .sym was loaded, and how to pass one |
| 6 | Lost the baseline .sym (a result of 4), hand-wrote one, and guessed an address off by one. | my-mistake | covered by 4 |
| 7 | `equiv --ram ADDR[,LEN]`: guessed the hex form. | docs | examples with hex and a symbol |
| 8 | `--audio-log` shows only this run's writes, not the engine's earlier setup. | docs | one line in the help |

Found by the main thread while checking: `gbcli gaps --help` says "needs a session", but `gaps` takes no `--session` and reads the whole project. The fix is to check every verb's CLASS line.

**Audio views:** mostly no help. For a 20-frame blip the piano roll came out 120×147 with a tiny mark, and the spectrogram 33×256 and nearly black. The decoded `--audio-log` lines (note, Hz, cents) and the wav's peak were what proved the sound. Action: eval-fix1 checks whether `--scale` reaches the views and makes short clips readable if a small change does it.

**Gap ledger:** `gbcli gaps` in the run's project printed `none`. The agent had already removed its first two sessions, so their counts are lost. Later runs keep every session.
