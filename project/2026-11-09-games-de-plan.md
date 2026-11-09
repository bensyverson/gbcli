# Plan: the second wave of example games (D roguelike, E rhythm)

2026-11-09, fifteenth session. Details the placeholders `4Nn3us` (D) and `0nfXkK` (E) under Stage 11 (`7Dap6M`), in the shape of A and B. The goal, Ben's rulings and how game agents work are in [2026-11-08-example-games-plan.md](2026-11-08-example-games-plan.md); this doc adds only what the first wave taught. Imported into `job` on 2026-11-09: the first block under `4Nn3us`, the second (copied to its own file, since `job import` reads only a file's first `tasks:` block) under `0nfXkK`. After import the tracker is the plan.

## What A and B taught the briefs

From [2026-11-08-examples-dx-review.md](2026-11-08-examples-dx-review.md) ▸ What the brief got wrong, and the fourteenth session's handoff:

- **Build both targets from session one.** `gbcli build` and `gbcli build --gbc` can disagree, so every session's criteria name both, not just the last one.
- **Write edit scripts to files.** Inline `python3 -c` edits can be refused by the worktree guard, and stale code then still builds.
- **Art can be text.** `.pixels` images (`gbcli help pixels`) are read by `build` directly; a Python generator is still fine for art that is computed (gradients, scaled copies). woodcase is available for larger art, suggested in briefs only, never in gbcli's help (Ben, 2026-11-08).
- **The halfway rule for context** (plan ▸ How the game agents work) replaces the plain 40% line.
- **Closed-loop input bots** were what helped both agents most; `gbcli recipes` documents the pattern now.
- **The pinned binary** is rebuilt at dispatch: `cargo build --release -p gbcli --target-dir /Users/ben/.cache/gbcli-examples/target`. Never `target/release` (LoadHigh runs it).

## The games

**D: roguelike (`examples/rogue/`).** Floors generated in code at run time from a seed: rooms and corridors in a map larger than the screen, scrolled as the player moves, turn-based enemies, items, stairs down. What it flexes: generation and pathing in assembly, which GB Studio's events can't express, checked with memory reads (the map dumped from WRAM and checked by a tool), determinism (one seed, one dungeon), and the frame budget while a floor generates. A vertical slice is a short descent: a few generated floors and a final one with the goal.

**E: rhythm game (`examples/rhythm/`).** Notes fall in time with a hUGE song, and hits are judged against the music driver's own tick, not a separate timer, so the chart can't drift from the audio. What it flexes: timing locked to the audio, checked with gbcli's audio views (the audio log's note onsets against the chart) and a scripted run that presses on exact frames and scores every note perfect. One song, start to finish, with a results screen.

## Order

Each game is a chain of four leaves, the risky technique first; D and E touch disjoint folders and DX docs, so they run in parallel. Both are blocked on `SQhTqx` (the gbcli fixes from the first wave), so they run on the improved gbcli.

