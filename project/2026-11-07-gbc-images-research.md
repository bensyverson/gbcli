# GBC images in dual mode: research

2026-11-07, leaf `k5Afkg`. This is the research behind [2026-11-07-gbc-images-plan.md](2026-11-07-gbc-images-plan.md). The goal: `gbcli build --gbc` builds a GBC-enhanced, dual-mode ROM (`rgbfix -c`, so it still runs on the DMG) from color art. Each image then needs a color version for GBC and a 4-shade version for DMG.

Two labels mark how a finding was reached. "Observed" means it was run in this session. "Inferred" means it was concluded without being read in a source.

Scratch work lived in the session scratchpad, which may not survive:
- a shallow clone of GB Studio at `eb6aa8a757203d9fdacabfc90e1e31cc5c52eb0d`, the same commit that [2026-11-07-asset-pipeline-research.md](2026-11-07-asset-pipeline-research.md) pinned;
- `gen.py`, which generates the rgbgfx test PNGs.

The experiments in §3 are small enough to restate as unit tests (plan, leaf 3).

Permalink base: `GBS = https://github.com/chrismaltby/gb-studio/blob/eb6aa8a757203d9fdacabfc90e1e31cc5c52eb0d`

## 1. How GB Studio handles color art and the DMG fallback

**Three project modes.** `ColorModeSetting` is `mono`, `mixed` or `color` (GBS/src/shared/lib/resources/types.ts#L67-L71). The UI calls them Monochrome, "Color + Monochrome" and "Color Only" (src/lang/en.json#L822-L832). The default is `mixed` (src/consts.ts#L335).
- Mixed passes `-Wm-yc`, which writes $80 to the header: dual mode.
- Color Only passes `-Wm-yC`, which writes $C0: GBC only (src/lib/compiler/buildMakeScript.ts#L147).

**Backgrounds take one of two paths: manual, or Auto Color** (compileImages.ts#L82-L86, L130-L193).

- **Manual.** The art is drawn in the 4 greens and read by green-channel thresholds: g<65→3, <130→2, <205→1, else 0 (src/shared/lib/tiles/tileData.ts#L5-L16).
  - The user paints a palette number 0–7 onto each tile in the scene editor (`tileColors`).
  - The scene picks its 8 BG palettes by hand (`paletteIds`, types.ts#L340).
  - The DMG version is the gray art itself, with a single BGP chosen per scene (`monoBGP`, compileData.ts#L457-L503).
- **Auto Color**, the default way to draw color art (`img.autoColor`, compileImages.ts#L145-L149). The DMG tile data is **derived from the color image by each color's rank within its palette**:
  - `extractTilePalette` takes the first 4 distinct colors in each tile (autoColor.ts#L215-L245).
  - `compressPalettes` merges color sets greedily while their union stays at 4 colors or fewer (L388-L433).
  - `fillVariablePalette` orders each palette by a preferred slot from `tileDataIndexFn(green)`. A color that collides with another spills into the nearest free slot, ranked by Lab lightness against anchors 100/66/33/0 (L34, L532-L631).
  - Each pixel's index is its color's slot in its tile's palette. So the same 2bpp tiles serve the DMG through BGP and the GBC through the palette (L95-L111, `buildIndexedTile` L306-L350).
  - So the DMG shades are **not** absolute lightness. A palette of four dark blues becomes white through black. Inferred: one color can take different shades in different palettes.
- **DMG override.** An optional sibling file, `name.mono.png` (src/shared/lib/assets/backgrounds.ts#L1-L2), used when Auto Color is on (compileImages.ts#L142-L151, mode `AUTO_COLOR_WITH_DMG`).
  - The tile indices come from the mono image. The colors come from the color image, per tile and per index (`autoPaletteUsingTiles`, autoColor.ts#L125-L185). `compressSparsePalettes` merges the palettes slot by slot (L479-L511).
  - The UI explains it: "…a monochrome override defined at "{tilesFilename}" which will determine the tile data used, while "{filename}" will determine the colors" (en.json#L880).
  - The docs (gbstudio.dev/docs/assets/backgrounds) say the DMG data "will be created automatically from your color image" and that it "may not perfectly match your intentions". That is why the override exists.
- **Tiles in mixed mode** use VRAM bank 0 only; `imageTileAllocationDefault` is used unless `cgbOnly` (compileImages.ts#L153-L155). BG flip dedup happens in Color Only mode only (L205-L208), since the DMG has no BG flip.

**Sprites have no color art.** Sheets are always 3 shades plus transparent, read by green thresholds (src/shared/lib/sprites/spriteData.ts#L23-L33).
- The color comes from a palette slot 0–7, chosen **by hand for each metasprite tile**, together with OBP0/OBP1 for the DMG (`MetaspriteTile.objPalette` and `paletteIndex`, types.ts#L550-L562; the editor is src/components/sprites/MetaspriteEditor.tsx#L795-L800).
- Each scene picks its 8 sprite palettes plus `monoOBP0` and `monoOBP1` (compileData.ts#L571-L630).
- The OAM attribute byte carries both: the DMG's OBP bit ($10), plus the CGB palette (bits 0–2) and VRAM bank (bit $08) (compileSprites.ts#L22-L27, L51-L66).

**Limits and how they are reported.** Every check is a warning; none fails the build.
- **Too many palettes.** More than 8 gives `WARNING_BACKGROUND_TOO_MANY_PALETTES` ("…({paletteLength} where limit is {maxPaletteLength})… Consider reducing the amount of colors", en.json#L2159, emitted at compileImages.ts#L196-L203). The warning names no tiles, and the palette index then **wraps silently** (`i % 8`, autoColor.ts#L423-L430).
- **A 5th color in a tile is dropped silently.** Extraction stops at 4 colors (autoColor.ts#L238-L240), and the pixel takes the nearest color by Manhattan RGB distance (L355-L382).
- **With a mono override,** a DMG index that covers two colors in one tile keeps the first color seen, silently (L269-L271).
- **Preview.** The UI has a "Preview as Monochrome" toggle (src/components/images/ImageViewerSettings.tsx#L82-L83).

## 2. Transparency

- **Sprites.** `spriteDataIndexFn` (spriteData.ts#L24-L25) makes a pixel transparent if any of these is true:
  - it is the key green (g>249, r<180, b<20, which covers #65ff00);
  - it is blue or magenta-ish (b≥200, g<20);
  - its alpha is below 200.

  The docs list #65ff00 as "Transparent" and don't mention alpha (gbstudio.dev/docs/assets/sprites).
- **Backgrounds** ignore alpha (`tileDataIndexFn`).
- **Color art** has no separate rule, because sprites never take color art.
- **For gbcli:** keep true alpha, and add a hint without keying anything automatically. When a sheet has no alpha-0 pixel but has pixels that match GB Studio's key, the `obj_no_transparency` warning says so: "GB Studio's transparent green #65ff00: make those pixels alpha 0".

## 3. rgbgfx 1.0.4 with palettes (observed)

Every case was run on generated RGBA PNGs.

| Case | Result |
|---|---|
| 9 disjoint 4-color tiles, no `-n` | exit 1, `FATAL: Generated 9 palettes, over the maximum of 8`. The default cap is 8. **No tile is named.** |
| Same, with `-n 9 -a -q` | exit 0. The attribute map wraps silently: tiles 0 and 8 both get palette 0. |
| 8 full palettes plus subset tiles | exit 0. The subsets are absorbed. |
| 8 full palettes plus a "bridge" tile {0,4} | exit 1, 9 palettes. **No single tile is to blame**: removing the bridge tile, or tile 0, or tile 1, fits in 8. |
| 16 two-color tiles, 32 colors | exit 0, packed into 8 palettes. |
| 17 two-color tiles, 34 colors | exit 1, 9 palettes. The lower bound ⌈34/4⌉ = 9 makes 8 impossible. |
| 5 colors in one tile | `FATAL: Tile at (0, 0) has 5 colors, more than 4` |
| Alpha 0 plus 4 opaque colors in one tile | `FATAL: … has 4 colors, more than 3`. Any transparent pixel makes slot 0 transparent in every palette (man page, PALETTE GENERATION). |
| Explicit `-c '#a,#b,#c,#d;…'`, a tile that fits no palette | `error: Failed to fit tile colors [...] in specified palettes`. It names the color set, **not where the tile is**. |
| Explicit `-c`, a pixel's index | Always the color's position in the spec, so gbcli controls the slot order. `#none` in slot 0 with alpha-0 pixels gives index 0. |
| Explicit `-c`, a tile that fits two palettes | rgbgfx picked palette 1, not the first match. An agreement check must compare pixel colors, not palette numbers. |
| `-c` with `-a`, no `-u`/`-t` | One attribute byte per tile. |

**How rgbgfx packs.** The man page calls palette generation NP-complete ("pagination"). It uses an "overload-and-remove" heuristic and gives "no guarantees" (rgbds.gbdev.io/docs/v1.0.4/rgbgfx.1, PALETTE GENERATION). `-vvvvvv` shows the steps:
1. It deduplicates each tile's set of colors, its "color set".
2. It assigns the sets in order of relative size.
3. When a palette goes past 4 colors, it removes the least efficient set.
4. It runs three "decanting" passes.
5. It sorts each palette's colors lightest first, by 2126R+7152G+722B.

`-n` caps the palette count and `-s` sets the colors per palette.

**What this means for gbcli.**
- rgbgfx can't name the tiles that force a ninth palette, and the answer isn't unique anyway. So gbcli should **pack the palettes itself, deterministically**:
  1. Take each tile's set of RGB555 colors.
  2. Deduplicate the sets, and drop any set contained in another.
  3. Pack them first-fit decreasing, with a small bounded search before giving up.
  4. Pass the palettes to rgbgfx with `-c`, in gbcli's own slot order.
  5. Check rgbgfx's output pixel by pixel, as `rgbgfx_agrees` does today.
- When the palettes don't fit, the error should state:
  - the lower bound: ⌈distinct colors / 4⌉, or /3 for sprites;
  - the tiles whose sets landed in palette 9 or later, with their colors;
  - the remedy: share colors, or move one color of a tile into a neighboring palette.

## 4. What a dual-mode build needs beyond images

- **Header.** `rgbfix -c` writes $80 at $0143 (dual mode). `-C` writes $C0 (GBC only) and overrides `-c` (man rgbfix). Today the build runs `rgbfix -v -p 0xFF` (`crates/build/src/project.rs:263`).
- **Detecting the GBC.** At entry, A = $11 on a GBC or GBA (`BOOTUP_A_CGB`, vendored hardware.inc:1128-1136; Pan Docs Power_Up_Sequence). On a GBA, bit 0 of B is also set.
  - The starter (`crates/build/templates/main.asm:19-30`) keeps A through `nop; jp Start; di; ld sp`, and first overwrites it at `ldh a,[rLY]`.
  - So `Start` must save `a == BOOTUP_A_CGB` to an HRAM flag before that read.
- **Palettes.**
  - How to load them: write BCPS (`rBGPI`, $FF68) with `BGPI_AUTOINC` (bit 7), then 64 bytes to BCPD. That is 8 palettes × 4 colors in RGB555, little-endian. OCPS/OCPD ($FF6A/B) work the same way (hardware.inc:678-694, 1195-1198).
  - When: writes are ignored in mode 3 (Pan Docs Palettes.html#lcd-color-palettes-cgb-only), so load them with the LCD off or during VBlank.
  - In CGB mode, BGP and OBP are ignored (Pan Docs: "Non-CGB Mode only"). On a DMG the CGB registers are unmapped, so writing them is harmless (inferred).
- **Attribute map.**
  - With VBK ($FF4F) set to 1, write to $9800. Each byte holds the palette (bits 0–2), the VRAM bank (bit 3), X-flip (bit 5), Y-flip (bit 6) and priority (bit 7) (hardware.inc:1047-1056; Pan Docs Tile_Maps.html, BG map attributes).
  - **These writes must run only on a GBC.** A DMG has no VBK, so the attribute bytes would overwrite the tilemap (inferred).
  - Set VBK back to 0 afterward.
- **Objects.** The OAM attribute byte carries both machines' bits: the DMG's OBP is bit 4; the CGB's palette is bits 0–2 and its bank is bit 3 (hardware.inc:1074-1083). Each machine ignores the other's bits, so one frame list serves both.
- **Double speed** isn't needed, since a dual ROM must fit the DMG's timing anyway. HDMA isn't needed either.
- **Sessions today.**
  - The default model is `Model::DmgB` (`crates/bundle/src/session.rs:33,149`). A dual ROM runs as a DMG unless `--model` says otherwise.
  - `model_from_name` (`crates/protocol/src/checkpoint.rs:209-223`) maps `cgb` to **Cgb0**, an early revision, and `cgb-e` to CgbE. There is no `gbc` alias.
  - A session keeps its model: restore refuses any other (`ModelConflict`, `crates/engine/src/machine.rs:287-291`). Running a ROM as both GBC and DMG takes two sessions.
  - SameBoy reports CGB mode (`ColorMode::CgbPalettes` vs `CgbDmgCompat`, `crates/sameboy/src/display.rs:32-45`), and the views already draw CGB palettes from palette RAM (`crates/view/src/colors.rs`).
- **Screenshots.**
  - CGB frames are `0x00RRGGBB`.
  - gbcli never calls `GB_set_color_correction_mode`, and `GB_init` zeroes the struct (SameBoy Core/gb.c:166). So correction is `DISABLED`, and each channel is scaled `(x<<3)|(x>>2)` (display.c:346-349, 373-376).
  - A GBC preview can therefore match a screenshot exactly by using the same scaling. This was read in the code, not run.
