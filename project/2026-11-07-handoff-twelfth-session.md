# Handoff: 2026-11-07 (twelfth session)

This handoff is current until the next commit after the one that adds it. Start with `job orient`, then read this. The previous handoff is [2026-11-07-handoff-eleventh-session.md](2026-11-07-handoff-eleventh-session.md).

## State of play

Ben was away for the first half, then present. The machine restarted once mid-session; nothing was lost but the scratchpad (in `/private/tmp`). Every agent commit was squash-merged after a read of its staged diff, through the pre-commit hook.

**Landed:**

| Commit | Leaf | What |
|---|---|---|
| `3b72e81` | tzEhz3 | Under `--gbc`, color art's DMG version is derived by the shade cascade (`images/derive.rs`); `name.dmg.png` overrides it (`dmg_override_size`); `shared_shade` is scoped to one tile or object; misplaced `.dmg.png` files are layout errors. |
| `764e4b3` | — | The recreated `job` state (see "Mistakes"), and a gotcha. |
| `bbc8de1` | 9RaRvi | One integration-test binary per crate (`tests/it/main.rs`; 107 binaries became 31, 1,550 tests before and after). Five files keep their own process and say why. `music_equiv` compares from the boot handover; 3 samples stay warm, all 15 whole songs pre-release. `build_safety` pays its waits once, not per source. |
| `da98a90` | Aifjyb, k5Kadv | The 81cf3d2 review fixes: `TileEntry { index, flip, palette }` replaces `tile_indices`/`tile_palettes` (renamed `tile_map`); one `NamedPalettes` rule set in `crates/protocol/src/palettes/rules.rs`, applied on deserialize and by the file parser; `BuildMade.target` defaults to dmg. Indexed art under `--gbc` (`images/indexed.rs`). |
| `75445cf` | FtrNML | The doctor's `git check-ignore`, and its test's `git init`, ignore a hook's `GIT_DIR`. The test had set `core.bare = true` in the main checkout's `.git/config`, which broke every git command. |
| `343e7b5` | ahKOkk | A restore across a compatible rebuild no longer writes the old flash save over the new build's code. Restore and power cycle share one rule: a changed sector reverts to the build only where the new build has a non-$FF byte, so a save in the `rgbfix -p 0xFF` padding survives rebuilds. `sav`'s both-or-neither claim is qualified. |
| `0f199b5` | — | Ben's ruling on indexed sprite sheets, in the plan under question 4. |
| `2efed71` | BiIfeX | Indexed sheets' GBC OBJ palette is the named palette reordered by the sheet's OBP; slots are keyed by (palette, OBP). The wire now refuses a repeated dmg color and a twice-assigned image. `ImageAsset`'s read check is fuller. |

**Ben's ruling this session:**
- Indexed sprite sheets reorder their named palette per OBP; slots are keyed by (palette, OBP). It's in the plan under question 4.

**Decisions made without Ben (two-way doors):**
- **Flash across a rebuild:** "the new build's code stays" means sectors where the new build has a byte other than $FF, not "inside the ROM file". Every gbcli build is $FF-padded, so a save there must survive.
- **Warm suite:** three song samples stay warm (Yyna, Strap in and Suit Up, Wyrmhole), chosen for effects, subpatterns and speed.
- **OBJ slot order:** assigned (indexed) palettes take the first OBJ slots and packed color palettes follow, so a palette swap's slots stay fixed. The packer numbers its slots after them.
- **Field rename:** `tile_indices` is renamed `tile_map`.

**Reviews this session:**
- **`81cf3d2`:** 2 medium findings, fixed in `da98a90`.
- **`54c9d3e`:** nothing serious. The OBJ `-c` spec needs `#none` in slot 0, settled against rgbgfx 1.0.4 and noted on 0racny.
- **`6c5b029`:** its fixes held, but one medium finding was new: the restore path. Fixed in `343e7b5`.
- **`3b72e81` and `da98a90`:** 1 high (the indexed-sheet colors, Ben's ruling) and 1 medium, fixed in `2efed71`.

Labeled `reviewed`: cnXB3b, d7DbmT, ZA5Z7c, vo5gRl, WmNCxc, DkhKgi, tzEhz3, Aifjyb, k5Kadv.

**Suite:** at `2efed71`, `python3 -I scripts/bench/test-times.py` summed 23.5 s wall over 32 binaries at load 20.7. The hook's whole-run figure was 32-33 s at that load, so re-time on an idle machine before moving more tests (speed doc, update block).

## Mistakes, so the next session doesn't repeat them

- **`git reset --hard` reverted the tracked `.jobs/log`.** Undoing a staged squash that way erased the session's job writes since the last commit. They were recreated under new IDs: jJjaK9 is Aifjyb, XeGILY is 9RaRvi, g83MoZ is ahKOkk. The gotcha says how to undo a squash safely.
- **A test can damage the repo it runs in.** A test running `git init` under a hook inherited `GIT_DIR` (fixed in `75445cf`). If git ever says "must be run in a work tree" in the main checkout, check `git config core.bare` first.

## In flight at handoff

Nothing. No agents running, no worktrees.

## Needs Ben

- **Override colors and the shade rule:** a `name.dmg.png` override's colors join the project-wide shade cascade, so adding an override can change the shade rule chosen for the whole project. This follows "one color, one shade, everywhere". Confirm or rule otherwise (tzEhz3's note).
- **For LoadHigh:** `tile_indices` is now `tile_map: Vec<Option<TileEntry>>`, each entry `{index, flip, palette}`.
  - A sheet entry's `index` is the unit's top tile.
  - It is filled only for indexed art under `--gbc`; DMG builds and color art still send it empty until 0racny.
  - LoadHigh's request to fill it on every build (the note on 7gGMeZ) is still open.

## Next, in order

1. **Review the unreviewed commits:**
   - `343e7b5` (ahKOkk): flash state across a rebuild. Review it first.
   - `2efed71` (BiIfeX): wire types again, and 0racny builds on it.
   - `75445cf` (FtrNML): small.
   - `bbc8de1` (9RaRvi): test layout only. A short check that no test is lost or mis-scoped.
2. **0racny, the joint conversion, which is now unblocked.** Read all its notes first. They cover:
   - the `#none` OBJ spec;
   - `census::alpha` before `pack`;
   - keeping the color pixels beside the DMG version (`images/mod.rs` is ~520 lines, so move the per-image rgbgfx and preview arm out);
   - the `.dmg.png` transparency check, plus the alpha of a color background with an override, both real gaps;
   - assigned OBJ slots first, then `pack`'s `reserved`;
   - the gbc_palettes message cap.
   After 0racny come zcxvjc and 7gGMeZ, then the starter (gbHEPa).
3. **k22J86**, the agent eval, once Stage 10's images land or before, as Ben prefers.
4. **sUOJQb**, the protocol version over the wire for LoadHigh's doctor: small and independent.
