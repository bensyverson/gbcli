# Design: pixel text, images an agent can write and read

2026-11-08, leaf XigG2u (decision for Ben; it blocks the build leaf uJIqBX). **Status: approved by Ben, 2026-11-08, with every open question's recommendation accepted.** The build is leaf uJIqBX, split into the crate and verb, then build integration.

Ben's ruling ([2026-11-08-examples-dx-review.md](2026-11-08-examples-dx-review.md), Decisions) asks for a pixel-art text format now. It is to images what song text is to music: an agent can write it, read it, convert it both ways, and `build` reads it. This doc proposes the format, the verb, and how both fit the image pipeline as it stands after the 2026-11-08 rulings: a plain build derives color art's DMG version ([GBC images plan](2026-11-07-gbc-images-plan.md), question 9, superseded), and leaf GQFHVi adds 8x8 sprite sheets.

## What the agents actually needed

Both example games wrote their art in Python: `examples/racer/tools/*.py` and `examples/shooter/tools/draw_art.py`. Their art falls into three groups.

- **Hand-drawn grids.** These are the shooter's ship, enemies, boss and bullet (rows of `0`-`3` with `.` for "leave alone") and the racer's font (rows of `#` and space). A legend maps each character to a color or a shade. This is what a text format replaces outright.
- **Glue around the grids.** This is a PNG writer (zlib and struct), helpers that split a 16x8 picture into 8x8 tiles, the palettes written twice (once in Python, once in `assets/palettes.txt`), and tile order kept by hand so `include/game.inc`'s tile numbers hold. A format with tiles, a legend that names palette entries, and file order as tile order removes this group too.
- **Procedural art.** These are the racer's road strips (one row per depth, computed from geometry), the mountains, an LCG starfield, ridge functions, glyph shadows, and a `light()` that swaps two shades. This group is programs. A text format doesn't replace it, and shouldn't try. A script can still write a PNG, or write pixel text instead.

So the format targets the first two groups. The review recorded that neither agent complained about the scripts. The payoff is less glue and art that can be read back, not a new capability.

## The format

A UTF-8 text file. The extension is `.pixels` and the first line is `format gbcli-pixels 1`. As in song text, a line's first word says what the line is.

```
format gbcli-pixels 1
size W H                 the image in pixels; required, once
color C VALUE            the legend: character C is VALUE (below)
fill C                   paint every pixel C first (optional)
rows [X Y]               a block of pixel rows, its top left at X,Y (default 0 0)
tile KEY                 an 8x8 tile: exactly 8 rows of 8
map [X Y]                a block of tile keys, one 8x8 cell each, from X,Y
```

- **Legend characters** are any printable ASCII character except `#` and space (93 of them), one character to a pixel. `#` is left out because it starts comments and colors. Space is left out because editors and agents trim it.
- **Values** come in four kinds:
  - `#rrggbb` or `#rgb`: a color, spelled as in `palettes.txt`. The file keeps all 8 bits; the build truncates to RGB555 as it does for a PNG.
  - `clear`: alpha 0, a sprite's transparent pixel.
  - `PALETTE N`: entry N (0-3) of a palette named in `assets/palettes.txt`. `dmg N` is shade N, the dmg line's color N (open question 4). This is how one project names its colors once.
  - `keep`: the pixel stays as earlier blocks drew it, so a block can be stamped over another.
- **Blocks.** A `rows`, `tile` or `map` line opens a block. The lines under it are its rows, and a blank line ends it. A row is one word: legend characters in a pixel block, tile keys separated by spaces in a map. Rows may be indented. Every row in a block has the same width. A tile is 8x8.
- **Tile keys** are words of letters, digits and `_` (`sky`, `gt`, `1f`). A map's rows must name tiles that are defined. A map has no flip bits, because the DMG's background has none.
- **Order.** Blocks paint in file order, later over earlier, after `fill`. A pixel that nothing paints is an error, never a silent default. A legend character must be defined before the first row that uses it.
- **Comments.** `#` followed by a space or tab, or `#` at the end of a line, starts a comment, on any line, as in `palettes.txt`. A line holding only a comment is skipped, even inside a block.
- **Partial alpha** has no spelling. The build refuses it anyway (`alpha`).

## How it maps to the image kinds

