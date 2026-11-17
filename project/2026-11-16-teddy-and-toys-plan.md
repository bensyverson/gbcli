# Plan: Teddy & Toys, an example game from Ben's design

2026-11-16, twentieth session. Imported into `job` on 2026-11-16 under root `9b5RBA` as Stage 14 (`hmDFvT`; leaves BinRmI, CPtphq, zxwfFU, Zgr9Xh, XLUObc); after import the tracker is the plan, so change leaves with `job edit`, not the YAML below.

> **Renamed, 2026-11-16:** the game's folder is now `examples/teddy-and-toys/` (Ben's call), so its ROMs build as `build/teddy-and-toys.gb` and `build/gbc/teddy-and-toys.gbc`. `examples/factory/` and `build/factory.gb` below are the names at the time.

## Goal

A sixth example game, and the first whose design is Ben's rather than the agent's: a toy-factory arcade game built from Ben's sketch ([2026-11-16-teddy-and-toys-sketch.jpg](2026-11-16-teddy-and-toys-sketch.jpg)). It is built the way the other five were (an agent using gbcli as a user; [2026-11-08-example-games-plan.md](2026-11-08-example-games-plan.md) ▸ How the game agents work), with one difference that shapes everything: **Ben wants to iterate on the gameplay.** So the game is built to be tuned. Every gameplay number lives in one file, the toys are data, and a rules check reruns every rule in seconds after a change.

## The design (Ben, 2026-11-16)

**Title:** Teddy & Toys. **Folder:** `examples/factory/`.

**Elle, the worker (Ben, 2026-11-16).** The main character is Elle, an 8-year-old girl. She has long, wavy, honey-blonde hair with full bangs, and wears a light-blue denim sleeveless top with a row of silver buttons and a purple flower choker. Ben gave a reference photo; it lives untracked at `local/elle-reference.jpg` in the main checkout (absolute path `/Users/ben/git/gbcli/local/elle-reference.jpg`, since a worktree has no `local/`), and must never be committed or copied into the repo. The pencil sketch's dark hair is only the pencil; the description here wins.

**The screen (from the sketch).** A side view from behind Elle. There are two horizontal belts: one at waist height behind the worker, and one in the foreground at the bottom. An analog clock sits top left with a score box beside it. A narrow key column runs down the right edge, showing the six finished toys stacked one above another.

**The belts.** The front belt is the **parts belt** and the back belt is the **Finished belt**. They run in opposite directions at a constant speed; the speed does not ramp. Down turns the worker to face the parts belt, which means facing the camera, so we see their face. Up turns them to face the Finished belt, so we see their back.

**The toys**, each made of two halves, top to bottom in the key:
1. Teddy bear
2. Train (you attach the smokestack)
3. Squishy (you put it in its container)
4. Slinky (two halves)
5. Gag glasses (glasses and nose attach to a separate mustache)
6. Crayons and crayon box

**Controls**

| Facing | Holding | Input | Result |
|---|---|---|---|
| parts belt | a free hand | A, with a piece in front of you | Picks it up (if it matches the half you already hold) |
| parts belt | one half | B | It goes back on the belt where you stand and keeps moving |
| parts belt | two halves | B | The last half you grabbed goes back on the belt |
| either | two halves | B+A together | Assembles them into the finished toy |
| Finished belt | an assembled toy | B | Puts it on the Finished belt; score +1 |
| anything else | | A or B | Nothing, with a buzz |

- Either half can come first. A half that doesn't match the one you hold is refused with a buzz.
- Held pieces show in the worker's hands, one per hand. The finished toy replaces them only when the player assembles with B+A; halves never join on their own.
- B+A is a chord. When B is pressed while holding two halves, the game waits a short, tunable window (start near 5 frames) for A before it treats the press as a plain B. A pressed first, with B inside the window, also counts as the chord.
- A missed piece costs nothing. Parts arrive at random, but the supply is skewed so a matching half is never far away.

**The round.** One minute, shown on the analog clock. A single hand sweeps once per round, and the face turns red in the last 10 seconds on a Game Boy Color. The score box counts the toys made this round. There is no win or lose: when time runs out, a results screen shows this round's score and the personal best, with "New best!" when you beat it. The best score is kept in battery-backed save RAM. This is a deliberate departure from the other examples' win and lose states.