```yaml
tasks:
  - title: 'Rogue 1: prove seeded generation and the scrolled map in gbcli'
    ref: rogue1
    desc: |
      Why: generating a connected floor in assembly within a sane time, and scrolling a map larger than the screen, are the risky parts; if either fails, the game's shape changes, so they go first. Part of D, the roguelike in examples/rogue/ (project/2026-11-09-games-de-plan.md; the plan for all games is project/2026-11-08-example-games-plan.md).

      What: `gbcli new` in examples/rogue, then a floor generator seeded from a value the ROM keeps (rooms and corridors, every floor tile reachable from the start), a map bigger than 32x32 tiles or exactly the background's size (the agent's call) scrolled to follow a placeholder player who moves tile by tile, and stairs that make a new floor. The agent proves it with gbcli: the map read out of WRAM (`--read` or a memory view) and checked for connectivity by a tool in examples/rogue/tools/; the same seed giving the same map twice; and the frame budget over generation (generation may span frames with the LCD on, or blank the screen briefly; say which and show it).

      Already decided: users only (gbcli help, recipes, --help, README.md; never crates/, docs/, scripts/, project/ other than its own DX doc project/2026-11-09-dx-rogue.md, or other examples). Build with `gbcli build` and `gbcli build --gbc` from this session on. Write edit scripts to files, not inline `python3 -c`. Python 3 standard library only, for tools.

      Open, the agent's call: the generation algorithm, map size, how the seed is chosen (a frame counter at the title screen is usual). Name the game.

      Risk: generation time on a 1 MHz-class CPU; measure it with the profiler and say how long a floor takes.
    criteria:
      - a tool shows every floor tile reachable for several seeds, from the map read out of a gbcli run (command in the DX doc)
      - one seed gives byte-identical maps on two runs, by a gbcli command shown
      - the frame budget over a walk and a floor change, by a gbcli command shown, with generation time stated
      - both builds succeed; the DX doc has this session's section
  - title: 'Rogue 2: the descent'
    ref: rogue2
    blockedBy: [rogue1]
    desc: |
      Why: turns the generator into a game. Part of D (project/2026-11-09-games-de-plan.md).

      What: turn-based play: enemies that move when the player does and chase by a simple pathing rule, bump-to-attack combat, hit points, a few items (healing, a better weapon), stairs down through a few generated floors to a final floor with the goal, a win state and death. A status line (window layer or sprites) shows hit points and depth. Placeholder art is fine; rogue3 makes the real assets.

      Already decided: as rogue1.

      Open, the agent's call: the number of floors, enemy kinds, item set, fog of war or not.
    criteria:
      - a scripted input run (a bot is fine, see gbcli recipes) reaches the win state with a fixed seed, and another dies (commands in the DX doc)
      - no lag frames while enemies take their turns on a busy floor, by the frame budget
      - both builds succeed; the DX doc has this session's section
  - title: 'Rogue 3: art, music and sound'
    ref: rogue3
    blockedBy: [rogue2]
    desc: |
      Why: a demo has to look and sound like a game. Part of D (project/2026-11-09-games-de-plan.md).

      What: every image as a .pixels file (gbcli help pixels) or drawn by a Python standard-library script in examples/rogue/tools/: wall and floor tiles, the player, enemies, items, the status line, the title screen. woodcase is available for larger art. A title theme and a dungeon theme as gbcli song text (gbcli help music), and sound effects (step, hit, pick-up, stairs) on a channel the music gives up. Colors that work on the DMG and with build --gbc.

      Already decided: agents make all assets; it may look programmer-made, but it should be deliberate and readable at 160x144.
    criteria:
      - every image in assets/ is a .pixels file or is produced by a script in tools/ that regenerates it byte-identical
      - the music and effects are visible in gbcli's audio views (commands in the DX doc)
      - both builds succeed; the DX doc has this session's section
  - title: 'Rogue 4: polish, the GBC build, and verification'
    ref: rogue4
    blockedBy: [rogue3]
    desc: |
      Why: closes the slice so it can be shown and rerun by anyone. Part of D (project/2026-11-09-games-de-plan.md).

      What: title screen to descent to result and back; `gbcli build` and `gbcli build --gbc` both clean; a README in examples/rogue/ that says what the game is, how to play, what's technically notable, and the gbcli commands that prove each claim (connected floors, one seed one dungeon, no lag, a scripted win). Add the game to examples/README.md's table and a ROM beside the others (scripts/examples/build-roms.sh shows how the first two were made; the main thread wires it in at merge if the agent can't). A final DX section: the five costliest stumbles and the three things that helped most.
    criteria:
      - both builds succeed with no warnings, or each warning explained in the README
      - every claim in the README is backed by a command shown there, rerun by the agent at the end
      - the DX doc ends with the summary section
```