**The name convention is the PNG's, unchanged.** The file name and the folder carry everything they carry for a PNG, and the text carries only pixels. So there is one convention, and nothing can disagree with the name.

| On disk | Is |
|---|---|
| `assets/bg/town.pixels` | a background (unique tiles and a map) |
| `assets/sprites/hero.pixels`, `hero.16x16.pixels` | a sheet of 8x16 objects, frames of the size in the name |
| `assets/sprites/hero.8x8.pixels` | 8x8 objects, whatever grammar GQFHVi settles on, read from the name the same way |
| `assets/tiles/font.pixels` | a tileset: unique tiles in raster order, no map |
| `town.dmg.pixels` | a color image's DMG version, beside `town.pixels` or `town.png` |

- **Frames need no syntax of their own.** A sheet's frames are regions of the sheet: frame k of a 16x16 sheet is the block `rows 16k 0` (wrapping to the next frame row by the sheet's width, as the PNG does). The canonical writer emits one `rows` block per frame, with a `# frame k` comment.
- **Tiles and tilesets.** In the tileset folder, the map's order is the tile order: `map` then one line of keys is a font's charmap, in the order the program counts tiles. That is the order the shooter kept by hand.
- **Mixing forms.** Any image may be PNG or text, and any `.dmg` version may be either, in any mix. Two forms of one image (`town.png` and `town.pixels`) are a layout error naming both, as a `.song` and a `.uge` of one name already are.
- **The kinds of art.** These are decided by colors and assignments, as today:
  - DMG art and indexed art draw in `dmg 0`-`dmg 3`, or in four colors as a PNG would.
  - Color art draws in any colors: `#rrggbb`, or `PALETTE N` to reuse a named palette's colors.
  - Indexed art is assigned in `assets/palettes.txt` as now. The assignment names the file as it is on disk: `bg/town.pixels = forest` (open question 2). The named palettes stay in that one file, because they are the `NamedPalettes` seam LoadHigh sends.
- **The hardware limits** stay the build's checks, unchanged: size a multiple of 8, `obj_pair_8x16`, palettes, tile counts. The text parser checks only that the file is self-consistent.

## The verb: `gbcli pixels`

The verb is shaped like `gbcli song`, in the `build` class. It needs no session and no RGBDS.

```
gbcli pixels FILE [--text PATH] [--tiles] [--png PATH] [--preview PATH]
                  [--palettes FILE] [--json]
```

- **FILE** is a PNG (known by its signature) or pixel text. With no flags, the verb checks the file and prints one row, for example `pixels path=… form=text size=32x16 colors=3 clear=yes frames=2 tiles=6`. Here `tiles` counts unique 8x8 tiles by legend identity.
- **`--text PATH`** writes the canonical text. From a PNG:
  - the legend is made: `.` for clear, then `0`-`9`, `a`-`z`, `A`-`Z` and punctuation, lightest color first (by the build's lightness), ties broken by hex;
  - so four-shade art reads as `0`-`3`, the convention both agents chose for themselves;
  - colors are written `#rrggbb` at 8 bits, so nothing is lost.
- **Canonical text from text** keeps the author's legend (characters, `dmg N` and palette references unresolved). It flattens `fill`, `keep` and overlaps.
- **The canonical layout** is rows: one block per frame when the name gives a frame size, otherwise one block per 8-row band. Each block header is unique, so a block can be edited by its header. **`--tiles`** writes tiles and a map instead. The keys are hexadecimal in first-use order (`00`, `01`, …), so the map reads as the image's own tile numbers (`TownTilemap`'s bytes, as the build counts them).
  > **Corrected (2026-11-09, leaf D2zD2P):** the keys count tiles by legend character, so they are the build's tile numbers only until the build merges two: colors equal in RGB555 (`#ffffff`, `#fefefe`) are one color to it, and on the DMG colors of one shade draw alike. Merging like the build would need the target and the project's shading, and would lose the text's 8 bits, so the claim was made honest instead (help pixels says so); `tiles=` counts the same way.
