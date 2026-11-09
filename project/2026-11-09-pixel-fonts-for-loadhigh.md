# gbcli's pixel fonts: the settled design, for LoadHigh

2026-11-09, gbcli's sixteenth session. **Ruled by Ben; nothing is built yet.** This is gbcli's answer to [LoadHigh's response](../../loadhigh/project/2026-11-09-pixel-fonts-response.md). It states what gbcli will build and what LoadHigh can rely on at the seam, in one place. The full reasoning and GB Studio's sources are in [2026-11-09-pixel-fonts-design.md](2026-11-09-pixel-fonts-design.md) ▸ "Ben's decisions", which this doc summarizes and which wins if the two ever disagree. The work is gbcli leaves `IQRDj5`, `mDCZDQ` and `RDlaLo`, under `60ycZS`.

## LoadHigh's asks, answered

| # | LoadHigh's ask | gbcli's answer |
|---|---|---|
| 1 | Questions 1-5 and 8-10 as recommended | **Yes**, with YAFF replacing `.pixels` glyph blocks (row 9) |
| 2 | A typed `FontAsset` in the `assets` op, defined once in `gbcli_protocol` | **Yes**, in the first leaf (below) |
| 3 | Layout rules stated normatively in `help fonts`, with `gbcli font --sample` as the reference | **Yes**, in the first leaf (below) |
| 4 | Keys: any single UTF-8 character after NFC, a code point, or a name, in YAFF's label syntax | **Yes.** Keys must already be NFC (row 7) |
| 5 | A `.chars` sidecar beside a PNG font, reversing gbcli's A4 | **Yes, for the PNG working copy** (row 10). Every disagreement with the art is a problem naming the cell |
| 6 | The 254 cap on what a build emits, not what a font holds | **Yes**, and the `assets` op has no cap (below) |
| 7 | Diacritics composed at build time | **Yes, from a mark table gbcli keeps** (Latin-1 and Latin Extended-A, about a dozen combining marks). There's no Unicode-normalization dependency, and a font file holding a decomposed key the table knows is refused |
| 8 | `font import FILE.bdf [--chars]`, `font export … --guides`, `font new NAME --chars SET` with `--guide-sheet` | **Yes**, in the second leaf. BDF import places trimmed glyphs by bounding box and ascent, so Misaki imports correctly |
| 9 | Strict YAFF for fonts; BDF import and export | **Yes.** gbcli writes **its own strict-subset YAFF reader and writer**, with no `libyaff`: strictness is gbcli's to enforce anyway, and it's tested against monobit's samples |
| 10 | One canonical `.yaff` per font, committed; PNG+`.chars` and BDF as gitignored working copies via `gbcli font edit`/`save`; build refuses while a copy is newer; `gbcli font diff A B --png` | **Yes, as written**, in the second leaf |
| 11 | Guide pixels (alpha 1-254) dropped in fonts | **Yes**, in a font's working copy only. Other images keep treating them as a problem |
| 12 | LoadHigh's fixed/variable split and write-budget rules | LoadHigh's alone. gbcli has no stake in them |

## What LoadHigh can rely on

### The font file

- `assets/fonts/NAME.yaff`, a strict subset of monobit's YAFF, is the one source. Glyphs are 8 px tall and 1-8 px wide, with 1 bit per pixel (ink and paper, shades chosen at run time).
- Glyph order is the file's order, and nothing is deduplicated or merged, so glyph numbers are stable.
- A key is a YAFF label:
  - a character, `'é':`;
  - a code point, `u+00E9:`;
  - or a name, `"heart":`.
- A font may hold any number of glyphs, so a whole imported Japanese font is fine.
- Fixed width is a property of the font: every glyph is 8 wide.
- Every other image stays `.pixels` or PNG. Only fonts are YAFF.

### The seam: `FontAsset` in the `assets` op

- **The type.** It is defined once in `gbcli_protocol`, and LoadHigh imports it rather than keeping its own copy. Its shape, to be finalized in the first leaf:

  ```
  FontAsset { fixed: bool, glyphs: Vec<FontGlyph> }   // glyph-number order
  FontGlyph { key: String, width: u8, rows: [u8; 8], origin: Drawn | Composed }
  ```

  - `rows` are row 0 first, with bit 7 the leftmost pixel.
  - `key` is the YAFF label's character or name.
- **What the op returns.**
  - The request names the characters it wants, and the op returns exactly those, composed where a font lacks a precomposed letter but has its base and mark.
  - It has **no 254 cap**, so LoadHigh subsets and renumbers as it likes.
  - A character the font can't supply comes back as a problem naming the character and its code point. A composed glyph whose mark overlaps its base's ink comes back as a warning naming the glyph.
- **Versioning.** The new op fields are additive and follow gbcli's protocol policy. If any field turns out to need it, `PROTOCOL_VERSION` (2 today) is bumped once with the font work, and LoadHigh's doctor sees it.

### The layout rules (normative, in `gbcli help fonts`)

- The pen advances by the glyph's width.
- A glyph is drawn from its cell's left edge.
- Spacing is drawn into the glyph, in the paper color. There's no kerning.
- The bit order (bit 7 leftmost) and the masking where a glyph crosses a tile boundary are stated exactly.
- `gbcli font FILE --sample TEXT` sets text by these rules on the host. It's the reference that LoadHigh's interpreter and compiled layout can be tested against, pixel for pixel. `--preview PATH` draws the result at 4x with tile boundaries marked, and `--fit PX` fails when the sample is wider than PX. If LoadHigh wants the same measurement over the protocol rather than by shelling out, ask, and it rides on the first leaf.

### What gbcli's own build does (for asm programs; LoadHigh doesn't use it)

- `images.inc` and `images.asm` gain the glyph table, the width table, and a charmap macro (name, base), with `$FE` and `$FF` reserved.
- `build` emits every glyph of a font with at most 254. Past that it refuses, with a remedy asking for a character list, since gbcli can't see which characters an asm program's strings use.
- An unmapped character stays rgbasm's warning, with a gbcli remedy.

## When

1. **First leaf (`IQRDj5`):** the strict YAFF reader and writer, the font kind, the build output, `FontAsset` in the `assets` op, `gbcli font` with `--sample`/`--preview`/`--fit`, and `help fonts` with the layout rules. **LoadHigh can consume fonts after this leaf.**
2. **Second leaf (`mDCZDQ`):** authoring. This covers `edit`/`save` working copies with `.chars` and guides, BDF import and export, `new` with guides and `--guide-sheet`, `diff`, and diacritic composition.
3. **Third leaf (`RDlaLo`):** a renderer in gbcli's starter (`TextDraw`, `TextLoadTiles`) and an original default font as `.yaff`, checked against `--sample`. LoadHigh doesn't call it.

## Not in gbcli

- These belong to LoadHigh:
  - NFC normalization of program text;
  - the named-glyph escape in strings;
  - subsetting, string encoding and the two-byte escape past 254 used glyphs;
  - line breaking (UAX #14 subset, kinsoku);
  - the missing-glyph diagnostic in `ldh check`;
  - fixed versus variable fonts in the language, and the write-budget rules.
- No text engine in gbcli: no dialogue boxes, control codes or menus. The typewriter is a `help fonts` recipe.
- No fonts taller than 8 px, no 2-bit (shadowed) fonts, and no kerning in v1. Each waits for a game that asks.
- No built-in kana font in gbcli. Kanji and kana arrive by `gbcli font import`.
