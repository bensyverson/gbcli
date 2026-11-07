# Asset pipeline research: GB Studio, rgbgfx, hUGEDriver and .uge

2026-11-07, leaf O5swhd. Answers the questions in [2026-11-07-asset-pipeline-plan.md](2026-11-07-asset-pipeline-plan.md); fixes the image leaf's checks (WdnjHI) and recommends un-parking music authoring now.

**Sources, pinned:** GB Studio `eb6aa8a7` (2026-10-21) with its gbvm submodule at `bd6f41cc`; hUGEDriver `d6863d29` (2026-08-25); hUGETracker `3fedced3` (2026-08-15); RGBDS 1.0.4 (`man rgbgfx`); Pan Docs `src/{OAM,Tile_Data,Palettes,Tile_Maps}.md` on master, fetched 2026-11-07. "Observed" means run in this research session; "inferred" marks a conclusion not read in a source.

## 1. GB Studio's limits and wording

Every check below is a **warning**; none stops a build.

| Check | Limit | Kind | Source |
|---|---|---|---|
| Unique background tiles | 192 (DMG/color) | engine | `MAX_BACKGROUND_TILES = 16*12`, src/consts.ts:62; compileImages.ts:288 |
| Unique background tiles, CGB-only | 384 | engine | `MAX_BACKGROUND_TILES_CGB`, consts.ts:63 |
| Auto-color palettes | 8 | hardware | compileImages.ts:196 |
| Width/height | multiple of 8; 160×144 to 2040 px; ≤16,380 tiles | hardware (multiple of 8) / engine (rest) | compileImages.ts:50–52, 254–285 |
| Sprite tiles per scene | 64–96 8×16 units, doubled in 8×8 mode, shrinking as background tiles pass 128 | engine | `maxSpriteTilesForBackgroundTilesLength`, src/shared/lib/helpers/sprites.ts |
| Actors | 20 per scene, 10 if single-screen | engine | consts.ts:52–53 |
| Actors on screen | 10 within any screen-sized window | engine heuristic | SceneInfo.tsx:403–425 |
| Projectiles | 5 unique per scene | engine | consts.ts:56 |

**192 is an engine budget, not hardware.** gbvm puts its UI and font tiles at `0xC4`–`0xCB` (gbvm include/ui.h:29–34), so reserving 192–255 for UI is inferred, strongly. The hardware has 384 tiles of VRAM (768 on CGB), shared with sprites.

**Wording worth borrowing** (src/lang/en.json:2135–2174): count, limit, consequence, remedy.
- "Background contains too many unique 8x8px tiles ({tilesetLength} where limit is {maxTilesetLength}) meaning it may not display correctly. Consider reducing the amount of detail in this image."
- "Background contains too many unique four color palettes ({paletteLength} where limit is {maxPaletteLength}) … Consider reducing the amount of colors used in this image."
- "Background width or height is not a multiple of 8px"
- "This scene is over available limits and may have rendering issues. Try reducing number of actors in scene or simplify their sprites to use less unique tiles"
- Tooltip: "A maximum of 40 sprite tiles can be displayed on screen at once, and a maximum of 10 tiles can be displayed on the same horizontal row." (en.json:576)

**Not checked:** the 10-objects-per-line limit (tooltip only); 8×16 pairing, avoided by construction (`optimiseTiles` slices 8×16 cells, deduplicates whole units with flips, assigns tile `ui*2`; src/lib/sprites/readSpriteData.ts:55–140); `WARNING_MUSIC_TOO_LARGE` exists in en.json but nothing emits it; bank budgets come from GBDK `-autobank` and a `romusage` report (buildMakeScript.ts:132; romUsage.ts), not warnings.

## 2. rgbgfx 1.0.4

**Flags:** dedup `-u`; flips `-X`, `-Y`, `-m` (= `-XY`, implies `-u`); tilemap `-t`/`-T`, attribute map `-a`/`-A`, palettes `-p`/`-P`, palette map `-q`; palette input `-c` (`dmg`, `dmg=E4`, `embedded`, inline `#…`, `gbc:file`); limits `-N a,b` (tiles per VRAM bank; fatal past the total) and `-n` (palette cap); `-b` base tile IDs, `-i` shared input tileset, `-L` slice a sheet; `-B` background color to omit, `-Z` column-major order, `-r` reverse (data back to PNG). **No 8×16 mode**: `-Z` is the only way to get top and bottom tiles adjacent.