**Sound.** A bouncy, loopy factory tune, plus sound effects for grab, assemble, a toy scored, the buzz, and time running out. The effects must be clearly louder than the music: keep the music's volumes low and give the effects a channel the music gives up.

**What it shows about gbcli (the technique):** two belts scrolling in opposite directions from mid-frame scroll splits (LY=LYC interrupts in HBlank), and the clock's hand drawn from a small set of pre-made rotations swapped into tile memory. The key column has to stay still while the belt lines scroll under it, and the window layer is the natural way to do that; the agent proves whatever it picks.

## Built to be tuned (Ben, 2026-11-16)

Ben's words: "I'm eager to get the gameplay right, so build it in such a way that we can iterate easily. That may mean a builder script or constants we can dial."

- **One tuning file** (for example `src/tuning.inc`) holds every gameplay number, each with its unit and a one-line comment. These include belt speeds (sub-pixel), piece spacing, the pick-up zone's width, supply skew, the chord window, round length, the clock's red-zone time and the buzz cooldown. No gameplay number lives anywhere else.

  > **Correction, 2026-11-16:** `src/tuning.inc` was a bad example. rgbasm's INCLUDE searches only `include/`, so the file is `include/tuning.inc`. The help didn't make that clear, so it's also F-11 in [2026-11-16-examples-dx-review-factory.md](2026-11-16-examples-dx-review-factory.md).
- **The toys are data.** One file lists the six toys: names, halves, art file names and key order. A tool in `tools/` generates the include the game reads. Changing or adding a toy means editing data and rebuilding, not editing code.
- **A rules check.** A script in `tools/` drives gbcli through one scripted scenario per rule in the controls table: grab, mismatch refused, B returns a half to the belt, B with two halves returns the last one, the chord inside and outside its window, drop only when facing Finished with a whole toy, the round's end and the best-score save. It reports pass or fail per rule in seconds, so a tuning change is checked before anyone plays it.
- **A playtest checkpoint.** After the gameplay leaf, the chain stops and Ben plays the placeholder build before any art is made.
- **The README's "Tuning" section** tells a human what to edit for each knob, and the one command that rebuilds and rechecks.

## Playtest rulings (Ben, 2026-11-16)

From the placeholder build (commit `00e8efa`), with the layout as built: HUD lines 0-55, Finished belt 56-87, floor 88-111, parts belt 112-143, and the key column in the window at WX=143.

- **The layout stays.** The belts' size and placement are about right.
- **The floor is white.** It replaces the placeholder's dotted gray.
- **The belts are gray with lines across them,** so their motion reads as they scroll. On the DMG the wall takes light gray, so the belts most likely take dark gray with black lines. Pieces and toys must then read against dark gray, for example with black outlines and light fills. This is the art step's call, shown in screenshots.
- **The wall** (the HUD band behind the clock and score) is light gray on the DMG and pink on the GBC. The clock face and score box need to stand out from it, for example a white face; the art step's call.
- **Halves must look different.** Each half gets its own silhouette, and only the crayon box is a box. The squishy goes in a clear cylindrical jar. The other pairs are: teddy head with ears and teddy body with limbs; a long engine with wheels and a tall, narrow smokestack; glasses and nose and a wide, flat mustache; a bundle of pointed crayons and an open box.
- **The slinky's halves are one piece.** Any two slinky halves combine. In the toy data, a toy can say its two halves are the same piece. Then holding one slinky half wants another of the same, and the supply's skew follows that.
- **Bigger pieces, within their 16x16 cells first.** The placeholder drawings fill about 10 px of each cell. Draw every piece and toy to fill its 16x16 cell, with bold outlines. If the six toys still can't be told apart at 16x16, go to 24x24 (the belts are 32 px tall). That is the art step's call, and it must be shown in screenshots and argued in the report.
- **Elle steps when she turns, and the turn costs play time.** Facing the Finished belt, she stands a few pixels higher, toward it (about 8 px). Facing the parts belt, she stands lower, toward the front. A side-view frame plays between the two. The turn lasts `TURN_FRAMES`, a new tuning constant starting at 8. During the turn, A and B are ignored with no buzz. That last detail was the main thread's call; Ben ruled only that the turn costs time.
- **The results screen's text is bigger:** the score, the best score and "New best!" at about twice the current height.

## Art review rulings (Ben, 2026-11-16)

From the art build (commit `c209b47`). "Elle approves."

