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

**Stumbles and fixes** (all landed in `847d250`):

| # | Stumble | Tag | Fix |
|---|---|---|---|
| 1 | Needed a named routine with a `.done` label to `measure`; guessed it from `help optimize`'s `Work..Work.done`. | docs (minor) | none needed; the recipe hinted it |
| 2 | `run --at start` without `--session`: `no_session`. The remedy was clear. | error (minor) | remedy names `GBCLI_SESSION` |
| 3 | `measure` takes no `--keys`, so it can't measure the held-input path; worked around with `run --keys … --measure`. | verb | `measure --keys`/`--keys-file`, same grammar as `run` |
| 4 | Rebuilt over `build/game.gb`; the baseline session then measured the optimized ROM. The agent reported "no warning", but the main thread reproduced the warning: `warning: the ROM changed but no live symbol moved (73 checked)` prints on every op. The real flaw is `help optimize` itself: step 4 rebuilds over the session's ROM, and step 5's `equiv fast.gb` names a ROM nobody built. | docs | the recipe freezes the baseline (`build --out build/fast.gb`); the warning says the session now runs the rebuilt file |
| 5 | `unknown_symbol "Update"` for a ROM copied without its .sym; the remedy didn't say the session had no symbols. | error | the error says when no .sym was loaded, and how to pass one |
| 6 | Lost the baseline .sym (a result of 4), hand-wrote one, and guessed an address off by one. | my-mistake | covered by 4 |
| 7 | `equiv --ram ADDR[,LEN]`: guessed the hex form. | docs | examples with hex and a symbol |
| 8 | `--audio-log` shows only this run's writes, not the engine's earlier setup. | docs | one line in the help |

Found by the main thread while checking: `gbcli gaps --help` said "needs a session", but `gaps` takes no `--session` and reads the whole project. `gaps`, `sessions`, `sessions prune` and `sessions rm` now sit in a new `project` class, and a test refuses any session-class verb that has no `--session`.

**Audio views:** mostly no help. For a 20-frame blip the piano roll came out 120×147 with a tiny mark, and the spectrogram 33×256 and nearly black. The decoded `--audio-log` lines (note, Hz, cents) and the wav's peak were what proved the sound. `--scale` never reached the views: the CLI sent no scale, and `--scale` without `--shot` was refused. Now `run --scale` scales every image the run writes (shot, filmstrip, piano roll, spectrogram). The spectrogram's darkness is about loudness, not size: its floor is −90 dBFS. Normalizing a short clip to its own peak would be a view redesign and is left open.

**Gap ledger:** `gbcli gaps` in the run's project printed `none`. The agent had already removed its first two sessions, so their counts are lost. Later runs keep every session.

## Run 2 (binary at 847d250)

**Result:** all three parts done in about 20 gbcli calls (16 tool calls, 1.5 min), half of run 1's. The main thread re-ran the key numbers in the agent's project and got the same results.
1. **Sprite:** with RIGHT and DOWN held for 30 frames, shadow OAM went from (48, 84) to (78, 114). `view oam` agreed, one frame behind, which is the DMA.
2. **Sound:** `--audio-log` decoded NR10-NR14 (`period 1792 = C5 (512.00 Hz)`). With `--scale 2`, the piano roll "clearly showed the upward pitch sweep". The spectrogram was still narrow and noisy.
3. **Cycles:** `measure VBlank..MainLoop.wait --session base --at start --iterations 60 --keys 'RIGHT@0'` fell from 3,920 T to 1,292 T on `build/fast.gb`'s own session, a cut of 67%. `equiv build/fast.gb --session base --frames 300 --keys '…' --ram wShadowOAM` exited 0. The agent followed `help optimize` "as an exact script", and the baseline stayed put.

**Stumbles:**

| # | Stumble | Tag | Fix |
|---|---|---|---|
| 1 | Code appended after a WRAM `SECTION`: rgbasm's "cannot contain code or data" came back as 126 diagnostic rows, the same message repeated per byte. The main thread reproduced it: 5 rows for 3 lines of code. The remedy was the generic "fix the first error". | error (filed as my-mistake) | identical diagnostics collapse; this message gets a remedy naming `SECTION "Code", ROM0` (agent eval-fix2) |
| 2 | macOS `sed` refused a multi-line edit. | environment | none |
| 3 | An unquoted `u16[wFrame]` was globbed by zsh; the recipes show it quoted. | my-mistake | none |
| 4 | `wFrame` read 88 when the stop said `frame=90`: the stop counts from boot, the program from LCD-on. The help says so. | docs (minor) | none; it's documented |

