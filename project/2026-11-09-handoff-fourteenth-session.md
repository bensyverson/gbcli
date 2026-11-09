# Handoff: 2026-11-08/09 (fourteenth session)

This handoff is current until the next commit after the one that adds it. Start with `job orient`, then read this. The previous handoff is [2026-11-07-handoff-thirteenth-session.md](2026-11-07-handoff-thirteenth-session.md).

## State of play

Ben was around all session and ruled as things came up. The original plan (root `9b5RBA`, Stages 0-10) closed. Stage 11 (`7Dap6M`), example games built by agents, is under way: two games done, their feedback reviewed, and most of the resulting gbcli changes landed. **Origin is at `1f1588f`** (the playable ROMs), pushed outside this session; the seven commits after it, from `a95722e` on, are not pushed. Ben said to push after review (see Next).

**Ben's rulings this session**, each recorded where it applies:
- **The bespoke core is parked,** with an un-park condition ([backlog.md](backlog.md) ▸ A bespoke Rust emulator core). The PPU-mode spike re-checked that condition, and it still doesn't hold.
- **d8FHRl: warn** (`shades_unpinned`, in the GBC images plan under question 3).
- **The example games:** five games in three waves. A and B first; then D and E; C last. Game agents are users only, and agents make all assets. Each game is a vertical slice, and the DX docs are written blind. Recorded in [2026-11-08-example-games-plan.md](2026-11-08-example-games-plan.md). Game agents may also run past 40% context once half their leaves are done.
- **The DX review** ([2026-11-08-examples-dx-review.md](2026-11-08-examples-dx-review.md), Decisions and Later rulings):
  - a plain build derives color art's DMG version, superseding GBC plan question 9;
  - every other recommendation accepted;
  - a pixel-art text format built now, with woodcase suggested in game briefs only, never in gbcli's help;
  - 8x8 objects become a project setting;
  - label breakpoints re-resolve whenever the ROM changes;
  - plain and `--gbc` tile data may differ (kept);
  - a plain build warns image-wide on shared shades.
- **The pixel text design** was approved with all eight recommendations ([2026-11-08-pixel-text-design.md](2026-11-08-pixel-text-design.md)).

**Landed** (each through the pre-commit hook; agent work squash-merged after reading the staged diff):

| Commit | What |
|---|---|
| `17f87bd`, `4381e6a` | Park ruling; suite re-timed (23.9 s warm); `shades_unpinned` |
| `49620e8`, `378d5dd`, `95fa88e` | Stage 11 plan and `examples/`; Vesper Swarm (shooter) and Switchback (racer) with DX docs |
| `cb8addd` | The DX review, rulings, and the fix leaves |
| `7595c6a` | PPU mode on raster writes; STAT conditions read while SameBoy batches; `condition_error` stops |
| `7765b89` | `run --oam-log`, `--audio-channel`/`--audio-summary`, `view palettes`, better profiler names |
| `105731f` | `lcd_off` frames apart from lag; one `keys_unapplied` row; a run keeps its stop when a read fails; `--dir` anchors every relative path |
| `4766e1c` | `run --from boot` restarts a session on a rebuilt ROM; `gbcli new` makes `.gbcli/` |
| `372d772`, `0602eb6` | Plain builds derive color art; 8x8 sprite sheets; quieter `shared_shade`; manifest remedies; then the 8x8 frame-list palette fix found by review |
| `1f1588f`, `a95722e` | Playable ROMs, `examples/racer/switchback.gb` and `examples/shooter/vesper-swarm.gb`, built by `scripts/examples/build-roms.sh` |
| `42c571f` | Label breakpoints re-resolve on every load; `new`'s next lines `cd` into the project |
| `f6b8a97` | Pixel text: `crates/pixels`, `gbcli pixels`, and `build` reading `.pixels` |