- **Slinky.** Each half is a short coil stack, about half the height of the art build's half. The finished slinky looks like the art build's half: one full, tall stack.
- **Gag glasses.** A reference photo is untracked at `/Users/ben/git/gbcli/local/gag-glasses-reference.jpg`; never commit it. It shows bushy black eyebrows over round black-framed glasses, a big pink nose, and a bushy black mustache directly under the nose. The mustache half is black, and much bigger than in the photo, so it reads as a Groucho-style mustache.
- **Crayon box.** The finished box holds more crayons packed tightly, so it no longer reads as a birthday cake.
- **Key column.** The icons are centered horizontally; today they have a margin on the left and none on the right.
- **What Elle holds, when she faces away.** She is drawn over it, as if it's in front of her. A held toy is hidden behind her body, and held halves may peek out at her sides. Assembly stays allowed facing either belt. If the DMG's X-based sprite priority makes the peeking wrong, hide held things entirely while she faces away.
- **Belt lines behind pieces.** Pieces on a belt show the belt's lines behind them, instead of a solid dark-gray cell. Pieces in Elle's hands keep a clear background.
- **A cheerful jingle** (a short rising run of notes) when a finished toy lands on the Finished belt, replacing the plain score effect.

## Play rulings (Ben, 2026-11-17)

From the finished build (commit `d526f64`), after Ben and Elle played it at home ("Elle is extremely happy"). Ben's six asks are the first line of each bullet. Where Ben left the detail open, the main thread decided it, and says so; each of those is easy to change.