**Gap ledger:** `gbcli gaps` printed `none` over both sessions, `base` and `fast`.

## Run 3 (binary at 5701328), with a fourth part: a background from a PNG

**Result:** all four parts done in about 30 gbcli calls (23 tool calls, 2.5 min).
1. **Sprite:** RIGHT held 40 frames and DOWN 20 moved it +40/+20 (`view oam`).
2. **Sound:** `--audio-log` decoded NR10-NR14 at the A press.
3. **Cycles:** `measure VBlank..MainLoop.wait --at start --iterations 60 --keys 'RIGHT@0 DOWN@0'` fell from 1,884 T to 1,264 T, a cut of 33%. `--max 1318` held. `equiv build/fast.gb --session base --frames 300 --keys '…' --ram wX,2` exited 0.
4. **Background:** a generated 160×144 three-gray PNG in `assets/bg/` built to 20×18 tiles, 4 unique, through `images.inc`. Its "`help images` and the build-time image conversion worked with no problems".

**Stumbles** (fixes by agent eval-fix3):

| # | Stumble | Tag | Fix |
|---|---|---|---|
| 1 | `read wX` printed the symbol's address (`value=49416 hex=$C108`). The agent expected the byte; `[wX]` reads it. | docs/verb | `read --help` says so, and a bare RAM symbol gets a hint |
| 2 | The piano roll at the default scale was a few dots, and the spectrogram a sliver until `--scale 4`. | docs | a larger default, or the recipe shows `--scale` |
| 3 | `run --frame-budget` prints 60 rows for 60 frames, with no summary. | verb | a summary row |
| 4 | The profiler names the halt loop `MainLoop.wait+$1`. | cosmetic | none |
| 5 | `equiv --ram wX` diverged at frame 1, before init. The optimized build moved `wX`, and power-on WRAM is random, so each side read a different unwritten byte. The remedy talked about checkpoints. The agent worked around it by padding its RAM layout to match. | error | a difference in a never-written byte says so, and how to start after init |
| 6 | A zsh variable holding a command didn't split; macOS `sed -i`. | my-mistake, environment | none |

**Gap ledger:** `gbcli gaps` printed `none` over five sessions.

## Run 4 (binary at b486fc9)

**Result:** all four parts done in about 25 gbcli calls (16 tool calls, 1.6 min).
1. **Sprite:** RIGHT and DOWN held for 30 frames moved it +30/+30.
2. **Sound:** `--audio-log` showed NR10-NR14 at the A press.
3. **Cycles:** `measure VBlank..MainLoop.wait --at start --iterations 60 --keys 'RIGHT@0'` fell from 5,476 T to 1,376 T, a cut of 75%. `equiv build/fast.gb --session base --frames 600 --keys '…' --ram wShadowOAM` exited 0.
4. **Background:** a checkerboard PNG built to 4 tiles, drawn behind the sprite.

`help optimize` and `recipes` were "accurate and runnable as written", and `--out` kept the baseline intact. The agent tagged no stumble gbcli-error or gbcli-bug.

**Stumbles** (all minor; fixes by agents eval-fix4 and the YI5gQ6 help leaf):

| # | Stumble | Tag | Fix |
|---|---|---|---|
| 1 | BSD `sed` and `\n`. | my-mistake | none |
| 2 | Not stated that an image's tile indices count from where its tiles are loaded, so hand-loaded tiles go after `<NAME>_TILES`. | docs | a line in `help images` (YI5gQ6) |
| 3 | The starter's music driver plays on the channels a sound effect uses; the agent removed it by guess. | docs | a note in the starter and the music help |
| 4 | `--frame-budget` printed 60 near-identical rows by default; the summary was already on its first row. Run 3 said the same. | verb | per-frame rows opt-in |
| 5 | `equiv` success prints only `equiv frames=600`, not what was compared. | docs | the row names what it compared |
| 6 | What `VBlank..MainLoop.wait` spans wasn't obvious. | docs | `help optimize` says it |
| 7 | The spectrogram's axes aren't labeled. | docs | the help names the axes and the scale |

