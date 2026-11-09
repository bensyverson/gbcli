# Design: pixel fonts, fixed and variable width

2026-11-09, leaf 5UVSjE (decision for Ben). **Status: ruled 2026-11-09, in a revised form: see "Ben's decisions" at the end, which supersede the options below wherever they differ (YAFF replaces `.pixels` glyph blocks; a `.chars` sidecar reverses A4; the 254 cap applies to what a build emits).** Nothing is built. Ben asked whether gbcli has a design for pixel fonts, and pointed at GB Studio's font atlas, whose variable-width fonts he wants. It has none. This doc reads how GB Studio does it, then gives options for gbcli with a recommendation for each, and ends with questions for Ben.

> **Status, 2026-11-09 (leaf IQRDj5, the first of the three leaves): built, so "Nothing is built" above no longer holds.** The strict YAFF reader and writer (`crates/font`, tested against monobit's `4x6.yaff` and `webby-small-kerned.yaff`), `assets/fonts/NAME.yaff` as a kind, the build output (`<Name>FontGlyphs`, `<Name>FontWidths`, `<NAME>_FONT_*` and the charmap macro), the refusal past 254 with a character list as its remedy (`assets/fonts/NAME.charlist`, text holding the characters the program uses, a named glyph as `<name>`: this doc named no mechanism, so the leaf chose one), `FontAsset` in `ImagesMade.fonts` with the `assets` op's `glyphs` request, `gbcli font` with `--sample`/`--preview`/`--fit`, and `gbcli help fonts` with the layout rules (`gbcli_font::RULES`). `PROTOCOL_VERSION` is 3: a new error code and three image limits are new values of response enums (design §5's update of this date). Leaves mDCZDQ (authoring, composition) and RDlaLo (the starter renderer) remain.

**Sources, pinned:** GB Studio `eb6aa8a7` (2026-10-21) and gbvm `bd6f41cc` (2026-05-30), which is the submodule commit GB Studio pins at `eb6aa8a7` (`git ls-tree eb6aa8a7 appData/engine/gbvm`). These are the same commits as [2026-11-07-asset-pipeline-research.md](2026-11-07-asset-pipeline-research.md). Paths are relative to each repo's root. RGBDS 1.0.4 (`man 5 rgbasm`, "Character maps"). "Observed" means read in source or run in this session. "Inferred" marks a conclusion that no source states. GB Studio is MIT-licensed. This doc learns its technique and copies none of its art, fonts or code.

## The problem

Today a gbcli font is a tileset: `assets/tiles/font.png` (or `.pixels`), with one fixed 8x8 glyph per tile and no charmap (`gbcli help images`). Both example games hit the same three gaps.

- **Each game wrote its charmap by hand.** The racer wrote `examples/racer/include/charmap.inc`, with a `GLYPHS` string and a `FOR` loop of `CHARMAP` lines at `FONT_BASE + K`. The shooter wrote `examples/shooter/include/charmap.inc`, with one `CHARMAP` line per glyph at `T_FONT + n`. In both, the glyph order lives twice: in the art script and in the charmap.
- **A tileset deduplicates its tiles.** A tileset keeps "the plan's unique tiles, each from its first cell" (`crates/build/src/images/run.rs:45-50`). So two glyphs drawn alike (two blank cells, or an `O` and a `0` drawn the same) become one tile, and every later glyph's tile number moves down by one. A hand-written charmap then points at the wrong glyphs, and nothing reports it. This is inferred from the code; neither game hit it.
- **No variable width.** A glyph is a tile, so an `i` takes as much room as an `M`. Text in that style is wide and looks dated. A 160-pixel line holds 20 characters.

## How GB Studio does it

### The asset (observed unless marked)

- **Where fonts live.** A font is any PNG under `assets/fonts/` (or a plugin's `fonts/`), found by glob (`src/lib/project/loadFontData.ts:97-104`). A JSON file of the same name beside it is optional (`loadFontData.ts:24-44`). The template's README says "Place .png and .json mapping files in this folder" (`appData/templates/blank/assets/fonts/README.md`).
- **The atlas is a grid of 8x8 cells read in raster order.** `TILE_SIZE = 8`, and cells are sliced row by row (`src/lib/fonts/fontData.ts:37, 45-46, 75-95`). The template fonts `gbs-mono.png` and `gbs-var.png` are 128x112: 16 cells across and 14 rows down, so 224 cells.
- **Cell order is the character code, starting at the space.** `FIRST_CHAR = 32`. When the atlas has fewer than 16 rows of cells, 32 empty entries are put ahead of the table, so cell i is code 32+i. With 16 rows or more, cell i is code i (`fontData.ts:38, 111-113`). A 128x112 atlas therefore covers codes 32-255.
- **Width is measured from the art, by a marker color.** Each cell is trimmed of fully transparent columns on both sides (`trimIndexedImageHorizontal`, `src/shared/lib/tiles/indexedImage.ts:252-279`). The trimmed width is the glyph's width, and the glyph is moved to the cell's left edge (`fontData.ts:84-92`). "Transparent" is a color, not alpha. Alpha is ignored (`fontDataIndexFn`'s `_a`). Any pixel with green above 249, or with red and blue both above 249, counts as transparent: magenta `#ff00ff` and pure green (`fontData.ts:153-156`). In the template's variable-width font, the space between glyphs is drawn into the art in the paper color. Magenta fills only the unused right part of each cell. For example, the space glyph is 4 paper columns and 4 magenta columns, and `!` is 3 columns wide. This was observed by dumping `gbs-var.png`'s indexed pixels with a script of this session's (`target/fonts15-scripts/pngpal.py`, not committed). So a glyph's advance includes its own spacing.
  - *Inferred:* pure white `#ffffff` has green 255, so it also counts as transparent. A font drawn in white and black would lose its paper. The templates draw paper as `#e0f8cf` (green 248), just under the cutoff.
- **Shades come from the green channel.** Green below 65 is shade 3, below 130 shade 2, below 205 shade 1, and otherwise shade 0 (`fontData.ts:157-166`).
- **Mono or variable width is detected, not declared.** If any cell trims narrower than 8, the font is variable-width (`fontData.ts:86-88`). If no pixel uses shade 1 or 2, the font is "1-bit" (`fontData.ts:66-72`).
- **Identical glyphs are merged.** Cells are deduplicated by width and pixels. A recode table then maps each code to its unique glyph (`fontData.ts:97-113`, `182-185`).
- **Metadata.** The JSON can hold the following (`fontData.ts:51-64`; `loadFontData.ts:28-44`):
  - `name`;
  - `table`: a character mapped to a glyph index, which overrides the recode table for codes below 256 (`fontData.ts:115-138`);
  - `mapping`: a string mapped to one code or a list of codes. Text is encoded through it by longest match, so a key can be several characters ("euro") or an emoji (`src/shared/lib/helpers/fonts.ts:12-38`; `test/fonts/fontMapping.test.ts:78-110`).
  - Both template JSONs hold only a name and an empty mapping.
- **Characters beyond ASCII** need no mapping if they are below U+0100. An unmapped code above 127 is written as its low byte (`code & 0xff`, `fonts.ts:57-63`). *Inferred:* the atlas's codes 128-255 therefore act as Latin-1 by default, and other characters wrap silently unless they are mapped.
- **DMG and GBC.** A font has no color data. Glyphs are shades. On a GBC the text box's tiles take the palette `text_palette` through the attribute map (`gbvm src/core/ui.c:277-288`).

### What the build emits (observed)

`compileFont` writes one C file per font (`src/lib/compiler/generateGBVMData.ts:1175-1215`). It holds:

- `<font>_table[]`, the recode table: 256 bytes, or 224 for a short atlas plus its 32 leading zeros.
- `<font>_widths[]`, one byte per unique glyph, for variable-width fonts only.
- `<font>_bitmaps[]`: 16 bytes of 2bpp per unique glyph, even for a 1-bit font (`indexedImageTo2bppTileData`, `indexedImage.ts:154-173`).
- A `font_desc_t { attr, mask, recode_table, widths, bitmaps }` (`gbvm include/gbs_types.h:154-165`). `attr` holds the flags `FONT_RECODE`, `FONT_VWF` and `FONT_VWF_1BIT`. `mask` is `0x7F` when the table has at most 128 entries, and `0xFF` otherwise.

For scale, a compiled copy of the template's variable-width font that is checked into gbvm (`examples/gbs_sample_color/data/src/data/font_gbs_variable_width.c`, not regenerated here) holds 203 unique glyphs. That is 3,248 bytes of bitmaps, 203 widths and a 256-byte table: 3,707 bytes.

### How the engine draws it (observed unless marked)

- **One character at a time, into a two-tile buffer in WRAM.** `vwf_tile_data` is 32 bytes (`ui.h:62`; `src/core/absolute.c:18-19`). `ui_print_render` works as follows (`ui.c:227-275`):
  1. It looks up the glyph through the recode table (`ch & mask`), then reads its width.
  2. It shifts each of the glyph's 8 rows right by the pen's offset in the current tile, and merges them into the first buffer tile under a mask.
  3. If `offset + width > 8`, it shifts the rest left into the second buffer tile.
- **The shift and mask are hand-written assembly.** `ui_print_make_mask_lr` builds a 16-bit mask from the width and offset, so the pixels already drawn and the pixels past the glyph's width are kept, and only the glyph's own columns are replaced (`src/core/gb/ui_a.s:22-49`). `ui_print_shift_char` loads both bit planes of each row, shifts them two bits per loop pass, and merges them under the mask (`ui_a.s:88-185`). Because of the mask, the bits past a glyph's width never matter.
- **Text color.** For a 1-bit font, each plane is XORed with the fill byte (`vwf_inverse_map = text_bkg_fill`, `ui.c:231`; `ui_a.s:109-120`). So one glyph set prints dark on light or light on dark. A 2bpp font is drawn as it was drawn in the art.
- **Into VRAM.** Each character uploads the first buffer tile to the current VRAM tile (`ui_load_wram_tile`, `ui.c:182-190, 259`). When the pen passes 8 pixels, the second buffer tile moves into the first (`ui_swap_tiles`, `ui_a.s:8-20`), the next VRAM tile is taken, and the map entry is written (`ui.c:260-266, 428-432`).
- **A mono font goes through the same path, without the buffer.** Each printed character copies its glyph into a fresh VRAM tile (`ui.c:268-274`). A mono font is never loaded as a whole tileset.
- **The VRAM budget.** The text tiles are a ring from `0xCC` to `0xFF`, which is 52 tiles (`TEXT_BUFFER_START`, `TEXT_BUFFER_LEN`, `include/ui.h:13-15`). On a GBC the ring continues through bank 1 from `0xC0` (`ui.c:192-207`). The frame takes `0xC0`-`0xC8`, the white and black fills `0xC9`-`0xCA`, and the cursor `0xCB` (`ui.c:22-30`; `ui.h:29-34`). The UI therefore holds tiles `0xC0`-`0xFF`, 64 tiles.
  - *Inferred:* `ui_current_tile` is never reset between dialogues, so text on screen at once must fit in 52 tiles on a DMG, or the ring overwrites tiles still being shown.
- **The window layer.** Text is drawn into the window map, at (1,1) inside the frame (`text_render_base_addr = GetWinAddr()`, `ui.c:109, 307`). Newline, carriage return (which scrolls the text area), gotoxy and font changes are control bytes 0x01-0x0D in the string (`ui.c:317-427`). Byte 0x05 escapes the next byte, so a glyph can sit at a control code.
- **Speed.** `ui_update` draws nothing while the window is moving (`ui.c:461-462`). Otherwise it draws one character whenever `game_time & current_text_speed` is 0. The masks give one character every 1, 1, 2, 4, 8, 16, 32 or 64 frames (`ui_time_masks`, `ui_a.s:83-84`; `ui.c:466-474`). At speed 0, or while A or B fast-forwards, it draws everything in one frame. A sound can play per character (`ui.c:476`). Text can also run right to left (`ui.c:244-256`).

### In five lines

1. A font is a PNG grid of 8x8 cells, read in raster order as codes from 32 (space). An optional JSON maps strings to codes and codes to glyphs.
2. Each glyph's width is the cell trimmed of a marker color (magenta or green). Spacing is drawn into the glyph in the paper color.
3. The build merges identical glyphs and emits 2bpp bitmaps, a width per glyph and a 256-byte recode table, plus flags for variable width and 1-bit.
4. At run time each character is shifted into a two-tile WRAM buffer under a mask built from the pen offset and the width. The buffer tile is uploaded to a 52-tile VRAM ring in the window, one tile per 8 pixels of text.
5. One character is drawn every 1-64 frames, or all at once. Codes 0x01-0x0D in the string are commands (speed, font, color, position, newline).

## What gbcli can do differently

GB Studio's text reaches the ROM as ASCII or Latin-1 bytes, so it needs a recode table on the Game Boy. gbcli's text reaches the ROM through rgbasm, and rgbasm has character maps. `CHARMAP` maps any string to any byte sequence, with UTF-8 keys, keys of several characters (`"<br>"`) and several named maps (`NEWCHARMAP`, `SETCHARMAP`, `PUSHC`, `POPC`) (`man 5 rgbasm`, "Character maps"). So **the build can number the glyphs densely and emit a charmap, and `db "Hello"` assembles straight to glyph numbers.** No recode table goes in the ROM. Each glyph's data sits at its number times its size.

Observed with rgbasm 1.0.4 on a probe file in this session (`target/fonts15-scripts/charmap-probe.asm`):

- `CHARMAP "é", 2`, `CHARMAP "<HEART>", 3` and `CHARMAP "\n", 255` all assemble as written.
- An unmapped `B` under a non-empty charmap prints `warning: Unmapped character 'B' [-Wunmapped-char]` by default, then passes through as `$42`.

So a missing glyph is already caught when the program assembles, if gbcli raises that warning.

## Options

### (a) The asset

**Where fonts live.**

- **A1: a new kind, `assets/fonts/NAME.png` or `.pixels`.** *Recommended.* It follows GB Studio's folder name and gbcli's existing rule that the folder carries the kind. A font is then neither a tileset (no dedupe, order kept) nor an image with a map.
- **A2: a name suffix in `assets/tiles/`** (`font.8x8font.png`). This is rejected: it is a second naming grammar for a kind that has its own rules.

**Atlas layout and character set.**

- **A3: GB Studio's grid.** Cells are 8x8, 16 to a row (128 px wide), and cell i is the code 32+i. Rows 1-6 cover the 95 printable ASCII characters plus cell 127. An empty (all-clear) cell is a character the font lacks. *Recommended for PNG*, because it is the layout artists and other tools already know. Characters beyond ASCII need names, and a PNG has nowhere to put them. So a PNG font is ASCII-only, and a font with more characters is pixel text, below. An agent converts a PNG with `gbcli pixels --text`, losslessly, as today.
- **A4: a sidecar naming the cells** (GB Studio's JSON, or `NAME.chars`). This is rejected for now. It is a second file that can disagree with the art, and A5 covers the need.
- **A5: glyph blocks in pixel text.** *Recommended for `.pixels` fonts.* One block per character, named by the character itself, with the width given by the rows' length:

  ```
  format gbcli-pixels 1
  # assets/fonts/main.pixels: a variable-width font, 8 px tall.
  color 0 dmg 0   # paper
  color 3 dmg 3   # ink

  glyph U+0020    # the space: 3 px of paper
  000
  000
  000
  000
  000
  000
  000
  000

  glyph i
  00
  30
  00
  30
  30
  30
  30
  00

  glyph <heart>   # a name of several characters is a charmap key
  0000000
  0330330
  3333333
  3333333
  0333330
  0033300
  0003000
  0000000
  ```

  A key is one printable character, `U+XXXX` (needed for space and `#`), or a name in angle brackets, which becomes a multi-character `CHARMAP` key. Every glyph is exactly 8 rows of 1-8 columns. ASCII glyphs take their grid cells, and others follow in file order. So the file converts to and from the A3 grid; past ASCII the grid needs the file's names, so a PNG of a font with extended characters is for looking at only. The canonical writer emits glyph blocks in code order.
  - This adds a keyword to `format gbcli-pixels 1`. It is allowed only in a font, so old files are unchanged (question 3).
  - It is the agent-friendly form: each glyph has a unique header to edit by, and its width is visible.

**How widths are given.**

- **A6: measured from clear columns.** *Recommended for PNG.* A glyph starts at its cell's left edge, and its width is up to the last column holding any opaque pixel. Spacing is drawn in the paper color, as in GB Studio's art.
  - The marker is alpha 0, not magenta. gbcli keys out no color: help images says GB Studio's green is a color, and to make those pixels alpha 0.
  - A clear pixel left of the last opaque column is an error naming the cell and pixel. That catches a slip of the brush. GB Studio would instead silently narrow the glyph, or move it left.
- **A7: declared widths** (a width line per glyph). Rejected: two sources that can disagree. In A5 the row length is the width, so measuring and declaring are the same act.

**Colors.**

- **A8: two colors, paper and ink, stored 1bpp.** *Recommended for v1.* The renderer picks the shades at run time, as GB Studio's 1-bit path does with XOR. That halves the ROM, and the shifting touches one plane. Art in more than two colors is a `font_colors` problem whose remedy says to use two.
- **A9: up to four colors, 2bpp,** for outlined or shadowed text. The racer drew glyph shadows procedurally ([2026-11-08-pixel-text-design.md](2026-11-08-pixel-text-design.md), "Procedural art"). This doubles the glyph data and the renderer's merge. Park it until a game asks (question 4).
- **Shades follow the project.** Colors map to shades through the existing cascade (help images), so `dmg N` and named palettes work as in any image. A font has no GBC color tiles: on a GBC, the map entries' palette colors the text. A font in color art is refused with a remedy.

**Height.** v1 glyphs are 8 px tall, as in GB Studio. Taller fonts (8x16 cells) are parked.

### (b) What `build` emits

Fonts are images, so they go in the generated `images.inc` and `images.asm`, the one include the program already has. For a font `assets/fonts/main.png`:

```
; images.inc
DEF MAIN_FONT_GLYPHS EQU 96        ; glyph numbers 0-95, in code order
DEF MAIN_FONT_FIXED  EQU 0         ; 1 when every glyph is 8 wide
DEF MAIN_FONT_END    EQU $FF       ; the string terminator the charmap reserves
DEF MAIN_FONT_NEWLINE EQU $FE      ; "\n"
MACRO MAIN_FONT_CHARMAP            ; \1 = the charmap's name, \2 = base added to every glyph number
    PUSHC
    NEWCHARMAP \1
    CHARMAP " ", (\2) + 0
    CHARMAP "!", (\2) + 1
    ...
    CHARMAP "\n", MAIN_FONT_NEWLINE
    POPC
ENDM

; images.asm
MainFontGlyphs::  ; 8 bytes a glyph, 1bpp, row 0 first, bit 7 the leftmost pixel
MainFontWidths::  ; 1 byte a glyph, 1-8
```

- **Dense glyph numbers, no recode table.** Glyph k's data is at `MainFontGlyphs + 8k`. This is GB Studio's idea with its table moved into the assembler. Identical glyphs are not merged: a duplicate costs 8 bytes, and merging would make one number stand for two characters. For the 95 ASCII glyphs, the font is 760 + 95 = 855 bytes. The compiled copy of GB Studio's template variable-width font cited above is 3,707 bytes, though it holds 224 codes in 2bpp.
- **The charmap is a macro the program invokes.** It is not defined on include. Two reasons, both from `man 5 rgbasm`:
  - `NEWCHARMAP` switches to the new map. A charmap defined inside `images.inc` would silently re-encode every `db "…"` in each file that includes it.
  - The base differs by use. The variable-width renderer wants glyph numbers (base 0). A fixed-width font loaded as tiles at `$80` wants `$80 + k` in the tilemap.
  - So the program writes `MAIN_FONT_CHARMAP hud, $80`, then `PUSHC hud` / `db "SCORE"` / `POPC`. The racer and the shooter each wrote this by hand.
- **Codes `$FE` and `$FF` are reserved**, so a font has at most 254 glyphs. 0 is a glyph, so a fixed-width font loaded at tile 0 still maps exactly.
- **Every glyph is mapped.** gbcli passes `-Werror=unmapped-char`, or rewrites rgbasm's warning in `diagnostics/hints.rs` with a remedy: draw the glyph in `assets/fonts/main.png`. Question 6.
- **Named-tile constants** (the backlog's "Tile and frame names as program constants") are not needed for fonts: the charmap is the name. `'A'` in an expression yields A's glyph number under the active charmap (`man 5 rgbasm`, character constants).

### (c) A renderer in the `gbcli new` starter

The starter already ships a music player (`crates/build/templates/music.asm`), because every game wants music. Both example games built text from scratch. This leaf asks whether text gets the same treatment.

- **C1: no renderer.** Ship the asset and the build output only, and have `help fonts` explain the technique. This is the cheapest option, but variable width is the hard part: an agent would write shift-and-mask code from a description, and get it subtly wrong at tile boundaries.
- **C2: a line renderer** in `src/text.asm`. *Recommended.*
  - `TextDraw`: in, `hl` is a string ended by `FONT_END` (bytes from a font's charmap), `de` is the first destination tile (VRAM with the LCD off, or a WRAM buffer), and `b`/`c` are the ink and paper shades. It returns the tiles used in `a`. A line is a run of consecutive tiles, and the program writes map entries N, N+1, ….
  - `TextLoadTiles`: expands a fixed-width font's 1bpp glyphs into 2bpp tiles in two shades, for map-only text. This is what the shooter's `src/font.asm` did by hand.
  - The starter's `main.asm` draws one line with it, beside the face.
  - **Cost, estimated:** about 150-250 bytes of code, 32 bytes of WRAM (a two-tile buffer, as GB Studio's), and the font's data. Per glyph that is 8 rows of a load, up to 7 shifts and four masked stores: roughly 300-450 M-cycles, about 2-3% of a 17,556-M-cycle frame. A full 160-px line, about 30 glyphs at 5 px, takes most of a frame, which is fine with the LCD off or spread over frames. These figures are estimates, not measurements. The build leaf measures them with `gbcli run --measure`.
- **C3: C2 plus a typewriter**: `TextPutChar`, one character per call, keeping its pen in WRAM, with the VBlank handler copying the finished tile (at most 32 bytes, well inside VBlank). This is GB Studio's per-character shape. It costs about 100 more bytes and a VBlank hook. Make it a `gbcli help fonts` recipe built from C2's pieces, not starter code, until a game wants dialogue boxes (question 8).
- **What the starter needs for C2:** a default font. GB Studio's fonts can't be copied, so gbcli draws its own, original, 95-glyph `assets/fonts/main.pixels`. The face row and the song set the precedent: the starter shows each subsystem working once. This reverses the pixel text design's question 8 ("no `.pixels` in `gbcli new` for now"), whose reason was that showing an image needs starter code. A text renderer is that code.
- **The VRAM reserve.** A variable-width line's tiles belong to the program, not to an image, so the image checks can't count them. The backlog's "8x8 objects and an engine-budget reserve for images" parks a reserve until "a starter grows a font that the image budget must leave room for". C2 meets that condition. *Recommended:* the starter uses a fixed 20-tile line buffer and documents it, and the reserve setting stays parked until a check misses it (question 9).

### (d) Fixed-width fonts, and tilesets

- **A fixed-width font is a font whose glyphs are all 8 wide.** It goes in `assets/fonts/` too, and the build emits the same data (`MAIN_FONT_FIXED EQU 1`). The program loads it with `TextLoadTiles` at a base it chooses, invokes the charmap macro with that base, and writes strings straight into the tilemap. That is the racer's and the shooter's whole charmap, generated. A variable-width font can be loaded the same way, with each glyph at the left of its tile.
- **Tilesets stay as they are**, for tiles that aren't text: deduplicated, with no charmap. Help images says "a font goes in `assets/fonts/`" where it now shows `assets/tiles/font.png`.
- **Nothing migrates automatically.** The examples keep their tilesets and charmaps. A later example that wants text uses `assets/fonts/`.

### (e) Verification

- **A verb, `gbcli font FILE [--sample TEXT] [--preview PATH] [--fit PX]`**, in the build class like `song` and `pixels`. *Recommended.*
  - With no flags it prints `font path=… glyphs=95 fixed=no widths=2..8 height=8 colors=2`.
  - `--sample` sets the text in the font, on the host, by the same rules as `TextDraw`, and prints `sample width=87 tiles=11 missing=`.
  - `--preview` writes it at 4x with tile boundaries marked.
  - `--fit 144` fails (exit 1, a `problem` row) when the sample is wider than 144 px. Overflowing a text box is the usual variable-width bug.
  - The alternative is `gbcli pixels FILE --sample` for files under `assets/fonts/`. That keeps one verb, but makes `pixels` know about fonts.
- **Build-time problems**, as image problems with remedies:
  - `font_colors` (more than two colors);
  - `font_cell` (a clear pixel inside a glyph's width, with `tiles=x,y` and, for `.pixels`, `line=`);
  - `font_glyphs` (more than 254);
  - `size_multiple_8`, as for any image.
  - The `images.preview.png` of a font shows each glyph with its width marked.
- **Every character fits**, checked as `db` strings assemble (b, above). gbcli can't see the program's strings before rgbasm does; scanning `src/` for them is out of scope.
- **The renderer matches the preview.** For the build leaf: the starter's ROM is run and shot, and the line's pixels are compared with `gbcli font --sample`'s, so the asm and the host renderer can't drift.

## What it deliberately doesn't do

- **No text engine.** It has no dialogue boxes, window sliding, control codes in strings (speed, color, gotoxy), menus or scrolling. Those are GB Studio's `ui.c`, and they are a game's design. C3's recipe is the furthest it goes.
- **No kerning, no right-to-left text, and no fonts taller than 8 px** in v1.
- **No GB Studio import.** A GB Studio font PNG uses magenta and green as transparent, and that is a color to gbcli. `gbcli pixels` can show the image, and the agent turns the marker color into clear.

## Questions for Ben

1. **Fonts as their own kind in `assets/fonts/`**, PNG or `.pixels`, not deduplicated, with order kept? *Recommend yes.* A tileset can't promise glyph order, and a charmap depends on it.
2. **Atlas and widths.** For PNG, GB Studio's grid (16 cells of 8x8 a row, cell i is code 32+i, ASCII only), with widths measured to the last column holding an opaque pixel and alpha 0 as the marker? *Recommend yes.* It is familiar to artists, and the marker is the same one sprites use.
3. **Glyph blocks in pixel text** (`glyph A`, `glyph U+0020`, `glyph <heart>`, width = row length) as the form for agents, and the only one for characters past ASCII? *Recommend yes*, as a keyword added to `format gbcli-pixels 1` that is allowed only in fonts, without a version bump, since gbcli is the format's only reader and old files stay valid.
4. **Two colors (1bpp) in v1, with ink and paper chosen at run time**, and 2bpp shadowed fonts parked in the backlog? *Recommend yes.* It halves the ROM and the renderer, and no game has needed a shadowed font in data yet (the racer drew its shadows procedurally).
5. **The build output.** Dense glyph numbers, `MainFontGlyphs` and `MainFontWidths` in `images.asm`, and a charmap *macro* (name, base) in `images.inc`, with `$FE` and `$FF` reserved? *Recommend yes.* There is no recode table in ROM, and a macro avoids the silent charmap switch on include.
6. **Unmapped characters an error?** Pass `-Werror=unmapped-char` to rgbasm in every build, or keep it a warning with a gbcli remedy? *Recommend the remedy, still a warning.* An error would also hit programs that never use a font but write a charmap of their own. Turning it into an error belongs with a project setting, which gbcli avoids.
7. **A renderer in the starter (C2)**: `TextDraw` and `TextLoadTiles` in `src/text.asm`, an original default font `assets/fonts/main.pixels`, and the starter drawing one line? *Recommend yes, as its own leaf after the asset and build leaf.* Both games built text from scratch, and variable width is the part an agent gets wrong. It reverses pixel text question 8 for the reason given in (c).
8. **The typewriter (C3)**: starter code or a help recipe? *Recommend a recipe* until a game wants dialogue boxes.
9. **The VRAM reserve.** Keep the backlog's engine-budget reserve parked, with the starter documenting its 20-tile line buffer? *Recommend yes*, and record in the backlog item that a starter font now exists, so the next check that miscounts un-parks it.
10. **Verification.** A `gbcli font` verb with `--sample`, `--preview` and `--fit`, or `--sample` on `gbcli pixels`? *Recommend `gbcli font`.* `pixels` stays about images, and the verb is where `help fonts` points.

## Ben's decisions (2026-11-09)

Ben ruled after reading LoadHigh's response, [loadhigh/project/2026-11-09-pixel-fonts-response.md](../../loadhigh/project/2026-11-09-pixel-fonts-response.md), and its [font formats survey](../../loadhigh/project/2026-11-09-pixel-font-formats-survey.md). LoadHigh's asks are adopted, and Ben ruled the five points where gbcli's side differed. Where these decisions and the options above disagree, these win.

**Adopted from LoadHigh's response (asks 1-11; Ben's direction of the same day):**
- **Questions 1, 2, 4, 5, 8 and 10 stand as recommended.** Fonts are their own kind in `assets/fonts/`: not deduplicated, order kept, GB Studio's grid with alpha 0 as the width marker, 1bpp with ink and paper at run time, dense glyph numbers, a charmap macro with `$FE`/`$FF` reserved, the typewriter as a recipe, and a `gbcli font` verb with `--sample`, `--preview` and `--fit`. Question 6 stands (a warning with a remedy). Question 9 stands (the reserve stays parked).
- **One canonical source per font: a `.yaff` file, committed.** It is a strict subset of monobit's YAFF, in place of question 3's `.pixels` glyph blocks. Keys are YAFF labels: any single UTF-8 character (`'é':`), a code point (`u+00E9:`), or a name for a symbol Unicode lacks (`"heart":`). Images other than fonts keep `.pixels`.
- **PNG (with a `.chars` sidecar, one line per row of cells) and BDF are working copies for editing tools, never committed.**
  - `gbcli font edit FONT.yaff --as png|bdf` writes a copy to a gitignored folder and prints its path, with guides for PNG.
  - `gbcli font save FONT.yaff` (or the copy's own path) converts the copy back and removes it.
  - A build refuses while a working copy is newer than its `.yaff`, naming the command.
  - Every disagreement between the `.chars` file and the art is a build problem naming the cell.
- **Guide pixels:** in a font's working copy, half-transparent pixels (alpha 1-254) are guides, and the conversion drops them. Other images keep treating them as a problem.
- **Authoring:**
  - `gbcli font import FILE.bdf [--chars FILE]` places trimmed glyphs by their bounding box and the font's ascent;
  - `gbcli font export`;
  - `gbcli font new NAME --chars SET`, with guides and `--guide-sheet`;
  - `gbcli font diff A B --png OUT`.
- **The seam:** a typed `FontAsset` in the `assets` op's manifest, defined once in `gbcli_protocol`. It holds glyphs in glyph-number order, each with its key, width, 8 row bytes and whether it was drawn or composed, plus the fixed flag. LoadHigh reads fonts only through it.
- **Layout rules, stated normatively in `gbcli help fonts`,** with `gbcli font --sample` as the reference:
  - the pen advances by the width;
  - a glyph starts at its cell's left edge;
  - spacing is drawn into the glyph;
  - there is no kerning;
  - the bit order and the masking at a tile boundary are stated exactly.

**Ben's rulings on gbcli's side:**
1. **gbcli writes its own strict-subset YAFF reader and writer**, with no `libyaff` dependency. Strictness is gbcli's to enforce anyway, since a general library accepts all of YAFF. Test it against monobit's published samples.
2. **The glyph cap.** A font file may hold any number of glyphs.
   - `gbcli build` emits every glyph of a font with at most 254, since gbcli can't see which characters an asm program's strings use. Past 254 it refuses, with a remedy asking for a character list that limits what it emits.
   - The `assets` op takes the characters the caller wants and has no cap.
3. **Diacritics are composed from a mark table gbcli keeps** (Latin-1 and Latin Extended-A, about a dozen combining marks), with no Unicode-normalization dependency.
   - A missing precomposed letter is its mark laid over its base, at the base's width, and a drawn glyph overrides it.
   - An overlap is a warning naming the glyph.
   - Keys must be NFC: a decomposed key the table knows is refused, naming its precomposed form.
   - Composition happens only for requested characters: the `assets` op's request, or a build's character list.
4. **The order of work: three leaves.**
   - First, the core: the strict YAFF reader and writer, the font kind, the build output, `FontAsset`, `gbcli font` with `--sample`/`--preview`/`--fit`, and `help fonts` with the layout rules.
   - Second, authoring: edit/save with working copies, `.chars` and guides, BDF import and export, `new` and `diff`, and composition.
   - Third, the starter renderer (question 7, C2), with an original default font as `.yaff`.
   - LoadHigh can consume `FontAsset` after the first.
5. **Two text formats are accepted:** YAFF for fonts, `.pixels` for every other image. A font has labels and widths that an image doesn't, and other tools read YAFF.