**At a limit** (each observed on a generated PNG):

| Case | Result |
|---|---|
| 360 unique tiles, `-u -t` | exit 0, `warning: Image contains 360 tiles, of which only 256 are representable in the tilemap` (IDs wrap) |
| Same, `-N 256` | exit 1, `FATAL: Image contains 360 tiles, exceeding the limit of 256 + 0` |
| Same, `-N 256,256 -a` | exit 0 (bank bit in the attribute map) |
| 5 colors in one tile | exit 1, `FATAL: Tile at (0, 0) has 5 colors, more than 4` |
| 9 palettes needed | exit 1, `FATAL: Generated 9 palettes, over the maximum of 8` (default cap 8) |
| Color image with `-c dmg` | exit 1, `FATAL: Image is not compatible with a DMG palette specification: it contains too many colors (26)` |
| Width 12 | exit 1, `FATAL: Image width (12 pixels) is not a multiple of 8` |
| **Two 4-color palettes, no `-c`** | **exit 0**: wrong on DMG, which has one BGP |
| **8×16 sheet, `-Z -u`** | **exit 0, tilemap `00 01 02 00`**: the second sprite needs pair (2,0), which hardware can't show (top = index&~1, bottom = index\|1; Pan Docs OAM.md:44–47) |

**Left for gbcli:** know the target (DMG vs CGB, background vs sprite), which rgbgfx doesn't; turn the silent cases (DMG palette, 8×16 pairing, IDs wrapping past 256) into errors; budget background tiles against sprite tiles and other images in the same VRAM; check objects per line and in total; flag sprite sheets with no transparent color (index 0 is transparent on hardware); say which tiles cost the most, with remedies; render the preview.

## 3. hUGEDriver, hUGETracker and .uge

**License:** "hUGETracker and hUGEDriver are dedicated to the public domain" (both READMEs; neither repo has a LICENSE file). gbvm (MIT) already vendors the driver under third-party/HUGE_TRACKER. **Vendorable into `new`: yes**; record the dedication and the commit.

**Format:** `.uge` is a raw little-endian dump of a FreePascal packed record (hUGETracker src/song.pas, hugedatatypes.pas), versions 0–6, `UGE_FORMAT_VERSION = 6` (constants.pas:21); readers upgrade older versions on load, and GB Studio's TypeScript `loadUGESong` handles v1–v6 (src/shared/lib/uge/ugeHelper.ts:49–255).

v6 layout in file order, verified by a parser that consumes GB Studio's template and three v6 sample songs to the last byte:

| Part | Size |
|---|---|
| Version | u32 |
| Name, artist, comment | 3 × 256-byte ShortString |
| Instruments | 45 × 1,381 B (15 duty, 15 wave, 15 noise), each with a 64-row subpattern |
| Waves | 16 × 32 B |
| Ticks per row | u32 |
| Timer enabled, timer divider | u8, u32 |
| Patterns | count, then per pattern a u32 key + 64 cells × 17 B (note, instrument, unused volume, effect as i32 each, plus a parameter byte) |
| Orders | 4 channels × (u32 n, n × u32); n is one more than the real count |
| Routines | 16 AnsiStrings (u32 length + bytes) |

Size: **63,438 fixed bytes + 1,092 per pattern**; the template is 68,102 bytes.

**Lossless text conversion: tractable, about one crate.** A semantic round trip is easy; byte-exact is not, because ShortString tails keep leftover bytes from earlier, longer names (observed: 14, 13 and 4 non-zero tails in the three samples), and the dead volume field and the extra order entry round-trip only through the bytes. Test: text → .uge → text is a fixpoint, and bytes match except the string tails.