- **`--png PATH`** writes the image at 1:1, the PNG the build would read. **`--preview PATH`** writes it four times larger with an 8x8 grid, for looking only; it never belongs in `assets/`.
- **`--palettes FILE`** resolves `PALETTE N` and `dmg N`. The default is the `assets/palettes.txt` of the project the file sits in. When a reference can't be resolved, the verb refuses and names the flag.
- **Round trip.** PNG to text to PNG is pixel for pixel. Text to PNG to text gives the canonical text with a made legend; it is the same picture, not the same bytes.
- **Failures:**
  - A file that doesn't parse is `pixels_unreadable` (exit 4), with a `problem` row giving path, line, column, message and remedy, exactly as `song_unreadable` does.
  - A PNG the format can't hold is also exit 4, naming the first pixel at fault: a half-transparent pixel, or more colors than legend characters (93; no GB image comes close, since a `--gbc` background holds at most 32).
  - Nothing is written unless the whole file reads.

## Build integration

**`build` and `assets` read `.pixels` directly, alongside PNGs** (open question 3). In practice:

- `find` accepts `.pixels` beside `.png`.
- The reader tells the two apart by content, as `read_song` tells `.uge` from text. So an `assets --manifest` entry's `file` may be either, with no schema change.
- The text becomes the same RGBA pixels a PNG decodes to, and every check runs unchanged.
- rgbgfx reads files, so each text image is written to a transient PNG beside the data. The shared-shade merge already does this (`shared.rs`, removed by `Removed`). rgbgfx's messages are already rewritten to name the source the user drew (`run.rs`).
- Nothing is ever written into `assets/`. The previews are made as today.

**Errors.** A text image that doesn't parse is an image `problem`, not a stop: `limit=pixels_text`, with `line=` and `column=`. So the build still checks every other image, as `palette_file` does. `ImageProblem` already has `line`; it gains `column`. For example, a typo in the meadow example below:

```
problem image=assets/bg/meadow.pixels limit=pixels_text severity=error line=52 column=7 message="no tile gx: the tiles are sk, cl, gt and di" remedy="name a tile defined with `tile KEY`, or define gx"
```

**Hardware problems on a text image** keep `tiles=x,y` and also give `line=`, pointing at the map cell or the row block that drew the first tile named. The field is already there, and it is the one thing a PNG can't offer (open question 5).

> **Refined (2026-11-09, leaf D2zD2P):** "the block that drew the first tile" was read from the tile's top-left pixel, so a `fill` under that corner won over the block that drew the colors. `line=` is now the line that drew the most of the tile's legend characters, the later on a tie (`pipeline::drew`).

## Large images: a 256x256 background

A raster this size isn't readable by anyone, so the design doesn't pretend otherwise. It gives three answers.

- **Tiles and a map** are the compact form. The racer's `road.png` is 256x256 with 116 unique tiles (`gbcli assets examples/racer`). As rows, that is 65,536 pixel characters on 256 lines of 256. As tiles, it is 116 x 64 = 7,424 pixel characters plus a 32x32 map of 2-character keys: about 11,000 characters, a sixth of the rows. The map is 32 short lines that show the image's structure (repeats, bands) at a glance. The 160x144 title (120 tiles) is 23,040 characters as rows, against about 9,700 as tiles.
- **Unique headers.** Every block opens with a unique header (`rows 0 120`, `tile 1f`). An agent's edit tool, which needs a unique string to anchor on, can address any part of a big image.
- **Procedural backgrounds stay procedural.** The road is geometry, one row per depth. Its source of truth is the script that computes it, which may write a PNG or pixel text. `gbcli pixels road.png --text --tiles` is still how an agent reads what the script made.

## Coexisting with PNGs from any tool

- **Every PNG keeps building exactly as now.** Whether it came from a paint program, a vector tool's render or a script, the help says "a PNG from any tool, or pixel text". It names no tool. Ben's ruling keeps woodcase out of gbcli's help; a game brief may recommend it for larger pieces.
- **One source of truth per image.** A PNG rendered from another tool's file stays a PNG in `assets/`. An agent may run `gbcli pixels x.png --text` to read it or to inspect a pixel. But a text copy edited and kept would be lost the next time the PNG is rendered, so the help says to keep only one form in `assets/`. The layout error enforces that for one image.
- **Text imported from a PNG** becomes the source when the agent means to hand-edit from then on. The import is lossless, so nothing is given up.

## What it deliberately doesn't do