```yaml
tasks:
  - title: 'Rhythm 1: prove note timing locked to the music driver in gbcli'
    ref: rhythm1
    desc: |
      Why: judging a press against the music's own tick, with no drift over a whole song, is the risky part; if the driver's tick can't be read or hooked cleanly, the game's shape changes, so it goes first. Part of E, the rhythm game in examples/rhythm/ (project/2026-11-09-games-de-plan.md; the plan for all games is project/2026-11-08-example-games-plan.md).

      What: `gbcli new` in examples/rhythm, then a test song in gbcli song text playing through the hUGE driver, a chart (note times in driver ticks) kept beside it, notes scrolling toward a hit line in time with the chart, and a press judged by how many ticks or frames it is from its note (perfect, good, miss). The agent proves it with gbcli: the audio log's note onsets line up with the chart for the whole song (a tool in examples/rhythm/tools/ compares them), and a scripted run that presses exactly on each charted frame scores all perfect while one a few frames late scores good.

      Already decided: users only (gbcli help, recipes, --help, README.md; never crates/, docs/, scripts/, project/ other than its own DX doc project/2026-11-09-dx-rhythm.md, or other examples). Build with `gbcli build` and `gbcli build --gbc` from this session on. Write edit scripts to files, not inline `python3 -c`. Python 3 standard library only, for tools.

      Open, the agent's call: how the chart is authored (by hand, or generated from the song text by a tool), judgment windows, the lane count. Name the game.

      Risk: the driver's tick is not a whole number of frames, so the chart's frame times drift if they're rounded once; say how the game avoids it, and show the last note of the song still lines up.
    criteria:
      - a tool compares the audio log's note onsets with the chart over the whole song and finds no drift (command in the DX doc)
      - a scripted on-beat run scores every note perfect, and a late run scores good, by gbcli commands shown
      - the frame budget over the song shows no lag frames, by a gbcli command shown
      - both builds succeed; the DX doc has this session's section
  - title: 'Rhythm 2: the song'
    ref: rhythm2
    blockedBy: [rhythm1]
    desc: |
      Why: turns the timing engine into a game. Part of E (project/2026-11-09-games-de-plan.md).

      What: one full song with its chart, a score and combo, a life or meter that empties on misses, a results screen with a grade, and a fail state. Placeholder art is fine; rhythm3 makes the real assets and the final song.

      Already decided: as rhythm1.

      Open, the agent's call: scoring, how failing works, difficulty.
    criteria:
      - a scripted run clears the song with the top grade, and another fails it (commands in the DX doc)
      - no lag frames in the densest passage, by the frame budget
      - both builds succeed; the DX doc has this session's section
  - title: 'Rhythm 3: art, music and sound'
    ref: rhythm3
    blockedBy: [rhythm2]
    desc: |
      Why: a demo has to look and sound like a game, and in a rhythm game the music is the game. Part of E (project/2026-11-09-games-de-plan.md).

      What: the final song (written for the game, in gbcli song text) with its chart re-made to match, a title theme, and hit and miss sounds that don't fight the music for a channel at the wrong moment. Every image as a .pixels file (gbcli help pixels) or drawn by a Python standard-library script in examples/rhythm/tools/: notes, lanes, hit line, judgments, the title and results screens. woodcase is available for larger art. Colors that work on the DMG and with build --gbc.

      Already decided: agents make all assets; it may look programmer-made, but it should be deliberate and readable at 160x144.
    criteria:
      - every image in assets/ is a .pixels file or is produced by a script in tools/ that regenerates it byte-identical
      - the final chart still lines up with the final song by rhythm1's tool, and the effects are visible in gbcli's audio views (commands in the DX doc)
      - both builds succeed; the DX doc has this session's section
  - title: 'Rhythm 4: polish, the GBC build, and verification'
    ref: rhythm4
    blockedBy: [rhythm3]
    desc: |
      Why: closes the slice so it can be shown and rerun by anyone. Part of E (project/2026-11-09-games-de-plan.md).

      What: title screen to song to results and back; `gbcli build` and `gbcli build --gbc` both clean; a README in examples/rhythm/ that says what the game is, how to play, what's technically notable, and the gbcli commands that prove each claim (no drift, a perfect scripted run, no lag). Add the game to examples/README.md's table and a ROM beside the others (scripts/examples/build-roms.sh shows how the first two were made; the main thread wires it in at merge if the agent can't). A final DX section: the five costliest stumbles and the three things that helped most.
    criteria:
      - both builds succeed with no warnings, or each warning explained in the README
      - every claim in the README is backed by a command shown there, rerun by the agent at the end
      - the DX doc ends with the summary section
```