**Existing text forms:** hUGETracker's pattern clipboard uses the ModPlug/OpenMPT XM paste format (`|C-501...2FF` per channel; clipboardutils.pas:252–300; GB Studio clipboard.ts), a good precedent for pattern rows; nothing covers instruments, waves or orders. The RGBDS export (`dn C_5,1,$2FF` rows, raw register bytes per instrument) is assembler-checked but lossy (names gone, duplicate patterns merged, instruments as register bytes) and can't go back to .uge.

**Song to driver data:** hUGETracker exports `.asm` (`RenderSongToRGBDSAsm`, codegen.pas:394–584) and ships a headless `uge2source infile.uge [-bBANK] descriptor out.asm|.c` (src/uge2source), which needs FPC/Lazarus to build, so it isn't on a typical PATH. GB Studio never uses it: its `exportToC` writes GBDK C against gbvm's bundled driver (src/lib/compiler/compileMusic.ts), and it imports `.mod` through mod2uge.

**Driver cost** (not documented; measured): assembled with rgbasm/rgblink 1.0.4 from the repo's `rgbds_example`, the "Sound Driver" section is 1,936 B in **ROM0**, WRAM 100 B, the sample song's data 9,956 B (ROMX), 192 B per pattern (64 rows × 3). `gbcli measure hUGE_dosound..isr_wrapper+7 --iterations 600` on DMG: 2,552–5,852 T per call, mean 3,260 T (3.6–8.3% of a frame, mean 4.6%); it depends on the song. A song with `TimerEnabled` must be driven from the timer interrupt, not vblank. (Measuring it needed the `.sym` fix below.)

**gbcli bug found here:** the session `.sym` loader refused rgblink 1.0.4's exported-constant lines (`64 hUGE_NO_WAVE`, from hUGEDriver.asm:69–70) with `symbols_unreadable … line 354: expected 'BB:AAAA Name'`. Fixed 2026-11-07 (`crates/engine/src/sym.rs`: constants are skipped; test `an_exported_constant_line_is_not_an_address_and_is_skipped`).

## 4. Other things worth borrowing from GB Studio

- **Deduplicate 8×16 sprites as whole units, flips included** (readSpriteData.ts): the fix for rgbgfx's pairing hole.
- **A live budget readout**, "S: {tileCount}/{maxSpriteTiles}" (SceneInfo.tsx:541), warning at equal and erroring over; gbcli's equivalent is `used/limit` in every limit row.
- **The wording pattern**: count, limit, consequence, remedy.
- **Bank packing**: nothing to borrow; rgblink places floating ROMX sections and its map prints "ROMX: N bytes used / M free", which gbcli can surface.
- **Batteryless saves** (for leaf L00RuF, already planned; the backend evaluation records the program sequence). From gbvm src/core/gb/flasher_s.s and flasher.c: sector erase is `$4000←$F0`, `$0AAA←$A9, $0555←$56, $0AAA←$80, $0AAA←$A9, $0555←$56, $4000←$30`, then poll until the byte reads `$FF`; sectors are 64 KB (4 banks), two 8 KB SRAM banks stored per ROM bank from the bank `_start_save`, up to 4 SRAM banks; byte program is `$0AAA←$A9, $0555←$56, $0AAA←$A0`, the byte, then read-back polling with a 256-try timeout. The command addresses fall in the MBC's RAM-enable range (why the routine disables SRAM around them), so the model must route `$0000–$7FFF` writes to both the MBC and the flash chip.

## Recommendations

### (a) The image leaf (WdnjHI)

**Conversion by convention:** `assets/bg/*.png` → `rgbgfx -u -t -c dmg` on DMG; on CGB add `-m -a -p -N 256,256` (flips, attribute map, palettes, both banks). `assets/sprites/*.png` → `rgbgfx -Z` with no `-u`/`-m` in 8×16 mode; gbcli deduplicates whole 8×16 units with flips, as GB Studio does. Always pass `-N`, so overflow is fatal rather than a warning.

**Checks**, each a structured error with limit, count, offending tiles or coordinates, and a remedy. Hardware limits only; engine budgets become a configurable reserve.