**Reviews.** Each reviewer was given the code, not the authors' reports.
- `4381e6a`/`77113de`: nothing serious.
- `7595c6a` (review-ppu14): the core rule is sound. Three medium and two low findings are filed as one leaf (open).
- `105731f`/`4766e1c`/`7765b89` (review-run14): one medium-high and three low findings, all fixed in `42c571f`.
- `372d772` (review-img14): one high, fixed in `0602eb6`. One low is added to `mjBkTn`. Two behavior questions went to Ben, who ruled.
- **Unreviewed:** `42c571f` (session load path) and `f6b8a97` (pixel text, about 4,700 lines, new wire types).

## Needs Ben

- **Push.** Ben asked to push after review. This batch's reviews are done, and their fixes have landed, except the PPU follow-up leaf (mediums: help oversells what a STAT breakpoint proves; `show`'s mode can be wrong in one window). `42c571f` and `f6b8a97` are not yet reviewed. The next session should review those two, then push, unless Ben says push now.
- **The protocol version:** `PROTOCOL_VERSION` is still 1, though the wire has gained `condition_error`, `mode` on raster writes, `lcd_off`, `keys_unapplied`, `ReadValue.error`, `ImageAsset.object_size`, `ImageProblem.column`, `pixels_text`, the `pixels` op, `oam_log` and the `palettes` view. version.rs's rule says bump. This is in the PPU follow-up leaf; decide it before the push.
- **`gbcli help WORD` now prefers a topic to a verb** (`help pixels` is the topic). This is a two-way door taken by pixels14.

## Next, in order

1. **Review `42c571f` and `f6b8a97`** (two reviewers, briefed as `review.md` says; this session's brief is reusable as described below). Then push, if Ben agrees.
2. **The open leaves under `SQhTqx`:**
   - "PPU mode follow-ups". Its probes and probe ROMs are in the kept worktree `.claude/worktrees/agent-a1fc06f75f25581eb`; remove it when the leaf lands.
   - `mjBkTn` (8x8 project setting, plus the manifest derived-art remedy in its note) and "plain build warns image-wide". These touch the same files: give them to one agent.
3. **Then D and E:** detail the roguelike (`4Nn3us`) and rhythm (`0nfXkK`) leaves in the shape of A and B. They're blocked on `SQhTqx`, so they run on the improved gbcli. Briefs should say:
   - build both targets from the start;
   - write edit scripts to files;
   - woodcase is available for larger art;
   - `.pixels` now exists.

   Rebuild the pinned binary first (`cargo build --release -p gbcli --target-dir /Users/ben/.cache/gbcli-examples/target`), and never touch `target/release` (LoadHigh runs it).
4. **For LoadHigh** (unpublished until the push): the wire changes listed above. `run --from boot` now skips the head. `--dir` anchors relative paths. Label breakpoints follow their labels. A run whose read fails now succeeds, with the error in that read's row.

## Reusable briefs

This session's briefs were in the scratchpad, which doesn't survive. Their substance:
- **Game agents:** the user contract (read only gbcli's help, the READMEs and the plan; never `crates/`, `docs/`, `scripts/`, other `project/` docs, or the other game), the pinned binary, `mkdir DIR/.gbcli` (fixed since by `new`), Python stdlib for tools, a DX doc per leaf with stumbles ranked by cost, and every README claim backed by a command.
- **Fix agents:** the delegation template, plus "read the DX docs' stumbles for your leaf", OTHERS listing each agent's area, and "re-bless shared goldens; the integrator re-blesses at merge".
- **Reviewers:** SURFACE / COMMITS / NEVER, plus review.md's bar. The NEVER list per surface was what found the high bug.

## Mistakes, so the next session doesn't repeat them

- **A test fixture hid a real bug.** To merge `4766e1c`, the integrator changed three CLI tests to delete the starter's new bundle (`game_in_outer_bundle`) instead of asking why they broke. The reviewer then found that following `new`'s printed lines really did lose the session. When a merge breaks a test, check the product before changing the test's setup.
- **Tests passing for the wrong reason, twice:** the 8x8 GBC test checked only that slots were set (slot 0 hid the bug), and the HBlank recipe's breakpoint was called a proof. Both were found only by reviewers given NEVER lists.
