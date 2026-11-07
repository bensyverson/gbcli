# Asset pipeline: images now, music after research

2026-11-07. The user asked whether gbcli should help with assets (images and music), pointing at GB Studio and its hUGETracker support. The answer is yes, with this framing: an agent can't drive a tracker or a sprite editor, so gbcli's value is **verification** — a fast, structured answer to "does this asset fit the hardware, and what will it look like or sound like" — not a GUI replacement. That is the same loop gbcli already gives for code.

> **Correction, 2026-11-07 (leaf O5swhd): several premises below were wrong.** DMG has no per-tile palettes (one BGP for the whole background); 384 tiles is the background plus sprite pool, not a background limit; rgbgfx has no 8x16 mode; `.uge` is a raw Pascal memory dump, tractable but not designed. The research is [2026-11-07-asset-pipeline-research.md](2026-11-07-asset-pipeline-research.md); leaf WdnjHI's description was rewritten from it, so the `tasks:` block below is stale.

## What exists already

- **Images:** RGBDS ships `rgbgfx` (PNG → 2bpp tiles, tile deduplication including flips, tilemaps, CGB palettes). Automatic tiling is mostly solved; what's missing is running it by convention and explaining hardware-limit failures with remedies.
- **Music:** hUGEDriver is the de facto player (hUGETracker, GB Studio). Its `.uge` song files are a binary but simple format; the user expects them to be tractable for agents, or convertible to an ASCII tracker format an agent can read and write. gbcli's audio views (write log, piano roll, per-channel WAV, spectrogram) already let an agent check playback.
- **GB Studio** (MIT) encodes years of practical limits in its engine and asset pipeline: background tile budgets, actor and sprite limits, bank layout, its asset warnings.

## Order and why

1. **Research first** (`assets-research`): the survey decides the shape of both later pieces, and costs one agent session.
2. **Image conversion and limit checks** (`assets-images`) in stage 6, after `build` lands (it extends `build`'s convention). Cheap, and the agent eval will show quickly whether agents need it.
3. **Music authoring** is parked in [backlog.md](backlog.md) until the research shows a tractable `.uge` ↔ text path, the agent eval shows a need, or a user asks.

Imported into `job` on 2026-11-07 under stage 6 (`LmBWdD`); after import the tracker is the plan.

```yaml
tasks:
  - title: "Research: GB Studio's asset pipeline, rgbgfx and hUGEDriver/.uge for agent-driven assets"
    ref: assets-research
    desc: |
      Why: the user wants gbcli to help agents author images and music for Game Boy games, learning from GB Studio. The image leaf (next) and a parked music leaf both depend on what this finds. Context and framing: project/2026-11-07-asset-pipeline-plan.md — verification over GUI replacement.

      Questions, each answered with sources (file paths at a pinned commit, docs URLs):
      - GB Studio (MIT): which hardware and engine limits does its asset pipeline check (unique background tiles, palettes per tile/scene on DMG and CGB, sprite and actor limits, 8x16 pairing, VRAM and bank budgets), what do its warnings and errors say, and which of those are true hardware limits vs GB Studio engine choices?
      - rgbgfx (RGBDS 1.0.4, the pinned version): which flags give dedup, flips, tilemaps, attribute maps and palettes; what does it report when an image exceeds a limit; what is left for gbcli to check or explain?
      - hUGEDriver and .uge: licenses (driver and tracker) and whether the driver can be vendored into `new`'s template; the .uge format (versions, structure) and how hard a lossless .uge <-> ASCII text conversion is; whether an existing text tracker format (e.g. a hUGETracker export, or another GB tracker's text format) could serve; how a song becomes driver data in an RGBDS build (hUGETracker's export, GB Studio's path).
      - Anything else in GB Studio worth borrowing for an agentic tool (asset preview, bank packing, error wording).

      Output: a dated finding in project/ answering each question, ending with a recommendation for (a) the image leaf's exact checks and messages, and (b) whether to un-park music authoring now, with the shape it should take. Effort: one session; read code and docs, don't build. Out of scope: implementing anything.
    criteria:
      - "A dated doc in project/ answers each question with sources and recommends the image checks and the music decision"
  - title: "Image assets: rgbgfx by convention in build, hardware-limit checks with remedies, and a preview"
    ref: assets-images
    blockedBy: [assets-research]
    desc: |
      Why: an agent producing art for a Game Boy game needs to know, in one command, whether an image fits the hardware and what it will look like — the asset equivalent of the build-error loop. Framing: project/2026-11-07-asset-pipeline-plan.md.

      What: `build` converts images by convention (e.g. PNGs under `assets/` to tile data and tilemaps via rgbgfx, from PATH like the rest of RGBDS), and a limits check reports, as structured errors with remedies: unique tiles after dedup against VRAM (384 tiles) and what one map can address (256); colors per tile (4 on DMG) and palettes (8 of 4 on CGB); for sprite sheets, the 40-object and 10-per-line limits, 8x16 pairing, and VRAM left after backgrounds. Example message: "this background needs 291 unique tiles; a map addresses 256; these 35 tiles occur once". A preview renders the converted asset as the hardware will show it, through crates/view.

      Already decided: RGBDS from PATH at the pinned version (plan decisions table); logic in a library crate (the build crate from leaf 7qzbqc), CLI only wires it. The research leaf fixes the exact checks and wording — read its doc first and follow its recommendation unless the code proves it wrong.

      Seams: crates/build and the `build` verb (leaf 7qzbqc), crates/view for the preview. Out of scope: music; GBDK; an image editor.
    criteria:
      - "An over-budget background and a sprite sheet over a hardware limit each come back as a structured error naming the limit and a remedy (tests)"
      - "A PNG under assets/ in a `new` project is converted by `build` and shows on screen in a screenshot test"
```

> **Update, 2026-11-07: music un-parked (user ruling) and its text format.** After the research, the user decided to start music authoring now; the backlog entry is retired. The canonical text format (main thread's recommendation; the user was open to alternatives) is a **tracker grid**, because time is position: each row is a tick step and the four channels align across it, so agents see simultaneity and move phrases by rows, where MML/ABC make time a running sum of note lengths that models miscount; models have also seen a lot of OpenMPT/XM paste text and hUGETracker's clipboard uses it; and every `.uge` field has a home, so text → `.uge` → text is checkable. Agent-ergonomic choices: an explicit row number on every row; instruments named, with readable fields rather than register bytes; waves as 32 hex nibbles; orders that reference patterns by name; one canonical output spelling (all 64 rows, hUGETracker's hex effect codes with a `help` topic listing them); a piano roll rendered from the text file itself. An MML-style compose-only layer that compiles into pattern rows waits for evidence from the agent eval. Leaves: `U2rLOt` (text form) and `pukTN6` (emitter, build, `new`) under stage 6.
