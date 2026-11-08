//! `gbcli help images`: the image convention `build` follows, the limits it
//! checks and what the program gets.

pub const IMAGES: &str = "\
Images — PNGs under assets/, converted by `gbcli build` with rgbgfx (RGBDS).

  assets/bg/town.png           a background: unique tiles + a tilemap
  assets/sprites/hero.png      8x16 objects; the whole sheet is one frame
  assets/sprites/hero.16x16.png  frames of 16x16, left to right, then down
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

assets/palettes.txt names palettes, one a line, 4 colors, index 0 (the
DMG's lightest) first, and assigns them to images by path under assets/:
  dmg:    #e0f8d0 #88c070 #346856 #081820   # reserved: the shades above
  forest: #fff #8c4 #363 #021               # #rgb is #rrggbb (#88cc44)
  bg/town.png = forest
Colors are #rrggbb or #rgb, kept as RGB555. Names are a lowercase letter,
then lowercase letters, digits or _. '#' then a space or tab (or '#' last
on a line) starts a comment; any other '#' word is a color, so a fifth
color is an error. A build uses the dmg line, checks the rest, and builds
an assigned image as DMG art. build --gbc colors it by its palette too:
a background's tiles draw index i in its entry i; a sheet's OBJ palette is
its palette reordered by the sheet's OBP (sheets of one palette drawn in
different shades take one OBJ palette each, of the GBC's 8). Their tile
data serves both machines.

build --gbc: an image no palette is assigned to is color art, in any
colors. Its DMG version is derived: each color takes its shade by the
rules above. shared_shade then warns only for two colors in one tile (a
sheet: one 8x16 object) that fall to one shade, naming those tiles; a
derived sheet in all 4 shades is dmg_palette. To draw the DMG version
yourself, put town.dmg.png beside town.png (hero.16x16.dmg.png beside
hero.16x16.png): the same size, drawn as DMG art. A .dmg.png is an error
without --gbc, and beside an assigned image, whose shades are already its
DMG version.

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
  shared_shade        a warning: colors drawn in one shade (names them), in
                      one image or across images
  palette_file        assets/palettes.txt doesn't read: a bad line, a name
                      defined or an image assigned twice, an unknown name
                      or image, or dmg assigned (line=N)
  shade_stretched     a warning: the stretch put a color 2+ shades from its
                      nearest (e.g. #101010 beside only black, drawn white)
  alpha               a pixel half transparent, or any in a background
  dmg_override_size   build --gbc: town.dmg.png isn't town.png's size
  rgbgfx_agrees       not a limit: rgbgfx drew other indices than gbcli
                      planned (a gbcli bug; re-save the PNG as 8-bit)

What the program gets (build/images/, rewritten by every build):
  INCLUDE \"images.inc\"   constants: TOWN_WIDTH, TOWN_HEIGHT (tiles),
                         TOWN_TILES, TOWN_BGP; HERO_TILES, HERO_FRAMES,
                         HERO_FRAME_WIDTH/HEIGHT, HERO_OBP
  labels                 TownTiles..TownTilesEnd (2bpp), TownTilemap..
                         TownTilemapEnd; HeroTiles..HeroTilesEnd (32 bytes
                         an object), HeroFrames: dw per frame -> count, then
                         y, x, tile, attributes per object ($20 X, $40 Y flip)
  named palettes         PaletteForest..PaletteForestEnd (RGB555 dw, 8 bytes),
                         PALETTE_FOREST (its place in assets/palettes.txt)
  with --gbc             GBCLI_GBC; TownTilesGbc (color art only), TownAttrmap,
                         TownPalettes, TOWN_PALETTES; HeroTilesGbc;
                         ObjPalettesGbc, OBJ_PALETTES; attribute bits 0-2 are
                         the GBC palette
  town.preview.png       the converted data as the DMG shows it, then its
                         unique tiles by use, most used first
  town.gbc.preview.png   with --gbc: the same in GBC colors, as screenshots
                         show them, then its palettes

  gbcli build game, then gbcli run build/game.gb --frames 60 --shot out.png
";
