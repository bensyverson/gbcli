# Plan: the last example game (C, first-person raycaster)

2026-11-09, sixteenth session. **Status: draft, Ben's two questions answered. Not imported, and not dispatched until the D/E DX review (`f57YRV`) is back and the fixes Ben accepts from it have landed** (Ben, 2026-11-09). This doc details the placeholder `uI1JDB` under Stage 11 (`7Dap6M`), in the shape of [2026-11-09-games-de-plan.md](2026-11-09-games-de-plan.md). The goal, Ben's rulings and how game agents work are in [2026-11-08-example-games-plan.md](2026-11-08-example-games-plan.md). This doc adds only the game and what D and E taught.

## What D and E taught the briefs

From the fifteenth session's handoff ▸ "What the D and E agents said the briefs got wrong". The D/E DX review adds to this list before import.

- **Don't point the agent at `scripts/examples/build-roms.sh`.** The user contract forbids `scripts/`. The integrator wires the ROM in at merge.
- **Give the binary's absolute path only.** The worktree guard refuses a `PATH=…:$PATH` prefix, and it refuses a shell variable that holds the binary.
- **Drop `gbcli gaps` from the DX doc's required content.** It lists only what the emulator core couldn't do, and it printed `none` in every D and E session. Stumbles belong in the DX doc.
- **The halfway context rule stays, but expect it not to bite.** Each four-leaf chain fit in about 35% of a 1M window. C has five leaves.
- **From the D/E DX review** ([2026-11-09-examples-dx-review-de.md](2026-11-09-examples-dx-review-de.md) ▸ What the brief got wrong):
  - Name only verbs that exist at the pin. D's leaf promised a memory view that didn't exist. `view memory` lands with the fixes C waits for.
  - Record key files late, after startup stops changing, or replay them with `--keys-since`.
  - Ask the agent to report its bot's call count and wall time. C is the test for the embedded-scripting backlog entry.
  - State in the brief when a frame ends and when a key lands, unless `help time` already says it at the pin.

## The game

**C: first-person raycaster (`examples/raycaster/`).** A first-person maze drawn by casting one ray per screen column each frame and drawing the walls into tile memory. The view is a block of unique tiles that the program rewrites, not a fixed tileset.

What it flexes:
- **The heaviest CPU work of the five.** The math is fixed-point stepping across grid lines (DDA), a distance-to-height table and column fills, done on a 1 MHz-class CPU.
- **Getting pixels into VRAM in time.** It can only be written outside mode 3, so the frame is drawn into a WRAM buffer and copied in blanking or over several frames.
- **Profile-driven optimization**, the showcase. Each change is measured before and after with `--profile`, `measure` and `--frame-budget`, and checked as unchanged with `gbcli equiv` (`gbcli help optimize`).

A vertical slice is one hand-made maze with an exit, something to find or avoid, a title screen, music, and a win and a lose state.

**Verification that only a raycaster offers:** a host reference renderer. That's a Python tool in `examples/raycaster/tools/` that uses the ROM's own fixed-point math and tables, renders the same view from the same map, position and angle, and compares it with `gbcli shot` pixel for pixel. That makes "the walls are right" a checkable claim, and it keeps the optimization pass honest beyond `equiv`.

## Order

Five leaves, one chain, one agent as far as its context line allows. The renderer comes first because it's the risk. Optimization gets its own leaf, before gameplay, because it's the point of the game and gameplay code competes for the same frame.

