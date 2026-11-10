# The image encoder: rgbgfx retired from the build (2026-11-09)

**Decision (Ben, 2026-11-09).** A build and `assets` encode images
themselves, in Rust. rgbgfx 1.0.4 stays as a test oracle only: `gbcli
doctor` no longer looks for it, and `assets` needs no toolchain at all.
rgbgfx was a dead end for a web build — nobody ships it as wasm
([2026-11-09-web-spike-rgbds.md](2026-11-09-web-spike-rgbds.md)) — and on
native it was the only tool a build needed that gbcli could replace with a
function of data it already had.

## What gbcli already decided

Every byte rgbgfx wrote was decided before it ran: each pixel's color index
(its shade), each cell's unique tile (`images/bg.rs`, `images/sheet.rs`),
and on the GBC each cell's palette (`images/pack.rs`). gbcli called rgbgfx
with explicit palettes (`-c`), output (`-o`), the GBC attribute map (`-a`)
and column-major sprites (`-Z`) — never its dedup, its `-u`, or its palette
choice — then checked what it wrote pixel by pixel (`images/agree.rs`).
That was an encoder plus a verification of the encoder.

## What replaced it

`crates/build/src/images/encode.rs` writes the 2bpp tile bytes (each row's
low plane then its high, left pixel in bit 7), a background's unique tiles
in plan order and its tilemap. `crates/build/src/images/gbc.rs` indexes
each color tile into the palette its cell was packed into, for a
background's unique tiles and a sheet's kept units. `images/output.rs`
writes them, and a color background's attribute map from its tile map's
slots. Nothing opens a file to convert, and nothing runs a process: the
plan goes in, the Game Boy's data comes out.

Retired with the tool: `BgPlan`/`SheetPlan`'s `spec` and `merges` (rgbgfx's
`-c` and the merged PNG it read), `images/run.rs`, `ImageTool`, `Toolchain`'s
rgbgfx field, `BuildError::Toolchain`, `AssetsOptions::tool_timeout`, the
`images/agree.rs` comparison, and doctor's `rgbgfx` row. `ImageLimit::RgbgfxAgrees`
and `Tool::Rgbgfx` remain in the protocol types, which this leaf did not
change; nothing produces them.

> **Update (2026-11-09, leaf HNBr9t).** Ben ruled that the unused values
> go: `Tool::Rgbgfx`, `CheckName::Rgbgfx` and `ImageLimit::RgbgfxAgrees`
> are removed from the protocol, so `"rgbgfx"` no longer reads as a
> diagnostic's tool or a doctor check, nor `"rgbgfx_agrees"` as a limit.
> `AssetsMade.diagnostics` stays on the wire, always empty.

## The differential test

> **Correction (2026-11-09, leaf HNBr9t): the test below, as first
> written, did not run rgbgfx "on the same image".** Its DMG half painted
> rgbgfx's input from gbcli's own `.2bpp` decoded through gbcli's own tile
> map, so it was a 2bpp round trip: reversing a background's tile order,
> reversing a sheet's unit order, and swapping the planes in
> `encode::background` all left it green (the independent review of
> c47f60d; other image tests caught each). The plane-swap line below
> credited the DMG check with a catch it could not make. Its GBC half
> repainted the source at 8 bits, after truncating 16-bit samples, so a
> truncating decoder passed it too. Now:
>
> - **DMG** reads the source's own pixels (its `name.dmg.png` when it has
>   one), each color repainted as the one `-c` lists for the index gbcli
>   gave it, and each cell's tile must be rgbgfx's tile at that place.
>   Tile and unit reversal turn it red (checked by applying each). It still
>   takes gbcli's color → index choice, so a uniform relabeling of indices
>   (the plane swap is one: 1 ↔ 2) passes the comparison. Where the
>   project's shade rule is a known palette, a background's or tileset's
>   index must also be its color's shade there (BGP $E4); a pure-gray
>   fixture (`gray_art_matches_rgbgfx_on_the_source_and_in_its_shades`)
>   makes that apply, and the plane swap fails it.
> - **GBC** hands rgbgfx the source PNG itself, so rgbgfx decodes it
>   (16-bit samples included); a 16-bit fixture whose samples sit just
>   under an RGB555 step fails when gbcli truncates instead of rounding
>   (checked), and `images_png_encodings.rs` pins the same colors without
>   rgbgfx.
>
> **What the oracle cannot see.** An image whose source isn't a PNG
> (`.pixels` text: 11 images in 3 of the 5 example games): its DMG data is
> not compared, and its GBC data is compared with a PNG painted from
> gbcli's own data, a round trip. A sheet's indices are not checked
> against its OBP's shades, and a background's only under a known palette;
> elsewhere gbcli's shade choice is taken, not checked. `assets` is not run
> with a recording rgbgfx; it takes no toolchain and the protocol has no
> rgbgfx tool to find.

`crates/build/tests/it/images_differential.rs` converts every example
game's images and a fixture of each kind and art with `assets` (no tool),
then runs real rgbgfx 1.0.4 on the same image with the same palettes:

- **DMG** — the tile data must be byte for byte what rgbgfx wrote. The
  image rgbgfx reads is rebuilt from gbcli's data with one color per index
  (the source PNG's colors, or four grays for a derived version), since the
  shade assignment is gbcli's unchanged decision; a broken 2bpp encoding
  fails the comparison (checked by swapping the planes).
- **GBC** — every pixel's color through gbcli's slot must equal the color
  rgbgfx drew through its own slot, which rgbgfx may choose per 8x8 tile
  (research §3). A color background's `.attrmap` is also checked against
  its tile map's slots.
- **No call** — a build with an rgbgfx on `PATH` that records being run and
  fails still builds, and the record is absent.

**Coverage limit.** The image fixtures the integration suite builds live in
their own temporary directories and are gone before another test runs, and
the crate exposes no image-plan API, so the differential test builds the
same shapes itself rather than reaching into those tests. Every one of them
is additionally built with no rgbgfx on `PATH` by its own test, so a
regression that made the build call rgbgfx would fail there. If the plan
ever becomes a public library type (the web target's "data in, data out"),
the differential test can walk the plans directly and drop that limit.

## What this does not change

Image decisions are untouched: shades, palette packing, dedup, limits,
warnings and remedies. A user-facing message that names rgbgfx's color model
(RGB555, five bits a channel) stays true — the encoder reads colors the same
way; a message that named a failed rgbgfx run is gone with the run.