**Gap ledger:** `none` (sessions `base` and `fast`).

## Run 5 (binary at 866ad97): the final run

**Result:** all four parts done in about 30 gbcli calls (17 tool calls, 1.6 min).
1. **Sprite:** +30/+30 in 30 frames.
2. **Sound:** `--audio-log` showed the five NR1x writes. The same run without A gave `writes=0`, "the clearest proof".
3. **Cycles:** `measure … --keys 'RIGHT@0'` fell from 5,344 T to 1,484 T, a cut of 72%. `--max 3700` held. `equiv build/fast.gb --session base --frames 300 --keys-file walk.txt --ram wX` printed `equiv frames=300 screen=1 ram=wX audio=writes`.
4. **Background:** `help images` "told me how to wire the tile and map offsets" (run 4's fix).

**Stumbles:** no gbcli error, verb or bug stumble.

| # | Stumble | Tag | Fix |
|---|---|---|---|
| 1 | An unquoted `u8[wX]` was globbed by zsh; the help quotes it. | my-mistake | none |
| 2 | Didn't know `hardware.inc` defines `PAD_*`, so the agent's own `PAD_RIGHT` collided. The error named both files and lines. | docs (minor) | the starter's include line says so (thirteenth session's last commit) |
| 3 | Found the sound-effect advice in `help music` only by grepping for "effect"; the starter's comment says the same. | docs (minor) | none: both places carry it |
| 4, 5 | The spectrogram is a narrow strip with no axis labels or legend on the image. | docs/verb (mild) | backlog: "A spectrogram scaled to its own peak", extended with labels |
| 6 | A few `measure`/`equiv` flags were found only in `help optimize`, which "covered everything". | docs (trivial) | none |

**Criterion check:** run 5 completed the task with no stumble that gbcli's errors or verbs caused. The remaining docs items are minor: one was fixed in the same commit, one is already documented in two places, and one is a view feature parked in the backlog with its un-park condition. I judge the criterion met with that caveat, and record it so the bespoke decision (qfXJij) reads it plainly.

## Summary

**Five runs, about 145 gbcli calls.** Every run completed. Calls per run fell from about 40 to about 20-30, and the time to finish from 2.3 to 1.6 minutes. The stumbles that cost real time came in four kinds, each fixed at the source:
- **The optimize loop overwrote its own baseline** (run 1). `help optimize` now runs as written in a `gbcli new` project, under a test.
- **Errors without the cause:** a missing .sym (run 1), 126 repeated rgbasm rows (run 2), an equiv divergence on never-written RAM (run 3).
- **Verbs that couldn't do the job:** `measure` under held keys (run 1), and frame-budget output with no summary and then 60 rows (runs 3 and 4).
- **Facts that weren't written down:** a bare symbol is its address (run 3); image tile indices count from the image's own first tile (run 4); `--scale` on the audio views (runs 1 and 3).

The independent reviews of those fixes found and fixed four more bugs:
- equiv side B's warning;
- a frame-budget summary skewed by a mid-frame start;
- a bare serve dropping the no-bundle cause;
- the macro hint's place.

**Gap ledger, for the bespoke-core decision (qfXJij).** `gbcli gaps` printed `none` in every run's project, over 13 sessions. Run 1's first two sessions were deleted before the ledger was read. In about 145 calls of ordinary build, run, measure, profile, equiv, audio and image work, the SameBoy backend answered `unsupported` zero times. No stumble in any run came from a core limit: every one was in gbcli's docs, errors or verbs. **For the decision:** this eval gives no evidence that the SameBoy core limits an agent doing DMG game work. The case for a bespoke core must rest on the other inputs the decision lists: the shootout failures, fidelity issues, and the dot-level timing the plan wanted.

**Did the spectrogram and piano roll help?** Mostly not.
- In all five runs the decoded `--audio-log` (register writes named as notes, Hz and cents) proved the sound, and a control run without the button proved its absence.
- The piano roll helped a little once it was readable (scale 2 by default since b486fc9): runs 2 and 5 saw the sweep's direction.
- The spectrogram never helped: a short clip is a narrow strip, unlabeled and dark at the −90 dBFS floor.
- For a 20-frame sound effect, text beats pictures. The views may earn their place for music and noise, which this task didn't exercise. The backlog holds the spectrogram's improvements with an un-park condition.