- **Faster belts.** "A bit faster, not double; maybe 50%." Both belts go from 96 to 144 (1/256 px a frame). The supply's spacing stays as it is unless the faster belt makes matches too rare, which the agent checks with the bot and reports.
- **The best score shows on the title screen.** It is saved and shown on the results screen, but the title doesn't show it. This is a bug: fix it with a rules-check case that fails first.
- **START pauses.** START during a round pauses with a short sound and opens a small menu: Continue or Quit. Main thread's calls: the clock, the belts, Elle and the music all stop while paused (the music silent, then resumed where it was); Up and Down move the cursor; A or START picks; B or START again also continues. Continue resumes the round. Quit goes back to the title without a results screen and without saving the score as a best. START is ignored during a turn's frames only if pausing there would be hard; otherwise it pauses anytime in a round.
- **The music speeds up toward the end.** The belts don't. Main thread's call: the tempo steps up once, when the clock turns red (`CLOCK_RED_SECONDS`), by a tuning constant, and returns to normal for the next round and on the title.
- **Color on the Game Boy Color.** The toys are colored: in Elle's hands, in the key column and on the belts. The title screen gets more color too; today nearly everything is gray. The DMG build keeps its four shades and must read as well as before.
- **Elle steps left and right.** Left and Right move her along the belt by a small amount: about one to two toy widths each way from her home spot (main thread's call: 24 px each way, a tuning constant), not the belt's full length. A "shuffle" sound plays as she moves. She can step facing either belt, but not during a turn. The pick-up reach is measured from where she stands, and a toy set on the Finished belt lands where she stands.

## How the game agent works

As [2026-11-08-example-games-plan.md](2026-11-08-example-games-plan.md) ▸ How the game agents work, with the lessons of [2026-11-09-games-de-plan.md](2026-11-09-games-de-plan.md) and [2026-11-09-raycaster-plan.md](2026-11-09-raycaster-plan.md) ▸ What D and E taught the briefs:

- **Users only.** The agent may use `gbcli help`, `gbcli recipes`, `--help` on each verb and the repo's `README.md`. It never reads `crates/`, `docs/`, `scripts/`, other examples, or `project/`, with three exceptions: this plan, the sketch and its own DX doc, `project/2026-11-16-dx-factory.md`.
- **The binary** is pinned and built outside the checkout: `cargo build --release -p gbcli --target-dir /Users/ben/.cache/gbcli-examples/target`. Brief its absolute path only, with no `PATH` prefix and no shell variable holding it, and one plain command per call.
- **Build both targets every session.** That means `gbcli build` and `gbcli build --gbc`.
- **Art** goes in `.pixels` files (`gbcli help pixels`), or is drawn by Python standard-library scripts in `tools/`. Music is gbcli song text (`gbcli help music`). Write edit scripts to files, not inline `python3 -c`.
- **Record key files late**, after startup stops changing. Report the bot's call count and wall time.
- **The integrator** wires the ROM into `scripts/examples/build-roms.sh` and adds the row to `examples/README.md` at merge.
- **One agent, one worktree, the chain in order.** It pauses at the playtest checkpoint and is resumed (or replaced, past its context line) after Ben plays.

## Order

1. **Belts and clock first.** The scroll splits, the still key column and the clock hand are the technique and the screen's skeleton, and they are where the tuning file starts.
2. **The shift.** The full gameplay loop with placeholder art, the toy data, the rules check and the best-score save.
3. **Playtest** with Ben, then retune or reshape as he decides.
4. **Art, music and sound.**
5. **Polish, the GBC build, the README** with its Tuning section, and verification.

```yaml
tasks:
  - title: 'Stage 14: Teddy & Toys, an example game from Ben''s design'
    ref: stage14
    desc: |
      A sixth example game, the first whose design is Ben's: a toy-factory arcade game from his sketch, built by an agent using gbcli as a user, and built to be tuned because Ben wants to iterate on the gameplay. The design, Ben's rulings and how the game agent works are in project/2026-11-16-teddy-and-toys-plan.md; the sketch is project/2026-11-16-teddy-and-toys-sketch.jpg. Read both before claiming a leaf.
    children:
      - title: 'Factory 1: prove the opposite belts, the still key column and the clock hand in gbcli'
        ref: factory1
        desc: |
          Why: the scroll splits, the key column that must not scroll, and the clock hand are the game's technique and its screen's skeleton; if they don't hold, the layout changes, so they go first. Part of Teddy & Toys in examples/factory/ (project/2026-11-16-teddy-and-toys-plan.md, which holds Ben's full design).

          What: `gbcli new` in examples/factory, then the screen from Ben's sketch with placeholder art. That means the HUD band at the top, the back (Finished) belt band at waist height and the front (parts) belt band at the bottom, scrolling in opposite directions at sub-pixel speeds by LY=LYC splits in HBlank. The key column on the right stays still over every line (the window layer is the natural tool; the agent proves its choice). An analog clock face has one hand that steps through pre-made rotations over a round, swapped into tile memory outside mode 3. The tuning file starts here (src/tuning.inc or similar), holding belt speeds, directions, the round length and the clock's step count, one commented constant each.

          Already decided (Ben, 2026-11-16): the layout of the sketch; belts in opposite directions at constant speed; a single sweeping hand that turns red in the last 10 s on GBC; every gameplay number in one tuning file. Users only (gbcli help, recipes, --help, README.md, plus the plan doc, the sketch and its own DX doc project/2026-11-16-dx-factory.md; never crates/, docs/, scripts/, other examples or the rest of project/). Both builds from this session on. Writes to scroll registers stay in HBlank.

          Open, the agent's call: band heights, how many clock rotations (60 one-second steps is the natural choice), how the clock tiles are laid out.

          Risk: the window covers everything to its right and below WY, so the HUD and the key column share its constraints; say how the layout fits them.
        criteria:
          - raster evidence (gbcli run --raster --json, result.raster.drawn) shows each belt band's lines drawn with its own scroll and the key column's pixels unchanged across frames, by gbcli commands shown in the DX doc
          - shots a few seconds apart show both belts moved in opposite directions and the clock hand advanced, by gbcli commands shown
          - frame budget over a full round shows no lag frames, by a gbcli command shown
          - both builds succeed; the DX doc has this session's section
      - title: 'Factory 2: the shift, built to be tuned'
        ref: factory2
        blockedBy: [factory1]
        desc: |
          Why: turns the screen into the game Ben designed, in a shape he can iterate on. Part of Teddy & Toys (project/2026-11-16-teddy-and-toys-plan.md ▸ The design, and ▸ Built to be tuned).

          What: the full one-minute round with placeholder art. The worker faces the parts belt (Down) or the Finished belt (Up). Pieces ride the parts belt with a skewed random supply. A grabs a piece in front of you if your hand is free and it matches the half you hold. B returns a half to the belt, where it keeps moving; with two halves it returns the last one grabbed. B+A together assembles, as a chord with a tunable window. B facing Finished with a whole toy drops it onto that belt and scores. Everything else buzzes. Held pieces show in the worker's hands. The score box counts toys, the clock ends the round, and a results screen shows the score and the personal best with "New best!", the best kept in battery-backed save RAM. The six toys and their halves are a data file turned into an include by a tool in tools/. Every gameplay number is in the tuning file. A rules check in tools/ runs one scripted scenario per rule in the plan's controls table and prints pass or fail per rule.

          Already decided (Ben, 2026-11-16): the controls table in the plan exactly; no penalty for missed pieces; constant belt speed; no win or lose, just the score against a personal best. Users only, as factory1.

          Open, the agent's call: the pick-up zone's width, the supply's skew rule, pieces as sprites or streamed background tiles (mind 10 sprites per line where the worker overlaps the back belt), the chord window's starting value.

          Out of scope: final art and sound (factory3). Stop after this leaf: Ben playtests before art is made.
        criteria:
          - the rules check passes every rule in the controls table, and fails when a rule is broken on purpose (shown once in the DX doc)
          - a scripted bot plays a full round to the results screen with a nonzero score, and a second round shows the best score kept, also across a power cycle via gbcli sav or a restored save (commands in the DX doc)
          - every gameplay number is in the tuning file, and the toy include is generated from the toy data by a tool
          - no lag frames over the bot's round, by the frame budget
          - both builds succeed; the DX doc has this session's section
      - title: 'Playtest: Ben plays the placeholder build and rules on the gameplay'
        ref: playtest
        blockedBy: [factory2]
        labels: [decision]
        desc: |
          Why: Ben said "I'm eager to get the gameplay right"; art and sound wait until he has played it.

          What: the main thread sends Ben the placeholder ROM and the tuning knobs; Ben plays and decides changes: tuning values, rule changes, layout. Small tuning changes go straight into the tuning file; rule changes edit factory3 or get a leaf of their own before it. Record the rulings in the plan doc.
        criteria:
          - Ben's playtest rulings are recorded in project/2026-11-16-teddy-and-toys-plan.md and applied or filed as leaves
      - title: 'Factory 3: art, music and sound'
        ref: factory3
        blockedBy: [playtest]
        desc: |
          Why: a demo has to look and sound like a game. Part of Teddy & Toys (project/2026-11-16-teddy-and-toys-plan.md).

          What: every image as a .pixels file (gbcli help pixels) or drawn by a Python standard-library script in examples/factory/tools/. That covers Elle, the worker, from behind and from the front (see ▸ Elle, the worker), each toy's two halves and its finished form, the key icons, the belts, the clock face and hand rotations, the score box, the title screen and the results screen. Then a bouncy, loopy factory tune as gbcli song text, and sound effects for grab, assemble, score, buzz and time running out on a channel the music gives up, clearly louder than the music. Colors that work on the DMG and with build --gbc, including the clock's red zone.

          Already decided (Ben, 2026-11-16): the six toys as listed in the plan; effects louder than the music. Agents make all assets; it may look programmer-made, but it should be deliberate and readable at 160x144. Any playtest rulings recorded in the plan bind this leaf.
        criteria:
          - every image in assets/ is a .pixels file or is produced by a script in tools/ that regenerates it byte-identical
          - the music and effects are visible in gbcli's audio views, and an effect's volume is shown above the music's (commands in the DX doc)
          - the rules check still passes; both builds succeed; the DX doc has this session's section
      - title: 'Factory 4: polish, the GBC build, the Tuning README and verification'
        ref: factory4
        blockedBy: [factory3]
        desc: |
          Why: closes the slice so Ben can keep tuning it and anyone can rerun its claims. Part of Teddy & Toys (project/2026-11-16-teddy-and-toys-plan.md).

          What: title screen to round to results and back; `gbcli build` and `gbcli build --gbc` both clean. A README in examples/factory/ covering what the game is and how to play; what's technically notable; a Tuning section naming each knob, its file, and the one command that rebuilds and reruns the rules check; and the gbcli commands that prove each claim (the opposite belts, the still key column, no lag, the rules check, the saved best). A final DX section: the five costliest stumbles and the three things that helped most. The integrator adds the ROM and the examples/README.md row at merge.
        criteria:
          - both builds succeed with no warnings, or each warning explained in the README
          - every claim in the README is backed by a command shown there, rerun by the agent at the end
          - the README's Tuning section names every constant in the tuning file
          - the DX doc ends with the summary section
```
