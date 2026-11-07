# Plan: GBC images in dual mode (`build --gbc`)

2026-11-07, leaf `k5Afkg`. **Status: revision 2, approved by Ben 2026-11-07 and imported under the root `9b5RBA` as Stage 10.** The tracker is now the plan.

The evidence is in [2026-11-07-gbc-images-research.md](2026-11-07-gbc-images-research.md).

> **Revision 2 (2026-11-07), from the review of LoadHigh's assets design** (`../loadhigh/project/design/09-assets.md`), agreed by Ben and LoadHigh's agent before Ben answered revision 1. Four changes, all carried below:
> 1. **Two kinds of art.** Color art, with palettes derived and packed automatically, stays the default. Beside it, **indexed art**: four-shade art colored on the GBC by a *named* palette, which a program can swap at run time. Explicit palette assignment leaves the backlog. Questions 4 and 5 are re-asked with new recommendations.
> 2. **One named-palette file**, `assets/palettes.txt`, replaces ygc9bb's `assets/palette.txt` (unreleased, so no migration). The DMG shades are the reserved name `dmg`. `#rgb` is CSS shorthand for `#rrggbb`, rounded to RGB555 like every color.
> 3. **The shared seam is gbcli's protocol types, not the file.** LoadHigh declares palettes in `.ldh` and sends the same struct the file parses into. The GBC conversion results land as protocol types first; `images.inc` is generated from them; a new `assets` op converts without building a ROM; and tilesets without maps are supported, since LoadHigh builds its own maps and attribute maps from tile kinds.
> 4. **gbcli settles these, and LoadHigh passes them through:** the check names (`gbc_palettes`, `gbc_tile_colors`, GBC per Ben's naming ruling), the flip rule (no BG flips in dual mode), and the `name.dmg.png` override.

## Goal

An agent making a Game Boy Color game draws its art in color. `gbcli build --gbc` turns that art into a GBC-enhanced ROM that still runs on the original Game Boy (`rgbfix -c`, header $80). Without `--gbc`, the build is unchanged.

## Rulings already made (Ben, 2026-11-07)

- A flag chooses the target: `gbcli build --gbc`. User-facing names say "GBC". The docs map that to Pan Docs' "CGB" and to rgbfix's `-c` (dual mode) and `-C` (GBC only).
- The goal is dual mode, so every image needs a color version and a 4-shade DMG version.
- DMG shades (leaf `ygc9bb`): each color takes the shade nearest its lightness, even when two colors share a shade. The build then warns. More colors than the hardware takes is an error.

## Recommended design (the questions below can change it)

**Two kinds of art.**
- **Color art** (the default): an image drawn in any colors. gbcli packs its GBC palettes and derives its DMG version.
- **Indexed art**: an image drawn in four shades, exactly as DMG art is today, that `assets/palettes.txt` assigns a named palette (`bg/town.png = forest`). The assignment is what makes it indexed. Its shades are read by the same cascade as DMG art (the `dmg` palette, then a known palette, then the project stretch). On the DMG, the shades are the art. On the GBC, the tile data is the same bytes, and the named palette colors it. A program swaps the look by loading another named palette: every palette in the file is emitted as data, whether or not an image uses it.
- So indexed art needs no derived version and no override, and its tiles are stored once for both machines.

**The palette file, `assets/palettes.txt`.** One palette a line, four colors, index 0 (the DMG's lightest) first; one image assignment a line. Comments and blank lines as today's `palette.txt`.

```
dmg:    #e0f8d0 #88c070 #346856 #081820   # reserved: the colors that mean shades 0-3 in the art
forest: #fff #8c4 #363 #021                 # #rgb is #rrggbb (#8c4 = #88cc44), rounded to RGB555
ember:  #ffe #f80 #a20 #200
bg/town.png = forest
sprites/hero.png = ember
```

- `dmg` replaces `palette.txt`'s four lines, and keeps its meaning: the first rule of the shade cascade.
- An unknown palette name, an assignment to a missing image, a duplicate name, or `dmg` assigned to an image is `palette_file`, with the line.
- For an object, index 0 is transparent, so the palette's first color is ignored on the GBC; the help says so.
- The file parses into one protocol struct, `NamedPalettes` (names, RGB555 colors and assignments). The `assets` op takes that struct directly, which is how LoadHigh sends the palettes it declares in `.ldh`.

**Where color art's DMG version comes from: a hybrid.**
- By default, the DMG version is **derived from the color image**, each color taking its nearest DMG shade by Ben's ruling.
- An optional **`name.dmg.png`** beside the color image overrides the derived version.

**The tile data is separate for each target, behind one shared tilemap and frame list.**
- gbcli deduplicates tiles on the pair (DMG tile, GBC tile), without flips. Two tiles count as one only if both versions match.
- So the map and the frame lists are the same on both machines. The GBC adds an attribute map and palettes.
- The program copies `TownTiles` on a DMG or `TownTilesGbc` on a GBC; nothing else differs.

**Why not GB Studio's single shared tile data?**
- In GB Studio, a pixel's index in the GBC palette *is* its DMG shade index, read through the DMG's one global BGP.
- Two colors in one tile that share a nearest shade then can't be represented: they need different indices on the GBC and the same shade on the DMG.
- GB Studio gets around that by spreading shades within each palette. That is the rule Ben just retired for DMG art.
- The cost of separate data is about 4 KB more ROM for a full 256-tile screen. The map isn't duplicated.

**Color art's palettes are packed automatically by gbcli, deterministically.**
- rgbgfx can't name the tiles that force a ninth palette, and the answer isn't unique anyway (research §3).
- So gbcli packs the palettes itself and passes them to rgbgfx with `-c`. Then it checks rgbgfx's output pixel by pixel, as `rgbgfx_agrees` does today.
- Each background gets up to 8 BG palettes, with one background resident at a time, as `vram_tiles` counts today. An indexed background uses one slot, its assigned palette.
- All sheets share the 8 OBJ palettes: the packed palettes of color sheets plus one slot for each distinct palette assigned to an indexed sheet. Palettes loaded only by the program for swaps are not counted; the program owns those slots.

**Flips.** No BG flips in dual mode: the DMG's tilemap has no flip bits, so tiles that differ only by a flip are kept twice. Objects can flip on both machines, but sheets aren't deduplicated by flips today, and this stage doesn't add it.

**The seam: protocol types and an `assets` op.**
- The GBC conversion results are protocol types in `crates/protocol/src/images.rs`, beside `ImagesMade`: per image, the tile count, the source tile → tile index map, the palettes (RGB555), the palette of each tile or object, and the problems. `images.inc` and `images.asm` are generated from them, so the include file and the op can't disagree.
- A new `assets` op (and CLI verb) converts a project's images, or a manifest of files and options plus a `NamedPalettes`, without building a ROM, and returns those types.
- **Tilesets without maps**: images under `assets/tiles/` are converted to tiles only, deduplicated, with no tilemap. LoadHigh builds its maps and attribute maps itself from tile kinds.

**When the shared-shade warning fires in `--gbc` (color art).** Color art routinely maps many colors onto one shade, so ygc9bb's warning would fire on every image. In `--gbc`, it fires only when two colors **in the same tile or object** fall to one shade, since detail is lost there. The remedy names `name.dmg.png`.

**Sessions.**
- `run` gets a `gbc` model alias. A dual ROM is checked in two sessions, one per model, because restore refuses a model change.

## Questions for Ben

> **Answered by Ben, 2026-11-07.** Question 4: the recommendation, one named palette per indexed image (after asking what it implies: only indexed art is held to one palette; color art keeps up to 8 per background). A third way that surfaced in the discussion, color art matched tile by tile to named palettes, is in [backlog.md](backlog.md). Question 5: the recommendation. Questions 1-3 and 6-10: the recommendations, accepted as written.

Each question gives the recommended answer first and names the leaf that waits on it.

1. **Tile data.** Separate data per target behind a joint map (recommended), or GB Studio's single shared data: half the image ROM, but it breaks the nearest-shade ruling inside a tile. Leaf `joint`.
2. **The shared-shade warning with color art.** Warn only within a tile or object (recommended), give one summary per image, or stay silent. Leaf `derive`.
3. **The override's name.** `.dmg.png` (recommended, matching gbcli's "DMG" naming), GB Studio's `.mono.png`, or both. Leaf `derive`.
4. **Palette assignment (re-asked in revision 2).** Recommended: **color art packed automatically, plus indexed art with a named palette assigned per image** in `assets/palettes.txt`, the assignment marking the image indexed. Assignment per tile within one plain-gbcli background is left out: a program colors tiles differently by swapping palettes, and LoadHigh assigns palettes per tile kind and builds the attribute map itself. Alternatives: mark indexed art by folder (`assets/bg-indexed/`) or by filename (`town.forest.png`), which ties the label to the palette; or allow a second, palette-index PNG per background for per-tile assignment. Leaves `palettes`, `indexed`.
5. **The palette budgets (re-asked in revision 2).** Recommended: **the build counts what it places, and the program owns the rest.** Per background, packed palettes plus its one assigned palette; across all sheets, packed OBJ palettes plus one per distinct assigned palette; each at most 8. Palettes in the file that no image uses are emitted as data and not counted, since the program decides when they're resident. Per-scene budgets stay in the backlog: gbcli can't see scenes, and LoadHigh, which can, counts its own. Alternatives: count every named palette against the 8 (simple, but it refuses a project with ten swap palettes it never shows at once); or count nothing (no check catches nine packed palettes). Leaf `pack`.
6. **`run`'s model for a $80 or $C0 ROM.**
   - The default: keep DMG, with a `--model gbc` alias (recommended: explicit, and it keeps today's sessions as they are), or choose GBC from the header.
   - The `cgb` name: should it stop meaning Cgb0, an early revision? Recommended: `gbc` → CgbE, with `cgb` unchanged.
   - Leaf `model`.
7. **Scope.** GBC-only ROMs (`-C`, VRAM bank 1, BG flips) go to the backlog (recommended).
8. **Previews.** They match gbcli's SameBoy setup (no color correction), not the look of a real GBC LCD. Acceptable? (Recommended: yes, since previews must match screenshots.) Leaf `api`.
9. **`.dmg.png` without `--gbc`.** Recommended: an error, naming `--gbc`, so a stray file isn't silently ignored. Leaf `derive`.
10. **Palettes and assignments without `--gbc` (new in revision 2).** Recommended: `dmg` still drives the shade cascade, the named palettes are checked and emitted (so a DMG build of a GBC-ready project still assembles), and indexed art builds as DMG art. Alternative: refuse non-`dmg` lines without `--gbc`, as question 9 does for `.dmg.png`. The difference: an assignment is harmless on the DMG, while a `.dmg.png` would be silently ignored. Leaf `palettes`.

## Order and why

- The header and the model alias come first: they are small, and every later test needs a GBC session.
- Then the protocol types (revision 2): every later leaf produces or consumes them, and LoadHigh can code against them as soon as they land.
- Then the palette file, which replaces `palette.txt` and feeds both kinds of art.
- Next come palette packing and the derived DMG version (color art), and indexed art, then the joint conversion, which consumes all three.
- Then the include API and previews, generated from the protocol results, and the `assets` op beside it; then the starter, which consumes the API.
- Docs and the review come last.

## Tasks

```yaml
tasks:
  - title: 'Stage 10: GBC images in dual mode (build --gbc)'
    desc: |
      Color art in, a dual-mode ROM out (rgbfix -c, header $80, still runs on DMG). Design, rulings and Ben's answers: project/2026-11-07-gbc-images-plan.md. Evidence: project/2026-11-07-gbc-images-research.md.
    children:
      - title: 'build --gbc writes the dual-mode header'
        ref: header
        desc: |
          Why: everything after this needs a target flag to hang on. What: a --gbc flag on `gbcli build` (crates/build, wired in crates/cli). rgbfix gets -c, and the build result records the target. Without the flag nothing changes: today's call is `rgbfix -v -p 0xFF` at crates/build/src/project.rs:263.

          Decided (Ben, 2026-11-07): user-facing names say "GBC". The help maps that to Pan Docs' "CGB" and to rgbfix's -c (dual mode) and -C (GBC only; out of scope here). No image changes in this leaf.
        criteria:
          - $0143 is $80 with --gbc and $00 without (test)
          - gbcli help names the GBC/CGB and -c/-C mapping (help golden)
      - title: 'Sessions run a ROM as GBC: the gbc model alias'
        ref: model
        desc: |
          Why: verifying a dual ROM takes one session per model (restore refuses a model change: ModelConflict, crates/engine/src/machine.rs:287-291). What: model_from_name (crates/protocol/src/checkpoint.rs:209-223) gains `gbc` → CgbE. Today `cgb` means Cgb0, an early revision.

          Ben's answer to plan question 6 sets the default model for a $80 ROM and whether `cgb` changes. If he hasn't answered, keep the DMG default and leave `cgb` as it is.
        criteria:
          - a $80 ROM under --model gbc reports CGB mode, and its shot shows palette-RAM colors (test)
          - the DMG default and ModelConflict are unchanged (test)
      - title: 'GBC conversion results as protocol types'
        ref: types
        blockedBy: [header]
        desc: |
          Why: the protocol types are the seam LoadHigh imports (ldh-assets defines no twin), and images.inc must be generated from the same results the op returns. What, in crates/protocol/src/images.rs beside ImagesMade, with serde round-trip tests and schema entries:
          - NamedPalettes: palette names, four RGB555 colors each, and image assignments (what assets/palettes.txt parses into and what the assets op accepts);
          - per image: the target(s), tile count, the source tile -> tile index map, the palettes used, the palette of each tile or object, and whether the art is color or indexed;
          - ImageKind gains a tiles-only kind (assets/tiles/, no map);
          - ImageLimit gains gbc_palettes, gbc_tile_colors and dmg_override_size, in the shape of today's limits.
          No conversion logic; later leaves fill the types.
        criteria:
          - every new type round-trips through serde_json, and the schema lists it (tests)
          - existing DMG builds produce the same ImagesMade JSON as before (golden unchanged)
      - title: 'assets/palettes.txt: named palettes, the reserved dmg name, and assignments'
        ref: palettes
        blockedBy: [types]
        desc: |
          Why: one file for plain gbcli users, parsed into the NamedPalettes protocol struct; it replaces ygc9bb's assets/palette.txt while that is unreleased (no migration). What, replacing crates/build/src/images/palette.rs:
          - `name: c0 c1 c2 c3` lines, colors #rrggbb or CSS-shorthand #rgb, each rounded to RGB555; `path = name` assignment lines (path relative to assets/); comments, blank lines and BOM as today;
          - `dmg` is reserved: the colors that mean shades 0-3, the first rule of the shade cascade (shades.rs), exactly as palette.txt was;
          - palette_file errors with the line: unknown name, missing image, duplicate name or assignment, dmg assigned to an image, a line that's neither form;
          - every named palette is emitted as data (labels in the api leaf); without --gbc, per plan question 10.
          Rename every palette.txt mention (help, design §12, tests) in this leaf.
        criteria:
          - the edge cases of images_shade_edges.rs pass against the new file (tests moved, not dropped)
          - '#8c4 reads as #88cc44 and then RGB555 (test)'
          - each palette_file error names its line (tests)
          - a second color after a color on one line is a palette_file error, not a comment (test; review of d0e891a found palette.txt drops it)
      - title: 'Color census and deterministic palette packing'
        ref: pack
        blockedBy: [types]
        desc: |
          Why: rgbgfx can't name the tiles that force a ninth palette, and the answer isn't unique (research §3), so gbcli packs palettes itself. What: a library module in crates/build/src/images:
          - the set of RGB555 colors in each tile (backgrounds) and each object (sheets);
          - deduplicate the sets and drop subsets;
          - deterministic first-fit-decreasing packing with a small bounded search;
          - a lower bound of ⌈colors/4⌉ (⌈colors/3⌉ for sprites).

          Problems, in the shape of the existing ImageLimit problems (crates/protocol/src/images.rs):
          - `gbc_palettes`, an error naming the tiles that landed in palette 9 or later, their colors, and the lower bound;
          - `gbc_tile_colors`, 5 colors in one tile, or 4 opaque colors in one object, named by tile coordinates.

          Budgets (plan question 5): per background, packed palettes plus its assigned palette, at most 8; across all sheets, packed OBJ palettes plus one per distinct assigned palette, at most 8. Palettes only the program loads aren't counted. Results go into the protocol types from leaf types.
        criteria:
          - 'unit tests replay research §3: nine disjoint 4-color tiles and seventeen 2-color tiles both error with bound 9; sixteen 2-color tiles pack into 8'
          - the eight-plus-bridge case's error names the bridge tile (test)
          - a 5-color tile is named by its coordinates (test)
          - the same input always gives the same palettes (test)
      - title: 'Derived DMG version, and the .dmg.png override (color art)'
        ref: derive
        blockedBy: [pack]
        desc: |
          Why: dual mode needs a 4-shade version of every color image. What:
          - Under --gbc, the DMG version is derived from the color image: each color takes its nearest DMG shade by lightness. This is Ben's rule from leaf ygc9bb; reuse its code in crates/build/src/images/shades.rs.
          - ygc9bb's shared-shade warning is scoped to colors within one tile or object (plan question 2), with the remedy naming the override.
          - An optional sibling `name.dmg.png` (question 3) overrides the derived version. It must be the same size as the color image (`dmg_override_size` error).
          - A derived sheet that comes out with 4 opaque shades is a dmg_palette error, with the override as the remedy.
          - A .dmg.png in a build without --gbc is an error naming --gbc (question 9).
          - A .dmg.png beside indexed art is an error: its shades already are the DMG version.
        criteria:
          - derived shades match the nearest-shade rule (test)
          - an override wins over the derived version, and a size mismatch is an error (tests)
          - two colors in one tile that share a shade warn, naming the tile; across tiles they don't (tests)
      - title: 'Indexed art: four-shade tiles colored by a named palette'
        ref: indexed
        blockedBy: [palettes, types]
        desc: |
          Why: palette swaps (plan question 4): one set of tiles that a program recolors by loading another palette, the way GB Studio and LoadHigh use palettes. What:
          - an image assigned a palette in assets/palettes.txt (or in a NamedPalettes sent to the assets op) is indexed: its shades come from the existing cascade, and the GBC tile data is the DMG tile data;
          - its attribute map (backgrounds) or frame attributes (sheets) carry the assigned palette's slot;
          - more than four shades is today's dmg_palette error; for a sheet, four opaque shades is the existing error.
        criteria:
          - an indexed background's GBC and DMG tile bytes are identical, and its attribute map names one slot (test)
          - two images assigned one palette share its slot; the slot count feeds the pack leaf's budget (test)
      - title: 'Joint conversion: per-target tiles behind one map and frame list'
        ref: joint
        blockedBy: [derive, indexed]
        desc: |
          Why: separate tile data per target keeps Ben's shade ruling inside tiles; a shared map keeps the program simple (plan question 1). What:
          - Deduplicate tiles on (DMG tile, GBC tile) pairs, with no flips (the DMG has no BG flip), into one tilemap or frame list.
          - Make the GBC tiles with `rgbgfx -c <packed palettes>` in gbcli's slot order, plus an attribute map (palette in bits 0-2).
          - The agreement check (crates/build/src/images/agree.rs) compares pixel colors, never palette numbers: rgbgfx may choose another palette that fits (research §3).
          - The limits bg_tiles_map, vram_tiles and obj_tiles count joint tiles.
          - Tiles-only images (assets/tiles/) are deduplicated the same way, with no map; their source tile -> index map is the result.
          - Results fill the protocol types from leaf types.
        criteria:
          - rebuilding both versions from the generated data reproduces both PNGs pixel for pixel (test)
          - a tile identical in gray but different in color is kept twice (test)
          - the agreement check fails a GBC conversion that disagrees with the plan (test)
      - title: 'Include API and GBC previews'
        ref: api
        blockedBy: [joint]
        desc: |
          What: generated from the protocol results (leaf types), next to today's labels in images.inc and images.asm:
          - per background: TownTilesGbc..End (same count and order as TownTiles), TownAttrmap..End (one byte per map entry), TownPalettes..End (RGB555 dw, 8 bytes per palette) and TOWN_PALETTES;
          - per sheet: HeroTilesGbc;
          - for all sheets: ObjPalettesGbc..End and OBJ_PALETTES;
          - frame-list attribute bytes carry the CGB palette in bits 0-2 beside the DMG OBP bit (each machine ignores the other's bits);
          - per named palette in assets/palettes.txt: PaletteForest..End (8 bytes, RGB555 dw) and PALETTE_FOREST, its index in file order, whether or not an image uses it;
          - indexed images have no *TilesGbc label (their tiles are shared); their attribute bytes name the assigned palette's slot;
          - GBCLI_GBC EQU 1 when built with --gbc.

          Previews: `*.gbc.preview.png` beside today's DMG preview, with a palette swatch strip. Draw it with SameBoy's channel scaling, (x<<3)|(x>>2) with no color correction (research §4), so it matches screenshots (plan question 8).
        criteria:
          - a program using every new label assembles, and the constants appear in the .sym (test)
          - the GBC preview goldens pass, and a preview pixel equals the screenshot pixel for the same color (test)
      - title: 'The assets op: convert images without building a ROM'
        ref: assets
        blockedBy: [joint]
        desc: |
          Why: LoadHigh needs the tile indices and palettes to emit metasprite tables and attribute maps, before and apart from a build (its 09-assets.md, "Shared types across the seam"). What: a server op `assets` and CLI verb `gbcli assets` (crates/server, crates/cli; the decision lives in crates/build, the adapters only wire it):
          - input: a project directory, or a manifest of image files and options plus a NamedPalettes struct, and the target (DMG or GBC);
          - output: the protocol results from leaf types, the problems, and optionally the previews; nothing written to the project's build/ unless asked;
          - read cost: no rgbasm or rgblink run.
          Follow project/agents/cli-design.md for the verb.
        criteria:
          - the op's results equal the ones build records for the same project (test)
          - a manifest with a NamedPalettes and a tiles-only image converts without a project (test)
          - help, schema and the design doc list the op (goldens)
      - title: 'Dual-mode starter: detect GBC, load palettes and the attribute map'
        ref: starter
        blockedBy: [api, model]
        desc: |
          What: the starter (crates/build/templates/main.asm) does the following.
          - It saves `a == BOOTUP_A_CGB` to an HRAM flag at Start, before `ldh a,[rLY]` overwrites A (lines 19-30).
          - On a GBC, with the LCD off, it loads the BG and OBJ palettes through BCPS/BCPD and OCPS/OCPD with autoincrement.
          - It writes the attribute map with VBK=1, then sets VBK back to 0.
          - It copies the GBC tiles.

          The DMG path must not touch VBK. A DMG has no VBK, so the attribute writes would overwrite the tilemap. Decide whether `new --gbc` or `build --gbc` brings the color path into the starter, and say which in the report.
        criteria:
          - one ROM in two sessions (--model dmg, --model gbc) gives the expected shot in each (goldens)
          - run --raster on the DMG session shows no VBK, BCPS or attribute-map writes (test)
      - title: 'GBC images: help, design and backlog'
        ref: docs
        blockedBy: [starter]
        desc: |
          What:
          - A GBC section in `gbcli help images`: the convention, the override, the limits, the labels, and a recipe that verifies a ROM in two sessions.
          - A dated update to design §12.
          - In obj_no_transparency, a hint naming GB Studio's #65ff00 when a sheet has no alpha-0 pixel but has GB Studio's key green (g>249, r<180, b<20). No automatic keying.
          - The two kinds of art and assets/palettes.txt in the help, with a palette-swap recipe.
          - Backlog entries, each with its un-park condition: GBC-only (-C) builds, VRAM bank 1 tiles and BG flips, per-tile palette assignment in plain gbcli, and per-scene palette budgets.
        criteria:
          - the help golden is updated, and scripts/help/check-doc-links.py passes
          - backlog.md carries the four entries
      - title: 'Independent review of the GBC images stage'
        ref: review
        blockedBy: [docs]
        desc: |
          Review leaves types, palettes, pack, derive, indexed, joint, api, assets and starter by project/agents/review.md. The new protocol types (LoadHigh's seam), the generated-include ABI and the palette file's grammar are the risk points.
        criteria:
          - the confirmed findings are filed as leaves or fixed, and the reviewed leaves carry the reviewed label
```
