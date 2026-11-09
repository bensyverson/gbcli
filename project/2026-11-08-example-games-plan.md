# Plan: example games built entirely by agents

2026-11-08, fourteenth session. Imported into `job` on 2026-11-08 under root `9b5RBA` as Stage 11; after import the tracker is the plan, so change leaves with `job edit`, not the YAML below.

## Goal

Prove gbcli out by having agents build real Game Boy games with it, start to finish: code, art, music and verification. Each game should push past what GB Studio can do, in genre or technique, so the demos show what an assembly toolchain driven by agents adds. The agents' developer-experience (DX) feedback is the second product: after the first two games, Ben and the main thread read it and decide what to change in gbcli.

## Rulings (Ben, 2026-11-08)

- **Five games, in three waves.** First the pseudo-3D road racer (A) and the bullet-heavy shooter (B). In a later session, the roguelike (D) and the rhythm game (E). Last, the first-person raycaster (C).
- **Game agents are users, not gbcli developers.** They use gbcli as shipped: the `gbcli` binary, `gbcli help`, `gbcli recipes`, `--help` on each verb, and the repo's `README.md`. They never read gbcli's source (`crates/`), its `docs/`, `scripts/`, `project/` (except their own DX doc), or another example. When gbcli is in their way, they record it (`gbcli gaps`, their DX doc, their report) rather than patch it. Separate agents fix gbcli.
- **Agents make every asset.** Pixel art is drawn by scripts the agent writes (the PNGs and the scripts are both committed); music and sound effects are composed as gbcli song text (`gbcli help music`). It will look programmer-made, and that's accepted.
- **Each game is a vertical slice:** one polished level or track, a title screen, the gameplay loop, music, and a win and a lose state. DMG first, and also built with `gbcli build --gbc` so it shows in color on a GBC. About 3-5 agent sessions each.
- **DX feedback goes in `project/`, blind.** Each game has one dated DX doc, appended to by every session that works on it. No agent reads another game's DX doc.
- **After A and B, stop and review the feedback** (leaf `review-ab`) before starting D and E.

## Originality

Every game is an original design: its own name, characters, art, music and levels. A genre or a technique is fair game; a recreation of a specific existing game is not, renamed or otherwise. The concept lines below are starting points, and the game agent names its game.

## The games

**A: pseudo-3D road racer (`examples/racer/`).** A road that curves and climbs, drawn by changing the scroll registers on every scanline from an LCD STAT (or LY=LYC) interrupt during HBlank, with roadside objects and rival cars drawn as sprites at a few pre-scaled sizes. What it flexes: scanline-timed interrupt code, which GB Studio's event scripting can't express, checked with gbcli's raster and layer observations, the frame budget and the profiler. One track, with a lap timer or a race against rivals.

**B: bullet-heavy shooter (`examples/shooter/`).** 30 or more bullets and enemies at once under the hardware's limits (40 sprites, 10 per line), by a sprite multiplexer that rotates OAM priority so excess sprites flicker rather than vanish. Parallax sky from mid-frame scroll splits, and hUGE music with sound effects on a channel the music gives up. What it flexes: sprite pressure GB Studio can't sustain (it slows down past a dozen actors), checked with OAM views and the frame budget, plus the audio views. One stage ending in a boss.

**Later waves, kept open until `review-ab`:** D, a roguelike with generated dungeons (generation in code, which GB Studio's events can't do); E, a rhythm game locked to the music's timing (the audio views and timing checks); C, a first-person raycaster drawing walls into tile memory each frame (profile-driven optimization). Their leaves are placeholders, detailed after the review.

## How the game agents work

- **The binary.** The main thread builds one gbcli at the commit of dispatch, outside the checkout (LoadHigh runs `target/release/gbcli`, which must not be swapped under it), and names its absolute path and commit in each brief: `cargo build --release -p gbcli --target-dir /Users/ben/.cache/gbcli-examples/target`. RGBDS comes from `PATH`.
- **The project.** `gbcli new` makes the project under `examples/<game>/`. Before the first run, the agent makes `examples/<game>/.gbcli/`: gbcli finds sessions by walking up from the working directory, and without one there, sessions land in the repo's own `.gbcli/` (a trap of nesting a project in gbcli's repo, not one a user meets). `.gbcli/` and `build/` stay uncommitted.

  > **Correction, 2026-11-09:** making `.gbcli/` by hand is no longer needed: `gbcli new` makes it since leaf nqXbq4 (`created path=…/.gbcli/`). The raycaster's DX review caught the stale line ([2026-11-09-examples-dx-review-c.md](2026-11-09-examples-dx-review-c.md), What the brief got wrong).

