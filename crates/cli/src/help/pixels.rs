//! `gbcli help pixels`: the pixel text format, the verb, and the worked
//! example from gbcli-pixels (so the example can't drift from what parses).

use std::sync::LazyLock;

const HEAD: &str = "\
Pixels — images as text an agent can write and read.

  gbcli pixels hero.png --text hero.pixels   a PNG as pixel text
  gbcli pixels town.png --text t.pixels --tiles   as tiles and a map
  gbcli pixels hero.pixels --png hero.png    the text as the PNG the build
                                             reads; `gbcli pixels FILE` checks
  gbcli pixels hero.pixels --preview big.png 4x with an 8x8 grid, to look at
                                             (never put it in assets/)

An image in assets/ is a PNG from any tool, or pixel text; the build's name
convention (gbcli help images) is the PNG's, with .pixels for .png. Keep one
form of each image in assets/: a PNG another tool renders stays a PNG, and
pixel text you edit by hand becomes the source. PNG to text to PNG is pixel
for pixel; text to PNG to text gives the same picture in a made legend.

The format (UTF-8; a line's first word says what it is):
  format gbcli-pixels 1   the first line
  size W H                the image in pixels; once, before any block
  color C VALUE           the legend: character C (any printable ASCII but #)
                          is VALUE, one of
                            #rrggbb or #rgb   a color, 8 bits a channel
                            clear             transparent (alpha 0)
                            PALETTE N         entry N (0-3) of a palette in
                                              assets/palettes.txt; dmg 2 is
                                              shade 2, the dmg line's color 2
                            keep              leave what earlier blocks drew
  fill C                  paint every pixel C first (optional)
  rows [X Y]              a block of pixel rows, its top left at X,Y
  tile KEY                an 8x8 tile, exactly 8 rows of 8 (KEY: letters,
                          digits, _); it paints only where a map places it
  map [X Y]               a block of tile keys, one 8x8 cell each, from X,Y
A block's rows follow its header, may be indented, are all one width, and
end at a blank line. Blocks paint in file order, later over earlier; a pixel
nothing paints is an error. '#' and a space or tab (or '#' at a line's end)
starts a comment. There are no shapes, transforms or partial alpha: a
script that needs them writes the file.

Canonical text (--text): from a PNG the legend is . for clear, then 0-9,
a-z, A-Z and punctuation, lightest first, so four shades read as 0-3; from
text, your legend is kept and dmg/palette entries stay unresolved. Rows come
a block per frame when the name gives a frame size (hero.16x16.pixels),
else a block per 8-row band. --tiles writes the distinct tiles keyed 00, 01,
… in first use and a map 0 0: use it for large backgrounds. Tiles differ by
legend character, so the keys are the build's tile numbers (TownTilemap's
bytes) until the build merges two: colors equal in RGB555 (#ffffff and
#fefefe) are one color to it, and on the DMG, colors of one DMG shade draw
alike. Every block header is unique, so an edit can anchor on it. PALETTE
N and dmg N resolve through the assets/palettes.txt of the project the file
sits in, or --palettes FILE; with no dmg line, dmg N is refused (add
`dmg: #ffffff #aaaaaa #555555 #000000`).

A file that doesn't read is pixels_unreadable (exit 4), with a problem row
giving its line and column (a PNG: its pixel) and a remedy; nothing is
written. The verb's flags: gbcli pixels --help.
";

const EXAMPLE_HEAD: &str = "
Example: assets/sprites/hero.16x16.pixels, two 16x16 frames side by side
(`gbcli pixels hero.16x16.pixels` prints frames=2 tiles=6):

";

/// The topic: the format, then the example file whole.
pub fn text() -> &'static str {
    static TEXT: LazyLock<String> =
        LazyLock::new(|| format!("{HEAD}{EXAMPLE_HEAD}{}", gbcli_pixels::EXAMPLE));
    &TEXT
}
