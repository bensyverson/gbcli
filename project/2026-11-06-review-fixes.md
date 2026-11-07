# Plan: fixes from the review of the stage-3 commits

A code review on 2026-11-06 covered commits 3b49997 (bundle store and stdio server), c5da0fe (`--until` cadence) and 47957a9 (per-pixel layer identity). Another model wrote them in another harness. The review used four reviewers, one per surface, and most findings were confirmed by a throwaway probe or a trace with line numbers. fmt, clippy `-D warnings` and the test suite were all green at 47957a9: the bugs live where the tests don't look.

This doc is the review's record and the plan to fix what it found. Line numbers are as of 47957a9.

## What the review found

- **Silent corruption.** The layer patch grew SameBoy's saved video section, but changed no version: not `blob::VERSION`, not `STRUCT_VERSION`, not the core commit. SameBoy loads a shorter section and shifts every later field, so a pre-patch checkpoint or boot cache loads without complaint and diverges within 1–3 frames (run). The handoff and vendoring docs claimed such checkpoints are rejected.
- **`--until` is not exact.** Four confirmed misses against a per-instruction oracle:
  - a `00:D000` WRAM symbol is armed as bank 0, which SameBoy never reports;
  - `frame` with `ly` collapses to the scanline trigger, missing LCD-off vblanks and stopping one line late at LY 144;
  - a read hit masks a write hit in the same instruction (`inc [hl]`);
  - cart RAM smaller than its bank switching aliases.

  The root cause is a hand-written inverse of `mem::banked_target` in `cadence.rs` that has already drifted from it.
- **Session state bugs in the server:**
  - two undos toggle instead of walking back;
  - a `--once` watch is restored from `session.json` after it fires;
  - a `write` or `checkpoint` before the first run pins the session at power-on;
  - `run --allow-rom-change` is ignored;
  - `run --checkpoint` records the old rev;
  - a bank-qualified poke ignores its bank.
- **Robustness:**
  - locking a missing session creates a broken one that blocks `prune`;
  - `rm` and `prune` delete without the lock;
  - `parse_duration` panics on non-ASCII input and overflows;
  - region offsets overflow `u16`;
  - a non-UTF-8 byte ends `serve`, and nothing catches a panic;
  - the wall guard leaks a 60 s thread per request;
  - a failed `--read` discards the run it followed.
- **Wrong answers:**
  - `checkpoint_remedy` twins the engine's `CheckpointError::remedy()` and has drifted from it, so codes, exits and remedies are wrong;
  - `rev_mismatch` blames the last reader;
  - the event log has no args and no rotation;
  - write-path `Unsupported` skips the gap ledger.