- **A game agent's context line (Ben, 2026-11-08):** past 40%, it keeps going if it has finished at least half its leaves, finishing the leaf in hand before it stops; short of half, it stops after the leaf in hand and a fresh agent takes the rest.
- **One worktree per game; each game's leaves in order, by one agent as far as its context line allows.** A and B touch disjoint folders and DX docs, so they run in parallel; each game's leaves are a chain. The main thread integrates each session by squash-merging its branch, as usual.
- **Tools beyond gbcli.** Python 3 standard library only, for art scripts (write PNGs with `zlib` and `struct`) and any other generator, kept in `examples/<game>/tools/`. No new dependencies.
- **Verification is the agent's own.** Each claim in a game's README (it runs at 60 fps, the raster split lands on line N, a scripted run reaches the win screen) is backed by a gbcli command the README shows, so a reader can rerun it.
- **The DX doc.** `project/2026-11-08-dx-racer.md` and `project/2026-11-08-dx-shooter.md`. Each session appends a dated section: what it tried, where gbcli helped, where it stumbled (the exact command and output, what the agent expected, what it did instead), anything missing, and `gbcli gaps` output. Rank the stumbles by cost. Be specific and blunt; praise only what saved real time.
- **When gbcli blocks the game,** the agent works around it if it reasonably can, records the stumble either way, and says so in its report. A blocker with no workaround stops that leaf; the main thread decides whether to fix gbcli then or wait for `review-ab`.

## Order and why

1. `setup`: the `examples/` folder and its README, done on the main thread. It's small, and every game leaf assumes it.
2. A and B in parallel, each a chain: the risky technique first, as a spike that proves it in gbcli (so a dead end shows up in session one), then the game loop, then assets and audio, then polish and verification.
3. `review-ab`: Ben and the main thread read both DX docs and decide on gbcli changes.
4. D and E, then C, detailed after the review.

