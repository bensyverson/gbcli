//! `gbcli help images`: the image convention `build` follows, the limits it
//! checks and what the program gets, on the DMG and, under `build --gbc`,
//! the GBC.

pub const IMAGES: &str = "\
Images — PNGs under assets/, converted by `gbcli build` with rgbgfx (RGBDS).

  assets/bg/town.png           a background: unique tiles + a tilemap
  assets/sprites/hero.png      8x16 objects; the whole sheet is one frame
  assets/sprites/hero.16x16.png  frames of 16x16, left to right, then down
  assets/tiles/font.png        a tileset: a background's unique tiles, no
                               map (build your own from --json's tile_map)
Draw each image in up to 4 colors (sprites: 3 + transparent, alpha 0).
Shades 0-3 (white to black) are chosen once for the whole project, so a
color is one shade in every image, by the first rule that applies:
  1. declared   the dmg line of assets/palettes.txt (below): the colors of
                shades 0-3; each color takes its nearest entry's shade
  2. known      every color is in one palette: the DMG grays (#ffffff
                #aaaaaa #555555 #000000), the DMG-01 pea greens (#9bbc0f
                #8bac0f #306230 #0f380f), GB Studio's greens (#e0f8cf
                #86c06c #306850 #071821) or the Pocket grays as SameBoy draws
                them (#c2ce93 #818d66 #3a4c3a #07100e)
  3. stretched  lightness rescaled so the project's lightest color is shade
                0 and its darkest shade 3, then the nearest shade (nearest:
                one lightness only, so nothing to stretch)
The images row says which: shades=declared|known|stretched|nearest, and
palette= for a known one. Colors in one shade draw alike, with a
shared_shade warning: one per image for colors that image holds together,
and one for the project (no image=) for colors that share it only across
images: colors one image holds, or chained through one, are a group (a/b);
it lists each group with its images, and count= is the groups. Colors
equal in RGB555 (5 bits a channel, as rgbgfx reads them) are one color.
gbcli keys no color out: GB Studio's transparent green (#65ff00) is a
color here; make those pixels alpha 0.

assets/palettes.txt names palettes, one a line, 4 colors, index 0 (the
DMG's lightest) first, and assigns them to images by path under assets/:
  dmg:    #e0f8d0 #88c070 #346856 #081820   # reserved: the shades above
  forest: #fff #8c4 #363 #021               # #rgb is #rrggbb (#88cc44)
  ember:  #ffe #f80 #a20 #200
  bg/town.png = forest
Colors are #rrggbb or #rgb, kept as RGB555. Names are a lowercase letter,
then lowercase letters, digits or _. '#' then a space or tab (or '#' last
on a line) starts a comment; any other '#' word is a color, so a fifth
color is an error. A build uses the dmg line, checks the rest, emits every
palette as data, and builds an assigned image as DMG art; build --gbc
colors it too (GBC, below).

Every hardware limit is a `problem` row: limit, count, max, tiles=x,y (in
the image's 8x8 grid; --json lists them all), message and remedy.
  bg_tiles_map        256 unique tiles per background map
  vram_tiles          384 tiles shared by a background and all sheets
  obj_tiles           256 tiles for objects
  dmg_palette         4 shades for the background; 3 + transparent per sheet
  size_multiple_8     width and height a multiple of 8
  obj_pair_8x16       sheet and frame heights a multiple of 16
  obj_per_line        10 objects side by side in a frame
  obj_total           40 objects in a frame
  frame_grid          a sheet's width and height whole frames
  frame_span          a frame at most 256 pixels each way (OAM's bytes)
  romx_bank           one image's data and frame lists in a 16 KiB bank
  obj_no_transparency a warning: no transparent pixel, every object a block
                      (its remedy names #65ff00 when the sheet holds it)
  shared_shade        a warning: colors drawn in one shade (names them), in
                      one image or across images
  palette_file        assets/palettes.txt doesn't read: a bad line, a name
                      defined or an image assigned twice, an unknown name
                      or image, or dmg assigned (line=N)
  shade_stretched     a warning: the stretch put a color 2+ shades from its
                      nearest (e.g. #101010 beside only black, drawn white)
  alpha               a pixel half transparent, or any in a background
  gbc_palettes        build --gbc: over 8 palettes (names the tiles past 8)
  gbc_tile_colors     build --gbc: over 4 colors in a tile, or over 3
                      opaque colors in an object
  dmg_override_size   build --gbc: town.dmg.png isn't town.png's size
  rgbgfx_agrees       not a limit: rgbgfx drew other indices than gbcli
                      planned (a gbcli bug; re-save the PNG as 8-bit)

What the program gets (build/images/, rewritten by every build):
  INCLUDE \"images.inc\"   constants: TOWN_WIDTH, TOWN_HEIGHT (tiles),
                         TOWN_TILES, TOWN_BGP; HERO_TILES, HERO_FRAMES,
                         HERO_FRAME_WIDTH/HEIGHT, HERO_OBP; FONT_TILES
  labels                 TownTiles..TownTilesEnd (2bpp), TownTilemap..
                         TownTilemapEnd; HeroTiles..HeroTilesEnd (32 bytes
                         an object), HeroFrames: dw per frame -> count, then
                         y, x, tile, attributes per object ($20 X, $40 Y flip);
                         FontTiles..FontTilesEnd
  named palettes         PaletteForest..PaletteForestEnd (RGB555 dw, 8 bytes),
                         PALETTE_FOREST (its place in assets/palettes.txt)
  town.preview.png       the converted data as the DMG shows it, then its
                         unique tiles by use, most used first
Tile numbers count from the image's own first tile: TownTilemap's bytes,
HeroFrames' tile bytes and --json's tile_map all start at 0. Copy
TownTiles to tile N and add N to each map byte (and object tile) you
write. Tiles you load by hand go after TOWN_TILES, or the map is offset.

  gbcli build game, then gbcli run game/build/game.gb --frames 60 --shot a.png

GBC — gbcli build --gbc makes a ROM in dual mode (rgbfix -c, $0143 = $80):
GBC colors on a GBC, the same game in 4 shades on a DMG. Each image is
one of two kinds of art:
  color art     no palette assigned: drawn in any colors. gbcli packs its
                GBC palettes and derives its DMG version, each color
                taking its shade by the rules above. Its GBC tiles are
                their own data, behind the same map and frame lists.
                shared_shade then warns only for two colors in one tile
                (a sheet: one 8x16 object) that fall to one shade; a
                derived sheet in all 4 shades is dmg_palette.
  indexed art   assigned a palette in assets/palettes.txt: drawn in 4
                shades as DMG art; one set of tiles serves both machines,
                colored on the GBC by its palette, which the program can
                swap. A background's (or tileset's) index i draws entry
                i. A sheet's OBJ palette is its palette reordered by its
                OBP (sheets of one palette in different shades take one
                OBJ palette each).
To draw a color image's DMG version yourself, put town.dmg.png beside
town.png (hero.16x16.dmg.png beside hero.16x16.png): the same size, drawn
as DMG art, its colors shaded with the project's. A .dmg.png is an error
without --gbc, and beside indexed art, whose shades already are its DMG
version.
Limits: 8 BG palettes per background (one resident at a time), 4 colors a
tile; 8 OBJ palettes for all sheets together (indexed ones first), 3 +
transparent an object. Palettes the program only swaps in aren't counted.
Every tile is in VRAM bank 0 and no background tile is flipped, since the
DMG's tilemap has no flip bits; a sheet's objects still flip on both.

What a --gbc build adds:
  GBCLI_GBC              defined by the build in every source, images or
                         not (rgbasm -D GBCLI_GBC=1), not by images.inc:
                         IF DEF(GBCLI_GBC) keeps color code out of a DMG
                         build. The same ROM runs on both, so also test at
                         run time: the starter's hGbc (A = BOOTUP_A_CGB at
                         boot, and the header's $80)
  TownTilesGbc..End      color art's GBC tiles, same count and order as
                         TownTiles (indexed art uses TownTiles); HeroTilesGbc
  TownAttrmap..End       a byte per map entry, its BG palette in bits 0-2;
                         write it with VBK=1 where the tilemap goes
  TownPalettes..End      RGB555 dw, 8 bytes a palette, BG palette 0 first;
                         TOWN_PALETTES of them (indexed: its one palette)
  FontAttrs..End         a tileset's: a byte per tile, its BG palette, for
                         each map entry that uses it; FontPalettes,
                         FONT_PALETTES
  ObjPalettesGbc..End    every sheet's OBJ palettes, OBJ_PALETTES of them;
                         HeroFrames' attribute bits 0-2 pick one
  town.gbc.preview.png   the data in GBC colors, as screenshots show them,
                         then its palettes
On a GBC, with the LCD off: palettes through rBCPS/rBCPD (BG) and
rOCPS/rOCPD (OBJ), the attribute map with VBK=1, then VBK=0 and the GBC
tiles. A DMG has no VBK, so it must skip all of it, or the attribute map
lands on the tilemap.

Swap a palette (a GBC, in VBlank or with the LCD off; palette RAM is shut
while a line draws). Town, assigned forest, is BG palette 0; PaletteEmber
there recolors it, its tiles and map untouched (palette n: | n * 8):
    ld a, BGPI_AUTOINC          ; BG palette 0, color 0, then the next
    ldh [rBCPS], a
    ld hl, PaletteEmber
    ld b, PaletteEmberEnd - PaletteEmber
.swap
    ld a, [hli]
    ldh [rBCPD], a
    dec b
    jr nz, .swap
A sheet's is the same through OBPI_AUTOINC, rOCPS and rOCPD, but its
palette is reordered by its OBP: PaletteEmber loads as it is only for a
sheet in shades 1-3 (HERO_OBP %11_10_01_00).
https://gbdev.io/pandocs/Palettes.html#lcd-color-palettes-cgb-only
https://gbdev.io/pandocs/Tile_Maps.html#bg-map-attributes-cgb-mode-only

Check one ROM on both: a session's model is fixed (restore refuses another
model), so one session each. From inside game/, after gbcli build --gbc:
  gbcli run build/game.gb --session dmg --model dmg --frames 60 --shot d.png
  gbcli run build/game.gb --session gbc --model gbc --frames 60 --shot g.png
Compare d.png with the .preview.png files, g.png with the .gbc.preview.png.

Without a build: gbcli assets game --json converts the same images the
same way, with rgbgfx alone, and writes nothing (--keep DIR keeps the data
and previews; --gbc as for build). --manifest FILE names images without a
project: each as its path under assets/ (it sets kind, labels and frame
size), the file to read, and a dmg version, with the palettes
palettes.txt would hold (`gbcli schema`). The CLI reads a relative file or
dmg path from the manifest's folder.
";