- **It is not a drawing program.** It has no lines, rectangles, circles, flood fill, gradients or dithering. It has no transforms: no flips, rotations, scaling, or palette swaps of a block. These are a script's job, or another tool's.
- **No procedural generation**: no expressions, loops or random fills. A script that needs them writes the file.
- **No animation data.** Frame timing, metasprite offsets and frame names stay with the program. The frame size stays in the file name.
- **No vector art, no layers** beyond paint order, and **no partial alpha**.
- **No editing verbs.** There is no `gbcli pixels set X Y C`. The file is the interface; an agent edits it as text.
- **No second palette store.** Named palettes and assignments stay in `assets/palettes.txt`.

## Worked example 1: a two-frame sprite sheet (color art)

`assets/sprites/hero.16x16.pixels`: two 16x16 frames side by side, 3 colors plus clear, so each 8x16 object fits an OBJ palette.

```
format gbcli-pixels 1
# The hero: two 16x16 frames, standing and stepping. Color art: a plain
# build derives its DMG shades; build --gbc packs its colors.
size 32 16
color . clear
color k #201018   # outline and eyes
color s #f0c090   # skin
color r #d03030   # tunic

rows 0 0   # frame 0: standing
................
.....kkkkkk.....
....kssssssk....
....kskssksk....
....kssssssk....
.....kssssk.....
....kkrrrrkk....
...krrrrrrrrk...
...ksrrrrrrsk...
...ksrrrrrrsk...
....krrrrrrk....
....krrkkrrk....
....krk..krk....
....krk..krk....
...kkk....kkk...
................

rows 16 0   # frame 1: stepping
................
.....kkkkkk.....
....kssssssk....
....kskssksk....
....kssssssk....
.....kssssk.....
....kkrrrrkk....
...krrrrrrrrk...
..ksrrrrrrrrsk..
..kk.krrrrk.kk..
....krrrrrrk....
...krrk..krrk...
..krk......krk..
..krk.......kk..
.kkk........kkk.
................
```

```
$ gbcli pixels assets/sprites/hero.16x16.pixels --preview hero.png
pixels path=assets/sprites/hero.16x16.pixels form=text size=32x16 colors=3 clear=yes frames=2 tiles=6
```

A typo on line 13 (`x` in place of an `s` at column 9) fails with the usual shape:

```
problem path=assets/sprites/hero.16x16.pixels line=13 column=9 message="x isn't in the legend: it has ., k, s and r" remedy="add `color x VALUE`, or use a legend character"
gbcli: pixels_unreadable: assets/sprites/hero.16x16.pixels:13:9: x isn't in the legend: it has ., k, s and r
```

## Worked example 2: a background in tiles and a map (indexed art)

`assets/bg/meadow.pixels`: 64x24, four tiles repeated through a map, drawn in shades and colored on the GBC by a named palette.

```
# assets/palettes.txt
dmg:    #ffffff #aaaaaa #555555 #000000
meadow: #f0f8ff #a8d8f0 #58a040 #604020   # cloud, sky, grass, dirt
bg/meadow.pixels = meadow
```

```
format gbcli-pixels 1
# A meadow strip for the title. Indexed art: drawn in the four DMG shades,
# colored on the GBC by the meadow palette (assets/palettes.txt).
size 64 24
color 0 dmg 0   # cloud
color 1 dmg 1   # sky
color 2 dmg 2   # grass
color 3 dmg 3   # dirt

tile sk
11111111
11111111
11111111
11111111
11111111
11111111
11111111
11111111

tile cl
11111111
11100111
11000011
10000001
00000000
10000001
11111111
11111111

tile gt
11111111
11111111
11111111
21112111
22122112
22222222
22222222
22222222

tile di
33333333
33323333
33333333
33333323
33333333
32333333
33333333
33333233

map 0 0
sk cl sk sk sk cl cl sk
gt gt gt gt gt gt gt gt
di di di di di di di di
```

`gbcli build` converts it like any background (`MeadowTiles` holds 4 tiles; `MeadowTilemap` is the 24 cells). `gbcli pixels assets/bg/meadow.pixels --text m.pixels --tiles` writes the same picture with keys `00`-`03` in first-use order (sk, cl, gt, di). The map then reads `00 01 00 00 00 01 01 00`, the bytes of `MeadowTilemap`'s first row.