```yaml
tasks:
  - title: 'Stage 11: Example games built by agents'
    ref: stage11
    desc: |
      Agents build original Game Boy games with gbcli, as users, to prove the system out and to gather developer-experience feedback. The plan, Ben's rulings and how the game agents work are in project/2026-11-08-example-games-plan.md; read it before claiming a leaf.
    children:
      - title: 'Set up examples/ and the pinned gbcli binary for game agents'
        ref: setup
        desc: |
          Why: every game leaf assumes a place to put its project and a gbcli binary that won't change under it.

          What: examples/README.md (one paragraph on what the examples are and that agents built them, a table of games with one line each, and how to build and run one with gbcli). A release gbcli built outside the checkout (cargo build --release -p gbcli --target-dir /Users/ben/.cache/gbcli-examples/target), with its commit recorded in the brief. Root .gitignore already ignores .gbcli/ at any depth; confirm each project's build/ is ignored by the starter's own .gitignore.

          Out of scope: the games themselves.
        criteria:
          - examples/README.md exists and is linked from the root README
          - the pinned binary runs `gbcli version` from its path outside the checkout
      - title: 'A: road racer'
        ref: racer
        desc: |
          The pseudo-3D road racer in examples/racer/: one track, curves and hills by per-scanline scroll, pre-scaled roadside and rival sprites. See the plan ▸ The games, and ▸ How the game agents work. Each leaf is one agent session in the same worktree chain; each appends to project/2026-11-08-dx-racer.md.
        children:
          - title: 'Racer 1: prove the scanline road in gbcli'
            ref: racer1
            blockedBy: [setup]
            desc: |
              Why: the per-scanline road is the risky technique; if it can't hold timing on the DMG, the game's shape changes, so it goes first.

              What: `gbcli new` in examples/racer (make examples/racer/.gbcli/ first, see the plan), then a road drawn by an LCD STAT or LY=LYC interrupt that sets SCX (curves) and SCY (hills) each line in HBlank from a per-line table, with the horizon split from a static sky. Steering and speed move the table live from the joypad. The agent proves it with gbcli: raster and layer observations show each write lands in HBlank on the intended line, and the frame budget shows no lag frames at full speed.

              Already decided: users only (gbcli help, recipes, --help, README.md; never crates/, docs/, scripts/, project/ other than its own DX doc, or examples/shooter). Writes stay in HBlank: mid-mode-3 writes are both risky on hardware and the one family SameBoy is known to get wrong.

              Open, the agent's call: road geometry, how many lines are computed per frame vs. tabled, how hills are faked. Name the game.

              Risk: the interrupt's cost per line on top of game logic; measure it with the profiler and say what's left for the game.
            criteria:
              - a scripted run shows the road curving and climbing (screenshots or a short frame sequence, with the commands in the DX doc)
              - raster evidence that every scroll write lands in HBlank on its line, by a gbcli command shown
              - frame budget over a steering run shows no lag frames, by a gbcli command shown
              - the DX doc has this session's section
          - title: 'Racer 2: the race'
            ref: racer2
            blockedBy: [racer1]
            desc: |
              Why: turns the road into a game.

              What: the player's car (sprites), track data with curves and hills over one lap, off-road slowdown, roadside objects and rival cars at pre-scaled sizes chosen by distance, collisions, a lap timer or race position, and win and lose states. Sprite use stays within 10 per line; the agent checks it with gbcli's OAM views.

              Already decided: as racer1. Placeholder art is fine here; racer3 makes the real assets.

              Open, the agent's call: race against the clock or against rivals; how sprites are scaled.
            criteria:
              - a scripted input run completes a lap and reaches the win state, and another reaches the lose state (commands in the DX doc)
              - no lag frames in a busy stretch of the lap, by the frame budget
              - the DX doc has this session's section
          - title: 'Racer 3: art, music and sound'
            ref: racer3
            blockedBy: [racer2]
            desc: |
              Why: a demo has to look and sound like a game.

              What: every image drawn by a Python standard-library script in examples/racer/tools/ that writes the PNGs into assets/ (gbcli help images), so the art can be regenerated and tweaked: car, rivals at each size, roadside objects, sky, title screen. A title theme and a race theme as gbcli song text (gbcli help music), and sound effects (engine, crash, lap) on a channel the music gives up. Colors that work on the DMG and with build --gbc.

              Already decided: agents make all assets; it may look programmer-made, but it should be deliberate and readable at 160x144.
            criteria:
              - every PNG in assets/ is produced by a script in tools/ and regenerates byte-identical
              - the music and effects are visible in gbcli's audio views (piano roll or audio log, commands in the DX doc)
              - the DX doc has this session's section
          - title: 'Racer 4: polish, the GBC build, and verification'
            ref: racer4
            blockedBy: [racer3]
            desc: |
              Why: closes the slice so it can be shown and rerun by anyone.

              What: title screen to race to result and back; `gbcli build` and `gbcli build --gbc` both clean; a README in examples/racer/ that says what the game is, how to play, what's technically notable, and the gbcli commands that prove each claim (60 fps, HBlank writes, a scripted win). A final DX section summarizing the whole game's experience: the five costliest stumbles and the three things that helped most.
            criteria:
              - both builds succeed with no warnings, or each warning explained in the README
              - every claim in the README is backed by a command shown there, rerun by the agent at the end
              - the DX doc ends with the summary section
      - title: 'B: shooter'
        ref: shooter
        desc: |
          The bullet-heavy shooter in examples/shooter/: one stage and a boss, a sprite multiplexer for 30+ objects, parallax by mid-frame scroll splits, music with sound effects. See the plan ▸ The games, and ▸ How the game agents work. Each leaf is one agent session in the same worktree chain; each appends to project/2026-11-08-dx-shooter.md.
        children:
          - title: 'Shooter 1: prove the sprite multiplexer and parallax in gbcli'
            ref: shooter1
            blockedBy: [setup]
            desc: |
              Why: more objects than the hardware shows at once is the risky technique, so it goes first.

              What: `gbcli new` in examples/shooter (make examples/shooter/.gbcli/ first, see the plan), then a test scene with 30+ moving objects (bullets as 8x8 or 8x16 sprites) driven by a shadow-OAM manager that rotates which sprites get priority each frame, so excess on a line flickers rather than vanishes, plus a parallax sky from two or three scroll splits by LY=LYC interrupt in HBlank. The agent proves it with gbcli: OAM views over consecutive frames show the rotation, raster observations show the splits on their lines, and the frame budget shows no lag frames.

              Already decided: users only (gbcli help, recipes, --help, README.md; never crates/, docs/, scripts/, project/ other than its own DX doc, or examples/racer). Writes stay in HBlank.

              Open, the agent's call: object pool layout, update cadence, how much flicker is acceptable. Name the game.
            criteria:
              - OAM evidence over several frames that every object is drawn in some frame, by a gbcli command shown
              - raster evidence of the parallax splits on their lines, by a gbcli command shown
              - frame budget with 30+ objects shows no lag frames, by a gbcli command shown
              - the DX doc has this session's section
          - title: 'Shooter 2: the stage'
            ref: shooter2
            blockedBy: [shooter1]
            desc: |
              Why: turns the engine into a game.

              What: the player ship, shots, enemy waves with bullet patterns over one scrolling stage, collisions, score, lives, a boss at the end, and win and lose states. Placeholder art is fine; shooter3 makes the real assets.

              Already decided: as shooter1.

              Open, the agent's call: the stage's length and pacing, enemy and boss patterns, the scoring.
            criteria:
              - a scripted input run beats the boss and reaches the win state, and another reaches game over (commands in the DX doc)
              - no lag frames in the stage's busiest stretch, by the frame budget
              - the DX doc has this session's section
          - title: 'Shooter 3: art, music and sound'
            ref: shooter3
            blockedBy: [shooter2]
            desc: |
              Why: a demo has to look and sound like a game.

              What: every image drawn by a Python standard-library script in examples/shooter/tools/ that writes the PNGs into assets/ (gbcli help images): ship, enemies, boss, bullets, parallax layers, title screen. A title theme and a stage theme as gbcli song text (gbcli help music), and sound effects (shot, hit, explosion) on a channel the music gives up. Colors that work on the DMG and with build --gbc.

              Already decided: agents make all assets; it may look programmer-made, but it should be deliberate and readable at 160x144.
            criteria:
              - every PNG in assets/ is produced by a script in tools/ and regenerates byte-identical
              - the music and effects are visible in gbcli's audio views (commands in the DX doc)
              - the DX doc has this session's section
          - title: 'Shooter 4: polish, the GBC build, and verification'
            ref: shooter4
            blockedBy: [shooter3]
            desc: |
              Why: closes the slice so it can be shown and rerun by anyone.

              What: title screen to stage to result and back; `gbcli build` and `gbcli build --gbc` both clean; a README in examples/shooter/ that says what the game is, how to play, what's technically notable, and the gbcli commands that prove each claim. A final DX section: the five costliest stumbles and the three things that helped most.
            criteria:
              - both builds succeed with no warnings, or each warning explained in the README
              - every claim in the README is backed by a command shown there, rerun by the agent at the end
              - the DX doc ends with the summary section
      - title: 'Decide: gbcli changes from the racer and shooter DX feedback'
        ref: review-ab
        blockedBy: [racer4, shooter4]
        labels: [decision]
        desc: |
          Why: Ben's ruling: after A and B, read the feedback and decide what to change before the next wave.

          What: the main thread reads project/2026-11-08-dx-racer.md and project/2026-11-08-dx-shooter.md and both `gbcli gaps` ledgers, groups the stumbles by cause (docs, errors, missing verbs, core limits), and brings Ben a ranked list with a recommendation for each; Ben decides; the accepted changes become leaves, and the rest go to project/backlog.md. Also detail the D, E and C leaves from what A and B taught.
        criteria:
          - a dated findings doc ranks the stumbles with a recommendation each, and records Ben's decisions
          - accepted changes are leaves; declined ones are in backlog.md
      - title: 'D: roguelike with generated dungeons (to detail after review-ab)'
        ref: rogue
        blockedBy: [review-ab]
        desc: |
          Placeholder. A roguelike with dungeons generated in code at run time, a genre GB Studio's events can't express. Detail it into session leaves after review-ab, in the shape of A and B. Ben's ruling: D and E come in the wave after A and B.
      - title: 'E: rhythm game synced to the music (to detail after review-ab)'
        ref: rhythm
        blockedBy: [review-ab]
        desc: |
          Placeholder. A rhythm game whose timing is locked to the hUGE driver's ticks, checked with gbcli's audio views and timing. Detail it into session leaves after review-ab. Ben's ruling: D and E come in the wave after A and B.
      - title: 'C: first-person raycaster (to detail after D and E)'
        ref: raycaster
        blockedBy: [rogue, rhythm]
        desc: |
          Placeholder. A first-person maze drawn by raycasting walls into tile memory each frame: the heaviest CPU work of the five, and the showcase for profile-driven optimization. Ben's ruling: it comes last. Detail it after D and E.
```
