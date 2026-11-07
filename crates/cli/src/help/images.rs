//! `gbcli help images`: the image convention `build` follows, the limits it
//! checks and what the program gets.

pub const IMAGES: &str = "\
Images — PNGs under assets/, converted by `gbcli build` with rgbgfx (RGBDS).

  assets/bg/town.png           a background: unique tiles + a tilemap
  assets/sprites/hero.png      8x16 objects; the whole sheet is one frame
  assets/sprites/hero.16x16.png  frames of 16x16, left to right, then down
Draw in up to 4 colors (any 4: grays or greens); each takes the DMG shade
nearest its lightness, unless two would share one: then all are spread
apart, lightest to darkest (2 colors: white, black; 3: white, light or dark
gray by the middle one's lightness, black; 4: all four shades). Colors equal
in RGB555 (5 bits a channel, as rgbgfx reads them) are one color.
Sprites: 3 colors + transparent (alpha 0).

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
  alpha               a pixel half transparent, or any in a background
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
  town.preview.png       the converted data as the DMG shows it, then its
                         unique tiles by use, most used first

  gbcli build game, then gbcli run build/game.gb --frames 60 --shot out.png
";
