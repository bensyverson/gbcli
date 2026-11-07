# Handoff: 2026-11-07 (tenth session)

This handoff is current until the next commit after the one that adds it. Start with `job orient`, then read this. The previous handoff is [2026-11-07-handoff-ninth-session.md](2026-11-07-handoff-ninth-session.md).

## State of play

Ben was present throughout and ruled on DMG shades three times, on the flash test ROM, and on the review flow. Every agent commit below was squash-merged after a read of its staged diff, and each went through the pre-commit hook.

**Landed:**

| Commit | Leaf | What |
|---|---|---|
| `4900064` | XJV3UL | `--rtc-advance` is bounded per cart clock (`gbcli_core::CartClock`: MBC3 1024 d, TPP1 3584 d, HuC3 65536 d). `oam[N]` is documented as reading raw OAM. `ObjectPalette` is `Dmg` or `Cgb`, never both. The commit also carries the GBC research and plan docs. |
| `273f199` | iH2KOO | The opcode evaluator: no panic on `x*0`, a 64-level nesting cap, `wrapping_pow`, evaluation from the operand as written, and the rest of rgbasm's constant syntax. |
| `105b46f` | ygc9bb | DMG shades are chosen once per project by a cascade: a declared `assets/palette.txt`, then a known palette, then lightness stretched over the project, then the nearest shade. Warnings: `shared_shade` and `shade_stretched`. A malformed `palette.txt` is `palette_file`. Also gitignores `local/`. |
| `81767c6` | (review) | Notes that one RTC advance reaches every reading only from in-range clock counters. |
| `e9cba4f` | L00RuF | The flash-chip model: the new `gbcli-flash` crate, SameBoy routing, blob format 4, and the power-cycle carry. Validated on Ben's GB Studio batteryless export. Stage 9 closed. |
| `d0e891a` | GTXXyr | The shade review's fixes: a project-level `shared_shade` across images, and `palette.txt` edge cases (BOM, `#` then a tab, a file without images). A colorless project reports `nearest`. rgbgfx diagnostics name the source PNG. |

**Ben's rulings this session** (recorded in design §12 and on ygc9bb):
- **First ruling:** each color takes the nearest DMG shade even when another color already has it, with a warning.
- **The cascade:** declared palette, then known palette, then a stretch over the whole project. The point is that one color is one shade in every image. A per-image stretch was rejected for that reason.
- **Stretch warnings:** keep the stretch, and warn (`shade_stretched`) when it moves a color two or more shades from its nearest shade.
- **Flash validation:** use GB Studio's default project, built with MBC5 and "Save game data to ROM". Ben's export is at `local/demo/build/rom/demo.gb`, gitignored. Override it with `GBCLI_GBSTUDIO_BATTERYLESS_ROM`.

**Decisions made without Ben (two-way doors):**
- XJV3UL used a per-cart bound rather than weakening the doc.
- The shade fixes added one project-level `shared_shade` per shade for colors that share it across images.
- The out-of-range clock counters got a doc caveat, not a wider bound.

**Reviews this session:**
- `0cc6a6a` (FPQsnp): images sound; the opcode evaluator had a panic and a stack abort. Fixed in iH2KOO.
- `105b46f` (ygc9bb): nothing serious; six low findings. Fixed in GTXXyr.
- `4900064` (XJV3UL): one doc gap, fixed in `81767c6`.
- `273f199` (iH2KOO): wrong answers with `&`/`%` written against a digit, and value/address slots that skip the evaluator. That is leaf radJxi.

Labeled `reviewed`: FPQsnp, XJV3UL, iH2KOO, ygc9bb.

## In flight at handoff

Nothing. radJxi (the opcode lookup's second-review fixes) lands in the same commit as this handoff: glued `&`/`%` refused, numbers wider than 32 bits read as 0, every data operand read by the evaluator (symbols syntax-checked without a value), and one evaluation per operand. lookup.rs is split into `operand.rs` and `expr/literal.rs`. It is unreviewed.

## Needs Ben

1. **The GBC images plan** (revise it first: the LoadHigh review, agreed by Ben and LoadHigh's agent, is a note on k5Afkg — indexed art with named palettes, one named-palette file replacing `assets/palette.txt`, protocol types and an `assets` op as the shared seam; then re-ask questions 4–5): [2026-11-07-gbc-images-plan.md](2026-11-07-gbc-images-plan.md). It has 9 questions, each with a recommendation; the research is in [2026-11-07-gbc-images-research.md](2026-11-07-gbc-images-research.md). The leaf k5Afkg stays open (`decision`) until he answers. Then import the plan under `9b5RBA` as Stage 10 and close k5Afkg.
2. **How a session chooses a flash board:** [2026-11-07-flash-model.md](2026-11-07-flash-model.md). The agent recommends `--flash PROFILE` at session creation, like `--model`: stored in the bundle meta and an optional checkpoint-header field, checked on restore, and kept across `power-cycle`. Two smaller questions go with it: whether custom board settings are allowed on the CLI, and whether `sav` carries the flash save area.
3. **The two GB Studio flash tests** (`crates/engine/tests/flash_save.rs`) take about 10–12 s each: keep them warm, or move them to the pre-release suite? Fresh clones fail them until the ROM exists, the same way the corpus tests fail.

## Next, in order

1. **Review `e9cba4f`** (L00RuF, the flash model). It changes FFI (`gbcli_reads_boot_rom`), `unsafe` code (the `rom()` slice from `GB_get_direct_access`, the callbacks kept installed), and a persisted format (blob version 4). Also review `d0e891a` (GTXXyr) and the radJxi part of the handoff commit.
2. **mDk1lR:** a standing differential test of `gbcli opcode` against rgbasm. Now unblocked. It replaces further opcode review rounds, and can stand in for the radJxi review.
3. **k22J86**, the agent eval. Images and opcode are now settled enough to run it.
4. Once Ben answers, Stage 10 (GBC images) and the flash session surface.

## On the review flow (Ben asked)

- **What works:** fresh reviewers given the code rather than the author's report; "what must never happen" as the goal; probes that become the fix's red tests; "what in this brief is wrong?" on implementers. That question surfaced both shade design problems before they shipped.
- **What to change:**
  - A parser with a reference program needs a differential test (mDk1lR), not repeated review rounds.
  - Fixers sometimes write the code before the test, so briefs should ask them to break the code by hand to show a test can fail.
  - Nobody reviews the combined session range. A short pass over it before handoff would catch problems that only appear when commits combine.

## Suite time

The hook reported 100–201 s on every commit this session, with three to four agents building at once, so those figures are noise. Re-time on an idle machine with `python3 -I scripts/bench/test-times.py` (gotchas, 2026-11-07). Known additions: the two GB Studio flash tests (about 20 s) and the HuC3 bound tests (about 0.14 s).