- **Principles:**
  - stringly typed `from`/`since`;
  - unknown request fields accepted silently (a typo'd `rev` skips the stale-rev guard);
  - files of 1,360 lines (`handle.rs`), 854 (`store.rs`) and 495 (`drive.rs`);
  - the CLI adapter decides session creation;
  - the oracle tests cover none of the banked, LCD-off or CGB cases.

Each leaf below restates its own findings in full.

## Rulings (2026-11-06, delegated to the agent by the user)

These are recorded in `project/2026-11-06-design.md`, in the sections they rule on:

- **§8, unwritten memory in a stop trigger.** A condition that triggers a stop (`--until`, a breakpoint or watch `--if`) reads an unwritten byte as "not yet". At that boundary the whole condition counts as not holding, without an error, and the cadence keeps waiting on writes. If the run ends without the condition holding while a byte it reads is still unwritten, the stop record carries a warning naming the byte, the symbol and the §8 remedy. Anything that *returns a value* (`--read`, `check`, `show`) keeps the `uninitialized_read` error. `--allow-uninit` keeps its meaning: read the power-on value. The why: a trigger asks "has it happened?", and an unwritten variable has not happened yet; a read asks "what is it?", and that answer would be garbage.
- **§3, a target not reached.** A run with a target (`--to`, `--until`) whose budget ends first is `budget_exhausted`, exit 3, whether the budget was explicit or defaulted. Reaching the budget is the answer only for a run with no target (`run --frames 10`). This reverses the last sentence of the stage-3 correction in §3.
- **§10, undo walks back.** Consecutive undos walk back through history: the first restores `~1`, the next the state before that, and so on. Any other head change resets the walk. Undo past the oldest kept entry is refused with a remedy naming the oldest. Redo is out of scope.

## Order

The SameBoy version fix comes first because it is the only silent data corruption. The `--until` and layer work is engine and backend only, so it runs in parallel with the server chain.

`handle.rs` is split before any server fix (a pre-carve with no behavior change), so the server leaves don't collide in one 1,360-line file. In the bundle crate, error codes come before the store work, because the store work constructs the new error variants. The protocol typing leaf blocks the CLI leaf (LUax7P), because the CLI would otherwise grow its own parse of `from`/`since`.

A fresh re-review closes the stage.

The `tasks:` block below was imported on 2026-11-06 as container `jPKQX8` under root `9b5RBA`; the tracker is the plan from then on. The CLI leaf `LUax7P` is blocked on the protocol leaf `BDAT2m`.

```yaml
tasks:
  - title: 'Review fixes: stage-3 commits 3b49997..47957a9'
    desc: |
      Fixes from the 2026-11-06 code review of the bundle store, stdio server, --until cadence and layer-identity commits. The findings, rulings and order are in project/2026-11-06-review-fixes.md; each leaf restates what it needs.
    children:
      - title: Refuse save states from before the layer patch, and key the boot cache on core identity
        ref: sbver
        desc: |
          Why: the only silent data corruption the review found. Commit 47957a9 grew SameBoy's GB_fifo_item_t from 4 to 6 bytes, so the saved video section is 32 bytes larger. But nothing that identifies the format changed: crates/sameboy/src/blob.rs VERSION is still 2, SameBoy/Core/gb.h STRUCT_VERSION is still 15, and SameBoy/COMMIT is unchanged. SameBoy's read_section (save_state.c:924-944) accepts a smaller saved section and memcpys it, so every field after bg_fifo shifts.

          A reviewer ran it: a mid-frame state of layers.gb saved by 47957a9^ loads on HEAD (168,070 vs 168,102 bytes) and its PC diverges from a native run within 1-3 frames. The checkpoint's core-commit field matches, so not even the OtherCommit warning fires. The boot cache key (crates/bundle/src/store.rs:278-287: ROM, model, boot ROM, seed) has no core identity either, so an existing .gbcli/cache/boot-*.gbck is reused.

          What: a state from before the patch is refused with a clear error and remedy, never loaded. The core identity a checkpoint records includes the patch set, so a future patch that changes the layout is refused the same way without anyone remembering to bump a number. The boot cache is keyed on that identity, so a stale cache entry is rebuilt rather than reused. Prefer one mechanism that covers future patches (e.g. a hash of patches/ folded into the reported core version at build time) over a hand-bumped constant alone, but bump blob::VERSION now regardless.

          Docs, corrected in place as marked block quotes (house rule: a wrong documented cause is worse than none): project/2026-11-06-handoff.md says pre-patch checkpoints "are rejected by the core with a mismatch message"; project/2026-11-06-sameboy-vendoring.md (display.h bullet) says the envelope's core-commit field "already rules out" incompatibility. Both are false.

          Seams: crates/sameboy/src/blob.rs, crates/sameboy-sys (build.rs or the COMMIT/version accessor), crates/bundle/src/store.rs (boot-cache key only — the store leaf owns the rest of that file; coordinate if it is in flight). Out of scope: layer capture behavior (its own leaf).
        criteria:
          - A test loads a blob of the pre-patch format and asserts it is refused with a named error and remedy (a committed fixture state from 47957a9^, or a synthesized one with the old section size)
          - The recorded core identity changes when any file in crates/sameboy-sys/patches/ changes (test or build-script check)
          - The boot-cache key includes the core identity; a test shows a cache entry from another identity is not reused
          - The handoff and vendoring doc claims are corrected in place as marked block quotes
      - title: Layer capture correctness after load, Rendering::Off and repeat vblanks, with tests for the hard cases
        ref: layers
        blockedBy: [sbver]
        desc: |
          Why: per-pixel layer identity (47957a9) is right on the paths its tests exercise, but the review found stale or misleading maps on paths they don't, and the tests miss every hard priority case.

          Defects (crates/sameboy unless noted):
          - Restoring a checkpoint mid-frame with capture on: load (lib.rs:413-458) restores the screen but leaves hooks.layers and layers_skip_next alone, so the next vblank publishes rows from the pre-load timeline. Confirmed by a run: all 64 of object 0's pixels tagged Background. The Core trait doc implies the next frame's map is good.
          - With Rendering::Off, supports(PixelLayers) says Yes and set_pixel_layers(true) succeeds, but on_vblank returns before the layers block (hooks.rs:162), so pixel_layers() always errors, blaming LCD-off. Either refuse up front or make it work.
          - set_pixel_layers(true) while already on throws away the current valid map (lib.rs:310-316); it should be idempotent.
          - Plausible: a CGB GB_VBLANK_TYPE_REPEAT vblank returns before the buffer is refilled (hooks.rs:162-164), so untagged pixels in the next frame inherit old tags rather than the documented "background". Also plausible: on DMG, STOP with the LCD on blacks the screen at a normal vblank (display.c:232-235), and a map is published for a black screen. Confirm each with a test and fix, or show it can't happen.
          - The DMG window pixel-insertion glitch pixel (display.c:681-684, 1111-1115) is tagged Background because the inserted item is {0}. Your call whether Window is truer; record it in the vendoring doc.

          Typing and docs:
          - The tag encoding (0 / 1 / 2+n) is magic numbers in display.c and in hooks.rs layer(). Name it once in C (a define or enum in the patch, shared through the shim) and decode it into the Rust enum in one place.
          - Revert the patch's needless change to display.h:33 (a no-break space replaced in an upstream comment), so the patch is smaller to rebase.
          - Fix gb.h:679 "one byte per 160-pixel row": it is one byte per pixel, in rows of 160.
          - save() clones frame.layers (46 KB) into a Blob that never encodes it (lib.rs:407).
          - The vendoring doc's "+0.44% off" figure came from a modified sb_bench.c (CLOCK_PROCESS_CPUTIME_ID) that is not in scripts/. Commit the benchmark under scripts/bench/ with the exact command, and re-measure if needed. Its claim that "with the buffer NULL the only cost is one branch" ignores the 6-byte FIFO items and render_line's wider object buffer; correct that too.

          Tests (crates/sameboy/tests/layers.rs, with layers.asm extended or a second ROM built by scripts/build-roms.sh):
          - CGB: LCDC.0 as master priority, the attribute priority bit, and index-based object priority.
          - Which object wins where two overlap (the dmg-acid2 check accepts any opaque object pixel, so a wrong index passes).
          - LCDC.1 off; a BG-priority object over BG color 0 (the object wins); a mid-frame load; mid-line window disable.
          - Make the "busy" variant assert it took the per-pixel path, or drop the claim.
          - layers.rs:68 says "the map matches the pixels" over assertions that check only lengths; make it true.

          Out of scope: the state-format version (sbver, which lands first because both touch the save path).
        criteria:
          - A test restores a mid-frame checkpoint with capture on and asserts the next published map matches the pixels (or that no map is published for that frame), red before the fix
          - Tests cover CGB master priority, attribute priority and index priority; overlapping objects (exact winner); LCDC.1 off; BG-priority object over color 0
          - Rendering::Off either refuses capture up front with a remedy, or produces maps (test either way)
          - The tag encoding has one named definition in C and one decode in Rust
          - The layer benchmark is committed under scripts/bench/ and the vendoring doc's figures name its command
      - title: Make --until cadence sound against bank, LCD-off and same-instruction hits
        ref: cadence
        desc: |
          Why: c5da0fe and design §11 claim the cheap write cadence is exact ("Symbols and region forms stay on the cheap path and are exact"; "6.9x faster, and exact"). The review refuted that with four misses, three of them run against a per-instruction oracle. A late or missed --until stop is a silent wrong answer, the worst failure this tool has.

          The misses:
          1. A WRAM symbol at $D000+ with bank 00 (a DMG game linked with rgblink -w) is armed as BankSel::Bank(0) (cadence.rs:117). The write hook reports gb->cgb_ram_bank, which SameBoy never sets to 0 (memory.c:1651-1653, gb.c:1724), while the evaluator reads bank.max(1) (mem.rs:56). The stop is never reached.
          2. A predicate reading both frame and ly: join takes max(), treating Scanline as finer than Vblank (cadence.rs:34, 42-43), and drive.rs:278 never looks at used.frames. With the LCD off, frame advances while LY stays 0, so `frame >= 3 && ly == 0` never stops. With it on, the stop is one scanline late at LY 144. The cadence is really a set of triggers, not a total order.
          3. A read hit masks a write in the same instruction. crates/sameboy/src/watch.rs:154-158 records the first hit per instruction, and the until path needs access == Write (drive.rs:273). With a false read watch on $C000, `inc [hl]` records the read and drops the write, so `[$C000] == 5` never stops. Write watches can already be masked the same way.
          4. Cart RAM aliasing: the real offset is masked by mbc_ram_size - 1, but the hook reports the raw mbc_ram_bank and address (gbcli_shim.c:42-47, 80). An 8 KB cart with RAMB=1 writing $A000 changes bank 0's byte without matching the bank-0 candidate. Likewise a 2 KB cart at $A800. Plausibly MBC2's mirrored 512 bytes and the Game Boy Camera's capture too.

          The root cause of 1 and 4: region_candidate and banked_candidate hand-invert mem::banked_target, with magic $C000/$D000/$A000 constants. Derive the candidates from the same mapping the evaluator reads through, or canonicalize both sides to one (area, bank, offset) identity, so they cannot drift. Where a mapping can't be made exact (Camera capture, MBC quirks), fall back to every-instruction rather than guessing.

          Docs: correct design §11's "exact" claims in place, and the trigger table that lists sram:/rom: in two rows. A marked correction block was added at review time pointing here; replace it with the as-built truth.

          Tests: extend crates/engine/tests/until.rs's oracle (it is independent, sharing only the evaluator, which is fine) to bank-switched ROMX and SRAM, a -w build's WRAMX symbols, CGB WRAMX via SVBK, small SRAM aliasing, LCD off, mixed frame+ly, and read-watch/write interplay. Each case is red before its fix.

          Seams: crates/engine/src/cond/cadence.rs, mem.rs, crates/engine/src/debug/drive.rs (the trigger loop), crates/sameboy/src/watch.rs, crates/sameboy-sys/shim. Out of scope: uninitialized-read semantics and drive.rs tidying (the untilsem leaf, after this one).
        criteria:
          - Each of the four misses has an oracle test that fails before and passes after (WRAMX bank-0 symbol, frame+ly with LCD off and on, inc [hl] under a read watch, aliased small cart RAM)
          - Write candidates are derived from the evaluator's own memory mapping (no second hand-written map); a test or type makes drift impossible
          - Design §11 states the cadence rule as built, with no claim the oracle doesn't back
          - until_bench re-run, and the cost figure in §11 updated with its command
      - title: '--until semantics: unwritten bytes mean "not yet", error stop records, and drive.rs tidying'
        ref: untilsem
        blockedBy: [cadence]
        desc: |
          Why: today a predicate reading never-written memory fails the run at once, even from boot, and its remedy says "run from boot". So `run --until 'u16[wFrame] >= 60'` fails on a fresh session, the most natural first use. Mid-run, `[wA]==1 && [wB]==2` fails at the first write to wB if wA is still unwritten.

          Already decided (design §8 ruling, 2026-11-06): a condition that triggers a stop (--until, breakpoint or watch --if) reads an unwritten byte as "not yet". At that boundary the whole condition does not hold, without an error, and the cadence keeps waiting on writes. If the run ends without the condition holding while a byte it reads is still unwritten, the stop record carries a warning naming the byte, the symbol and the §8 remedy. Value reads (--read, check, show) keep the uninitialized_read error. --allow-uninit keeps its meaning (read the power-on value). Not per-subexpression: never treat the byte as 0.

          Also in this leaf:
          - DebugError::Until's stop record says reason: predicate even though the predicate never held (both Until error sites in drive.rs). It should carry the true reason.
          - DebugError::UntilSyntax has no test; neither does a mid-run DebugError::Until.
          - After a passed-over exec candidate, the core stops before PC X and the loop calls decide on the same hit again (drive.rs:175-208). That evaluates the condition twice and duplicates --allow-uninit warnings. The exec-filter fix in 3b49997 is otherwise correct, and it also fixed a breakpoint on a 4,096-instruction chunk boundary that no test pins; add that test.
          - Condition::cadence() and write_candidates() each run cadence::analyze; until.rs:23-33 should analyze once.
          - Stale comments: until.rs:17-19 ("a finer cadence leaves this empty" — only EveryInstruction does); a dangling doc link on write_candidates in cond/mod.rs.
          - drive.rs is 495 lines; move the fired/until logic into until.rs to get under ~400.
          - until_bench's doc says the default is 2,000 iterations; the design cites 10,000. Make them agree.
          - Remove the junk `let _ = &mut config;` in the exec-filter regression test (tests/debugger.rs).

          Seams: crates/engine/src/cond/, crates/engine/src/debug/. The server's stop_of (handle.rs) not matching DebugError::Until is the server-state leaf's.
        criteria:
          - "Tests: --until on unwritten memory from boot runs until the write makes it true; a budget end with a still-unwritten read carries the warning; --read on unwritten memory still errors; a breakpoint --if on unwritten memory does not stop"
          - DebugError::Until's stop record carries the true reason (test); UntilSyntax and mid-run Until each have a test
          - A test pins that the condition is evaluated once per passed-over exec candidate, and a breakpoint on a chunk boundary is caught
          - drive.rs under ~400 lines; design §8 and §11 describe the as-built behavior
      - title: 'Address grammar: overflow, region bounds, hex bank prefix, symbol offsets'
        ref: addr
        desc: |
          Why: crates/engine/src/addr.rs (3b49997) can panic the server, and it silently accepts addresses that don't exist.

          - addr.rs:225-227: hram:0x80, io:0x100 and oam:0x200 overflow u16 (a panic in debug builds, kills serve; in release the address wraps to $0000).
          - Region offsets are never range-checked: oam:0xA0 gives $FEA0, vram:0x4000 gives bank 2, and wram:0x8000 gives bank 8, on models that lack those banks. Each region's size depends on the model and cart; the error should name the region's real size.
          - 0x03:0x4123 is rejected because the region-detection rule (addr.rs:96-98) treats 0x03 as a region name; $03:$4123 works. Design §7 says the prefix is optional in both parts.
          - split_offset turns Main+$10000 into Main by falling back to 0. There is no - offset, though the condition language accepts symbol minus a constant.

          Every error names the thing, says what happened and gives a corrected form (project/agents/cli-design.md). Seams: addr.rs only; the condition language's own address parsing is in cond/ — if the two duplicate a rule, share it rather than fixing both. Out of scope: poke's bank handling (server-writes leaf).
        criteria:
          - Tests for every out-of-range region offset (hram, io, oam, vram, wram, sram, per model) return an error with a remedy, never a panic or wrap
          - 0x03:0x4123, $03:$4123 and 03:4123 parse identically (test)
          - Symbol plus or minus an offset either resolves exactly or errors; an overflowing offset errors (test)
      - title: Split server/src/handle.rs into modules (no behavior change)
        ref: split
        desc: |
          Why: handle.rs is 1,360 lines, three times the house ceiling. Five fix leaves touch it, and in one file they would collide (delegation.md: pre-carve the shared seam in one serial commit, then fan out).

          What: a pure move. No behavior change: the existing tests pass unchanged, and no test is edited except for its module path. A suggested split (your call where the boundaries fall):
          - handle/mod.rs: Handler, session_request, dispatch, machine_for, warm, log
          - run.rs: run, step, after_run_error, budget_of, watch_wall
          - config.rs: break, watch, delete
          - history.rs: checkpoint, restore, undo, checkpoint_frame
          - writes.rs: apply_writes, rom_offset
          - reads.rs: show, read, read_values, since
          - bundle_ops.rs: sessions, gaps, prune, rm

          Each module keeps its tests beside it.

          Out of scope: any fix, however tempting. Note anything you see on the leaves that own it.
        criteria:
          - No file in crates/server/src over ~400 lines
          - The test suite passes with no test changed except module paths (state the diff stat of tests)
      - title: 'Bundle error codes: one mapping from engine errors, no wildcards, distinct variants'
        ref: errors
        desc: |
          Why: crates/bundle/src/error.rs (3b49997) gives several failures the wrong code, exit code and remedy, and contradicts both the engine's documented mapping (crates/engine/src/checkpoint_error.rs:72-99) and design §5-6.

          checkpoint_code (error.rs:166-175):

          | Variant | Mapped to | Should be |
          |---|---|---|
          | ModelConflict | checkpoint_mismatch, exit 4 | bad_request, exit 2 (§6 step 3) |
          | SymbolsMissing | checkpoint_unreadable (via the _ wildcard) | rom_changed |
          | PatchConflict | checkpoint_unreadable | rom_changed |
          | PatchRefused | checkpoint_unreadable | checkpoint_mismatch |
          | Unknown | checkpoint_unreadable | bad_request |

          For example, a user who rebuilds without a .sym gets exit 4 and "recreate the checkpoint" instead of "rebuild with -n game.sym, or pass --allow-rom-change".

          checkpoint_remedy (error.rs:177-203) re-implements CheckpointError::remedy() instead of calling it: a twin that has already drifted. Use the engine's code and remedy, and match without a wildcard, so a new variant fails to compile rather than mapping silently.

          Other mappings:
          - Io maps to refused, which the design reserves for "needs a flag", so disk-full shows as an exit-2 usage error. Find or add the right code.
          - Configuration covers several unrelated failures, all reported as bad_request with the remedy "check the session's model and seed": an unparseable session.json (store.rs:312), "session already exists" (:125), and "the boot ROM didn't reach the handoff" (:267, which also hides the run's real code). Split it into named variants, each with its own code and remedy.
          - SetupError::Core maps to checkpoint_mismatch (error.rs:207), but it is the core refusing a ROM.

          Also, in protocol: ErrorCode's hand-written Display (protocol/src/error.rs:102-128) twins the serde spelling. Derive one from the other, or test them together.

          Seams: crates/bundle/src/error.rs, crates/protocol/src/error.rs. The store leaf comes after and will use the new variants; keep store.rs edits to the call sites only.
        criteria:
          - A test per CheckpointError variant asserts its ErrorCode, exit code and remedy through the bundle, and the match has no wildcard arm
          - Io, an unparseable session.json, an existing session, a failed boot and a core ROM refusal each have a distinct, tested code and remedy
          - ErrorCode's Display and serde spellings cannot disagree (derived, or one test over every variant)
      - title: 'Bundle store: session lifecycle, locking, crash consistency and file split'
        ref: store
        blockedBy: [errors]
        desc: |
          Why: crates/bundle/src/store.rs (3b49997) can brick a session, race a deletion against a writer, and corrupt history after a crash. At 854 lines it is also twice the house ceiling.

          Lifecycle and locking:
          - lock() runs create_dir_all before read_meta (store.rs:460-465), and the server locks first, so one request with a mistyped session name leaves sessions/<name>/lock with no session.json. After that, exists() is true, so serve --rom skips creation and create() refuses; every request fails no_session, and the remedy lists the broken name. Bundle::prune (bundle.rs:143) then fails on the missing session.json, so every prune fails until someone deletes the directory by hand. Locking must never create a session, and prune must survive a broken directory.
          - remove() (store.rs:420-430, called by rm and prune) deletes without the session lock. A process mid-run then recreates the session through atomic_write's create_dir_all, and unlinking the lock file lets two writers lock different inodes at once. Take the lock to remove, and keep the lock file outside the directory being removed (or otherwise make this race impossible).
          - Prune calls remove(true), which silently deletes sessions that hold named checkpoints, although rm refuses to. It also judges idleness by session.json's mtime, which only head and debugger edits touch, so a session used only for show, read or events looks idle. Record last use explicitly.
          - parse_duration (bundle.rs:198-212): split_at(len-1) panics on a non-ASCII last character ("7é" kills serve), and n*60*60*24 is unchecked. In release, "213503982334602d" wraps to about 17 h and prunes everything idle that long. Use checked arithmetic and char-aware parsing.
          - Session creation is decided in the CLI adapter (crates/gbcli/src/main.rs:92-114: create when --rom is given and the session is missing, with default model and seed), which also silently ignores --rom when the session exists with a different ROM. Move that decision into the library, and refuse (or explicitly allow) a mismatched ROM with a remedy.

          Integrity:
          - write_head (store.rs:336-343) writes the head, then the server writes session.json. A kill between them leaves head = N+1 with meta = N. The next save then copies that orphaned head to history/sN, overwriting the real sN. Make the pair crash-consistent; e.g. record the rev in the head file, or write history from meta's rev.
          - There is no fsync of file or directory, and a failed rename leaves a .tmp.<pid> file. Decide whether power-loss safety is in scope (state the choice in the design doc), and clean up temp files either way.

          Paths and names:
          - create() joins a relative ROM path to the project directory but uses a relative .sym as given (relative to cwd) (store.rs:129 vs 134-140), and the .sym error reports a path it did not read (:592). Resolve both the same way.
          - checkpoint() (store.rs:346) doesn't validate the name on read, so restore "../history/s3", or an absolute path, reads any .gbck. Use validate_checkpoint_name.
          - The CheckpointMissing remedy for ~N lists raw filenames sorted lexically ("s1.gbck, s10.gbck, s2.gbck"); show ~N forms in order.
          - validate_name accepts names starting with "-", which a CLI will read as flags.
          - Under GBCLI_DIR, project() is the directory's parent whatever it's called; document it, or make it explicit.
          - The boot cache shares symbols_live across sessions with different .sym files; key or strip accordingly.

          Split store.rs (history, checkpoints, boot cache, io helpers) to get under ~400 lines.

          Tests: the bundle.rs walk-up test breaks when GBCLI_DIR is set in the environment; isolate it. Add tests for GBCLI_DIR discovery, the boot-cache key, reuse and corrupt-cache fallback, and ~0 and ~N past the eight kept.

          Seams: crates/bundle (all), crates/gbcli/src/main.rs (creation moves out). The event log (events(), log.rs) is the eventlog leaf's; the boot-cache core-identity key is sbver's (landed first).
        criteria:
          - "Tests: a request naming a missing session leaves no trace on disk; prune succeeds with a broken session directory present"
          - "Tests: rm and prune take the session lock (a held lock makes them wait, then session_busy); prune skips sessions with named checkpoints or reports them"
          - parse_duration rejects non-ASCII and overflowing input with an error (tests)
          - A test simulates a kill between the head and meta writes and shows history is not overwritten
          - Relative ROM and .sym paths resolve against the same base (test); checkpoint names are validated on read (test)
          - Session creation from --rom lives in the library; gbcli main.rs only wires it
          - No file in crates/bundle/src over ~400 lines
      - title: 'Server session state: undo walk-back, once-watches, first-run boot, rev and flag plumbing'
        ref: serverstate
        blockedBy: [split, store]
        desc: |
          Why: the server (3b49997) gets session state wrong in ways an agent would act on. The reviewer confirmed most of these by running them.

          - Undo toggles (handle.rs:694-730 with store write_head): it always restores ~1, and the replaced head goes to history as rev-1, so a second undo restores what the first replaced. Three 1-frame runs and three undos give frames 2, 3, 2. Already decided (design §10 ruling, 2026-11-06): consecutive undos walk back through history; any other head change resets the walk; undo past the oldest kept entry is refused with a remedy naming the oldest; redo is out of scope.
          - --once watches never go away: run and step don't copy machine.debugger_config() back into meta.debugger (only break, watch and delete do: handle.rs:561, 596, 626), so machine_for restores the watch from session.json on the next request. Confirmed: two runs both stop on watchpoint #1.
          - write, restore, undo and checkpoint take their machine with Need::Read, which with no head is a power-on machine. So `poke` before the first run saves a power-on head, every later run starts inside the boot ROM, and the boot cache is skipped. checkpoint at rev 0 writes a power-on checkpoint. They should start where run would (the boot cache).
          - run --allow-rom-change is ignored: machine_for passes LoadOptions::default(), and only restore reads the flag.
          - run --checkpoint records meta.rev before save_head bumps it (the response says s1 but checkpoint.rev is s0).
          - stop_of (HEAD handle.rs:836) matches DebugError::Eval but not the newer DebugError::Until, so an --until failure returns a stop record without saving the head that record describes. The reported stop and the head then disagree.
          - The warm machine is keyed on session name and rev only (handle.rs:288-294). If another process removes and recreates the session and runs it to the same rev, the warm machine is reused, even with a different ROM. rm never clears self.warm. Key it on something a recreate changes.
          - step saves the head with actor None, losing the actor. restore and undo drop load warnings (undo drops all of them). Poke-to-ROM's specific remedy is replaced by a generic core_error remedy.
          - The two-process test asserts only rev, which comes from meta; it would pass with a stale warm machine. Compare PC or ticks.

          Seams: the split handle modules (history, config, run, mod). Out of scope: run robustness (serverrun), the event log (eventlog), typing (protocol).
        criteria:
          - "Tests: three runs then three undos walk back three states; undo past the oldest kept entry is refused with a remedy; a run after an undo resets the walk"
          - "Test: a --once watch fires once across two run requests and is gone from session.json"
          - "Test: poke before the first run, then run, starts from the boot cache (since: boot)"
          - "Tests: run --allow-rom-change after a rebuild loads; run --checkpoint records the new rev; an --until failure saves the head its stop record describes"
          - "Test: rm and recreate at the same rev does not reuse the warm machine; the two-process test compares PC or ticks"
      - title: 'Server run robustness: validate before emulating, wall guard, serve loop, the budget ruling'
        ref: serverrun
        blockedBy: [serverstate]
        desc: |
          Why: several failures in the run path throw away work or kill serve, and the budget answer contradicts the design's "no answer is exit 3".

          - read_values hardcodes allow_uninit: false (handle.rs:939). It runs after the emulation, and a failure returns before save_head, so `run --frames 1 --allow-uninit --read [$C100]` errors, the rev stays the same, there is no stop record, and up to 60 s of emulation is lost. A bad --read expression and an invalid --checkpoint name have the same ordering problem (the name is validated after the run, handle.rs:389-391). Validate and compile everything before emulating; honor --allow-uninit; never discard a completed run because a post-read failed (save the head and report the read error with the stop record).
          - The wall guard (watch_wall, handle.rs:844) spawns a thread that sleeps 60 s and is never cancelled, on every run and step. A harness stepping at about 1 kHz holds about 60k sleeping threads until thread::spawn panics. Use one cancellable watchdog. Also: step never rewrites the reason to WallGuard (it reports user_break), and read, restore and boot-cache creation have no guard, though design §3 says the guard backs every request. Make the duration injectable so a test can fire it.
          - serve (serve.rs:17-18) ends on one non-UTF-8 byte (lines() plus `line?` gives InvalidData), and nothing catches a panic. A bad line gets a bad_request response and the loop continues. A panic in one request becomes an internal error response, not a dead server; state in the design doc what state is trusted after one (e.g. drop the warm machine).
          - Already decided (design §3 ruling, 2026-11-06): a run with a target (--to, --until) whose budget ends first is budget_exhausted, exit 3, explicit budget or not. Reaching the budget is the answer only without a target. Today `run --to X --frames N` that never reaches X returns ok, exit 0 (handle.rs:415-419).
          - Magic numbers: Budget::instructions(100_000) (handle.rs:496), events(Some(20)), and a "60 s" in a message that duplicates WALL_GUARD. Name them. Remove `let _ = flag;` (handle.rs:505).

          Seams: the split run and reads modules, crates/server/src/serve.rs. Out of scope: the protocol shape of errors (protocol leaf).
        criteria:
          - "Tests: run --allow-uninit --read on unwritten memory succeeds with a warning; a bad --read expression or --checkpoint name fails before any emulation (the rev unchanged, no ticks spent)"
          - "Tests: the wall guard fires on run and step with an injected duration, both reporting wall_guard; a thousand step requests leave no lingering threads"
          - "Tests over serve's stdio: a non-UTF-8 line gets bad_request and the next line is answered; an injected panic gets an error response and the loop continues"
          - "Test: run --to X --frames N that doesn't reach X is budget_exhausted, exit 3; run --frames N alone is ok"
      - title: 'Event log and gap ledger: args, rotation, the right writer, every Unsupported'
        ref: eventlog
        blockedBy: [serverrun]
        desc: |
          Why: design §10 makes events.jsonl the audit trail and the gap ledger the record of every unsupported feature an agent hit. As built (3b49997) both are incomplete, and rev_mismatch blames the wrong actor.

          - EventLogLine (protocol/src/session.rs:55-72) records the op name but not its args; §10 requires "the op and its args". Use the typed Op, not a String.
          - No rotation at 10 MB (§10), and events() (store.rs:507-526) reads and parses the whole file, including on every rev_mismatch. Read the tail.
          - rev_mismatch's "changed by" (handle.rs:216-221) takes the newest event with rev_after == meta.rev. Reads, failures and earlier refusals all log rev_after == rev, so a later show by another actor is blamed, and only 20 events are scanned. Match rev_before != rev_after, and don't let readers push the writer out of the scan.
          - --allow-rom-change is never logged, though §6 says it is.
          - Write-path Unsupported errors go through Failure::new(convert::core_error(..)) (handle.rs:964, 987, 995, 1008) with no gap entry, so an MMM01 --patch is never logged; §10 says every Unsupported is. Failure.gap is (&'static str, &'static str) rather than the typed Feature.
          - The per-event gaps map (handle.rs:799-803) holds the whole bundle's counts, but EventLogLine.gaps is documented as this session's.

          Seams: crates/protocol/src/session.rs, crates/bundle/src/log.rs and events(), the split handle modules.
        criteria:
          - "Tests: an event line carries the op's args; the log rotates past a size threshold (injectable) and events() still returns the newest"
          - "Test: alice writes, bob reads, carol's stale write is told alice"
          - "Tests: --allow-rom-change is logged; an Unsupported from a poke or patch lands in the gap ledger; per-event gap counts are this session's"
      - title: 'Protocol typing: enums for from/since, refuse unknown fields, typed gap and op'
        ref: protocol
        blockedBy: [eventlog]
        desc: |
          Why: the CLI leaf (LUax7P) is next, and it consumes these types. Fix the stringly typed seams now, or the CLI grows a twin parse of each, which the house rules forbid (one struct across a serialization seam).

          - RunArgs.from and since are free strings matched against "boot" and "power-on" in the server (handle.rs:327-337, 750-766). Make them protocol enums.
          - Unknown fields are accepted: Request uses #[serde(flatten)] and no args struct sets deny_unknown_fields. So "revv":"s3" silently skips the stale-rev guard, and "args":{"frame":10} silently runs the default budget. Refuse unknown fields with bad_request naming the field and the nearest known one. Check that flatten and deny_unknown_fields work together in serde; they often don't, and the fix may be to restructure rather than flatten.
          - Response has ok: bool beside the result and error Options, and ResultBody is a bag of Options, so invalid combinations are representable. Your call how far to go: at least make ok derived from which of the two is present, so it can't disagree.
          - BudgetLimits' doc says "null when the caller didn't say", but the fields are omitted. Make the doc and the bytes agree.

          Seams: crates/protocol, with the server call sites. Pinned response-byte tests may change; explain each change. Blocks the CLI leaf LUax7P.
        criteria:
          - from and since are enums in protocol, and the server matches them exhaustively
          - "Tests: a request with an unknown top-level or args field gets bad_request naming it"
          - Response cannot represent ok:true with an error (type or test)
      - title: 'Re-review the fixes: independent pass over the changed surfaces'
        ref: rereview
        blockedBy: [sbver, layers, cadence, untilsem, addr, split, errors, store, serverstate, serverrun, eventlog, protocol]
        desc: |
          Why: the original commits passed fmt, clippy and every test, yet carried the defects above. A green suite isn't evidence here, so close the stage with a review by someone who didn't write the fixes.

          What: a code review of the diff from 47957a9 to the last fix commit, by reviewers given the code, not the fix authors' conclusions. Check each leaf's criteria against the code and tests, not the reports. Re-run the --until oracle probes and the pre-patch state probe from scratch. The verdict goes as a note on the container; new defects are filed as issues.
        criteria:
          - Every leaf's criteria re-checked against code and tests by a reviewer who did not write them; the verdict noted on the container
          - The pre-patch state, the four --until misses, and undo walk-back re-verified by running them
```