## Where it lives

- **A new library crate, `crates/pixels` (`gbcli-pixels`)**, shaped like `crates/music`. It holds the model, the parser (`ParseError { line, column, message, remedy }`, as music's), the canonical writer, and PNG read and write. The PNG decode moves there from `crates/build/src/images/pixels.rs`, so one reader serves both forms.
- **No new dependency.** The `png` crate is already in the workspace.
- **Thin adapters.**
  - `crates/build` calls the crate from `find`, input and transient-PNG code.
  - `crates/cli` gets the `pixels` verb, a `gbcli help pixels` topic, a line in `help images` ("PNGs or pixel text"), and a recipe ("read an image as text").
  - `crates/protocol` gets `ImageLimit::PixelsText`, `ImageProblem.column`, `ErrorCode::PixelsUnreadable`, and the verb's result struct.
  - Docs: `docs/architecture.md` gets the crate.
- **Acceptance criteria for uJIqBX worth taking:**
  - every PNG under `examples/` (16 images) goes PNG to text to PNG pixel for pixel, in both layouts;
  - an example game with its PNGs replaced by their `.pixels` builds a byte-identical ROM, plain and `--gbc`;
  - every parse error has a test that reads its line, column and remedy.
- **Splitting uJIqBX.** It may be two leaves: the crate and verb, then build integration. Merge cost decides.

## Where this departs from the leaf

- **Frames get no syntax.** The leaf asks for "frames or tiles for sheets". The frame size is already in the file name, and a second copy in the file could disagree with it. Positioned `rows` blocks cover frames; `tile` and `map` cover tiles.
- **Shades are a legend value, not a separate system.** The pipeline gives shades to colors, project-wide, by its cascade. So `dmg N` resolves to the dmg line's color before any check runs.
- **"Build reads the text directly" goes through a transient PNG,** because rgbgfx reads files. The user never sees it.

## Open questions for Ben

1. **Names.** Should the verb, extension and first line be `gbcli pixels`, `.pixels` and `format gbcli-pixels 1`? *Recommend yes:* verb, extension and format share one word, as `song` does. `.pix` is shorter but drifts from the verb.
2. **Assignments for text images.** Does `palettes.txt` name the file as it is (`bg/town.pixels = forest`), or the image with no extension (`bg/town = forest`)? *Recommend the file as it is.* The current grammar and checks then stay untouched, and an "unknown image" error's remedy can name the other form when it exists.
3. **Does `build` read `.pixels` directly?** *Recommend yes,* through a transient PNG. The alternative, agents running `gbcli pixels --png` before each build, brings back the generated-PNG step and lets the two copies go stale.
4. **`dmg N` with no dmg line in `palettes.txt`.** Rendering it as the DMG grays can land on the wrong shade: the cascade stretches over all of the project's colors, so art drawn in `dmg 1`-`dmg 3` alone moves. *Recommend `pixels_text`,* an error whose remedy is the exact line to add (`dmg: #ffffff #aaaaaa #555555 #000000`). The alternative, treating such pixels as declared shades inside the cascade, would quietly change how every other image is shaded.
5. **Line numbers on hardware problems** (`line=` for `gbc_tile_colors` and the like on a text image). *Recommend in uJIqBX.* It is cheap, since the field exists, and it is the main reason to prefer text to a PNG when something doesn't fit.
6. **The default layout for `--text`.** Should it be rows (one block per frame, or per 8-row band) with `--tiles` to opt in, or chosen by folder (tiles for `bg/` and `tiles/`, rows for `sprites/`)? *Recommend rows by default with `--tiles`.* One rule to explain beats a guess by folder. `help pixels` tells agents to use `--tiles` on large backgrounds.
7. **Tile and frame names as program constants** (`FONT_TILE_A`, `HERO_FRAME_STEP`). The shooter hard-coded tile numbers in `include/game.inc`. *Recommend backlog,* to un-park if a third game hard-codes tile or frame numbers again. It changes `images.inc`, the LoadHigh seam, and the dedupe question (a merged tile would have two names).
8. **A `.pixels` in `gbcli new`.** *Recommend no for now.* The starter draws no image today, so adding one means starter code to show it. The help's example and `gbcli pixels` cover onboarding.