```yaml
tasks:
  - title: 'Raycaster 1: prove a raycast view drawn into tile memory in gbcli'
    ref: ray1
    desc: |
      Why: drawing a first-person view into VRAM fast enough to move through is the risky part; if the frame rate or the VRAM copy doesn't work, the game's shape changes (view size, frame rate, detail), so it goes first. Part of C, the raycaster in examples/raycaster/ (project/2026-11-09-raycaster-plan.md; the plan for all games is project/2026-11-08-example-games-plan.md).

      What: `gbcli new` in examples/raycaster, then a fixed test map, a player position and angle, one ray per column with fixed-point DDA, wall heights from a distance table, walls drawn into a WRAM buffer laid out as the view's tiles, and the buffer copied into VRAM without writing in mode 3 (in VBlank, split over frames, or with the LCD briefly off; the agent's call, stated and shown). Turning and moving with the d-pad. A host reference renderer in examples/raycaster/tools/ with the ROM's own math and tables, compared with `gbcli shot` for several positions.

      Already decided: users only (gbcli help, recipes, --help, README.md; never crates/, docs/, scripts/, project/ other than its own DX doc project/2026-11-09-dx-raycaster.md, or other examples). Build with `gbcli build` and `gbcli build --gbc` from this session on. The --gbc build may switch to double speed, but the DMG build must reach its own target without it (Ben, 2026-11-09). Write edit scripts to files, not inline `python3 -c`. Python 3 standard library only, for tools.

      Open, the agent's call: view size (a smaller window than 160x144 is normal), columns per ray (1 or 2 pixels), flat or textured walls, the copy strategy. Name the game.

      Risk: frame rate. Measure frames per view update with gbcli and state it; a first cut of a few updates a second is fine, ray2 optimizes.
    criteria:
      - the reference tool matches gbcli shot pixel for pixel at several positions (command in the DX doc)
      - no VRAM write lands in mode 3, shown by a gbcli command (gbcli help traps names one)
      - view updates per second stated, from the frame budget or measure, by a gbcli command shown
      - both builds succeed; the DX doc has this session's section
  - title: 'Raycaster 2: profile-driven optimization'
    ref: ray2
    blockedBy: [ray1]
    desc: |
      Why: this is the game's showcase: each speedup found by the profiler, measured before and after, and proven not to change the picture. Part of C (project/2026-11-09-raycaster-plan.md).

      What: a fixed scripted walk (a keys file) and a baseline; then rounds of profile, change, measure, following `gbcli help optimize`: each round names the hot spot from --profile, the change, the cycles before and after by `gbcli measure` on a named span, and `gbcli equiv` against the baseline (or the reference tool, where a change is meant to alter pixels, such as a precision change). A table of the rounds goes in the DX doc. A `measure --max` assertion holds the final figure.

      Already decided: as ray1. The agent stops when the view update hits its target or a round gains under 5%, and says which.

      Open, the agent's call: the target update rate, which techniques to try (unrolled column fills, lookup tables in place of multiplies, fewer rays with doubled columns, partial-buffer copies).
    criteria:
      - at least three measured rounds, each with the profile line, the change, cycles before and after, and an equiv or reference check (commands in the DX doc)
      - a measure --max assertion holds the final span, and the frame budget over the scripted walk is shown
      - both builds succeed; the DX doc has this session's section
  - title: 'Raycaster 3: the maze'
    ref: ray3
    blockedBy: [ray2]
    desc: |
      Why: turns the renderer into a game. Part of C (project/2026-11-09-raycaster-plan.md).

      What: one hand-made maze with collision, an exit, and a goal that makes it a game (keys and a locked exit, a timer, a pursuer drawn as a scaled sprite, or the agent's own idea), a win and a lose state, and a status line. Placeholder art is fine; ray4 makes the real assets. Gameplay must not undo ray2: re-run its measure assertion.

      Already decided: as ray1.

      Open, the agent's call: the goal, a minimap or not, sprites in the view or not.
    criteria:
      - a scripted input run (a bot is fine, see gbcli recipes) reaches the win state, and another reaches the lose state (commands in the DX doc)
      - ray2's measure assertion still passes, and the frame budget over the winning run is shown
      - both builds succeed; the DX doc has this session's section
  - title: 'Raycaster 4: art, music and sound'
    ref: ray4
    blockedBy: [ray3]
    desc: |
      Why: a demo has to look and sound like a game. Part of C (project/2026-11-09-raycaster-plan.md).

      What: every image as a .pixels file (gbcli help pixels) or drawn by a Python standard-library script in examples/raycaster/tools/: wall shades or textures, the status line, any sprites, the title screen. woodcase is available for larger art. A title theme and a maze theme as gbcli song text (gbcli help music), and sound effects on a channel the music gives up. Colors that work on the DMG and with build --gbc. Keep the reference tool matching if the wall art changes.

      Already decided: agents make all assets; it may look programmer-made, but it should be deliberate and readable at 160x144.
    criteria:
      - every image in assets/ is a .pixels file or is produced by a script in tools/ that regenerates it byte-identical
      - the music and effects are visible in gbcli's audio views, and the reference tool still matches (commands in the DX doc)
      - both builds succeed; the DX doc has this session's section
  - title: 'Raycaster 5: polish, the GBC build, and verification'
    ref: ray5
    blockedBy: [ray4]
    desc: |
      Why: closes the slice so it can be shown and rerun by anyone. Part of C (project/2026-11-09-raycaster-plan.md).

      What: title screen to maze to result and back; `gbcli build` and `gbcli build --gbc` both clean; a README in examples/raycaster/ that says what the game is, how to play, what's technically notable (the optimization table belongs here), and the gbcli commands that prove each claim. Add the game to examples/README.md's table; the main thread wires its ROM in at merge. A final DX section: the five costliest stumbles and the three things that helped most.
    criteria:
      - both builds succeed with no warnings, or each warning explained in the README
      - every claim in the README is backed by a command shown there, rerun by the agent at the end
      - the DX doc ends with the summary section
```

## Questions for Ben

1. **GBC double speed.** May the `--gbc` build switch to double speed (a GBC-only CPU mode) for a faster view, as long as the DMG build reaches its own target alone? *Recommend yes.* It exercises gbcli's speed-switch handling (`measure` refuses a span across a switch), and DMG fidelity still comes first.
   - **Ben, 2026-11-09: yes.** ray1's "Already decided" line says so, and the later leaves inherit it ("as ray1").
2. **Fixes first?** Should C wait for the gbcli fixes the D/E review leads to, as D and E waited for `SQhTqx`? *Recommend yes*, for whichever fixes Ben accepts, so C's DX doc reports on the improved tool.
   - **Ben, 2026-11-09: yes.** At import, ray1 is blocked on the container for the accepted fixes.