| id | Limit (source) | Example message |
|---|---|---|
| `bg_tiles_map` | 256 tiles addressable per map in one LCDC mode (Tile_Maps.md, Tile_Data.md) | "assets/bg/town.png needs 291 unique tiles after dedup; one map addresses 256; 35 tiles occur once (listed). Remedy: simplify those tiles, enable flip dedup on CGB, or split the image" |
| `vram_tiles` | 384 tiles in DMG VRAM shared by background and sprites; 768 on CGB (Tile_Data.md:4,14) | "backgrounds use 300 + sprites 120 = 420 of 384 VRAM tiles" |
| `dmg_palette` | one 4-shade BGP for the whole background; two 3-color OBPs | "town.png uses 2 four-color palettes; DMG has one background palette. Remedy: reduce to 4 grays (rgbgfx -c dmg)" |
| `tile_colors` | 4 per tile (3 + transparent for sprites) | rgbgfx's "Tile at (x, y) has 5 colors", with a remedy |
| `cgb_palettes` | 8 BG + 8 OBJ palettes of 4 (Palettes.md) | rgbgfx's "Generated 9 palettes", adding which tiles force the 9th |
| `size_multiple_8` | — | pass through |
| `obj_pair_8x16` | top tile = index&~1 (OAM.md:44–47) | fires only on a user-supplied tilemap or `-u` |
| `obj_no_transparency` | color 0 is transparent (Palettes.md:30) | "sprite sheet has no transparent color; index 0 (#ffffff) will not draw" |
| `obj_per_line` / `obj_total` | 10 per line / 40 total (OAM.md:4–5) | from metasprite width, or at runtime from OAM through the existing views: "frame 120, line 64: 12 objects; 2 will not draw" |

**Preview:** render the converted data, not the source PNG, through `crates/view` (tileset, tilemap, palettes, and a contact sheet of deduplicated tiles by use count); `rgbgfx -r` can cross-check it.

### (b) Music: un-park now (a decision for the user)

The backlog's un-park conditions are met or nearly: the driver is vendorable, `.uge` is fully specified by two readable implementations, and verification exists (`--audio-log`, `--piano`, per-channel `--wav`, `equiv` on sound-register writes). Proposed shape:

1. **A `gbcli-music` crate:** read `.uge` v1–v6 (port GB Studio's loader), write v6; a canonical UTF-8 text form — song header, named instruments as fields rather than register bytes (each with its subpattern), waves as 32 hex nibbles, orders per channel, patterns as 64 rows of `C-5 01 ... 2FF` cells (the XM paste convention). Test: text → .uge → text is a fixpoint on hUGETracker's sample songs (public domain, committable).
2. **Our own `.uge`/text → RGBDS `.asm` emitter**, a port of `RenderSongToRGBDSAsm`; `build` converts `assets/music/*.uge` (or text) by convention. Pin it against hUGETracker's export for the samples, as goldens or by `equiv` on register writes.
3. **`new` vendors hUGEDriver** at a pinned commit with a player stub calling `hUGE_dosound`, and its cost in the docs (about 2 KB ROM0, 100 B WRAM, about 3–6k T per call).
4. **Verification recipe:** build, then `run --frames N --audio-log --piano`.

Out of scope: a MOD importer, GBDK C output, routines (custom code) beyond round-tripping their text.

## Corrections to the plan's premises

- rgbgfx has no 8×16 mode; `-Z` with `-u`/`-m` silently breaks pairs.
- DMG has no per-tile palettes: one BGP for the background, two OBPs. The plan's "colors per tile (4 on DMG)" missed the real DMG limit, 4 shades across the whole background.
- 384 tiles is the background **plus** sprite pool, not a background limit; CGB backgrounds address 512 through the bank bit, and 256 per map can be exceeded by switching LCDC mid-frame (GB Studio's 360-tile logo mode).
- GB Studio checks neither 8×16 pairing, VRAM nor bank budgets as such, and all its checks are warnings.
- `.uge` is a raw Pascal memory dump (at least 63 KB, string-tail residue, an off-by-one in order counts): tractable, not designed.
- hUGETracker's export is not GUI-only: `uge2source` is headless, but needs FPC to build.
