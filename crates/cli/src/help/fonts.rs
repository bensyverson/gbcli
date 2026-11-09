//! `gbcli help fonts`: the font file, the layout rules (from gbcli-font,
//! so the help can't drift from what `gbcli font --sample` does), and
//! what a build gives the program.

use std::sync::LazyLock;

const HEAD: &str = "\
Fonts — pixel fonts for text, fixed or variable width.

  assets/fonts/NAME.yaff     a font: glyphs 8 px tall and 1-8 wide, a bit a
                             pixel (ink or paper), numbered in file order
  gbcli font FILE            check it: its glyphs, fixed, the widths
  gbcli font FILE --sample \"Press START\" --fit 144
                             set text by the rules below; exit 1 past 144 px
  gbcli font FILE --preview big.png [--sample TEXT]
                             4x, tiles marked; every glyph without a sample

The file is a strict subset of YAFF 1.0 (monobit's YAFF.md, which other
font tools read): comments start with #; properties (name: Talk) come
first; then each glyph is one label and 8 indented rows of . (paper) and @
(ink), as wide as the glyph advances:

  name: Talk

  # The space: 3 px of paper.
  u+0020:
      ...
      ...
      ...
      ...
      ...
      ...
      ...
      ...

  'i':
      ..
      @.
      ..
      @.
      @.
      @.
      @.
      ..

A key is one character ('é':), a code point (u+00E9:), or a name (\"heart\":
two or more ASCII letters, digits, _ and -, starting with a letter) for a
symbol Unicode lacks. Keys are NFC: a letter followed by a combining mark
that gbcli's mark table knows (Latin-1, Latin Extended-A, and kana with
u+3099 or u+309A) is refused, naming its precomposed letter. Refused too:
codepoint labels (0x41:), several characters in one key, unquoted labels
(A:), a second label for a glyph, control characters, YAFF's empty glyph
(-), per-glyph properties, kerning and bearings. `gbcli font` names the
line and column of the first.

The layout rules (normative; `gbcli font --sample` is the reference, and its
--json `data` is the line's tiles by rule 3):
";

const TAIL: &str = "
What a build gives the program, for each font of at most 254 glyphs, in
images.inc and images.asm (INCLUDE \"images.inc\"); for main.yaff:
  MainFontGlyphs     8 bytes a glyph, by rule 3: glyph n at MainFontGlyphs+8n
  MainFontWidths     a byte a glyph: its width, 1-8
  MAIN_FONT_GLYPHS   how many: glyph numbers run 0 to MAIN_FONT_GLYPHS - 1
  MAIN_FONT_FIXED    1 when every glyph is 8 wide
  MAIN_FONT_NEWLINE  $FE, the charmap's \"\\n\";  MAIN_FONT_END  $FF, a string's
                     end. No glyph takes either.
  MAIN_FONT_CHARMAP  a macro: MAIN_FONT_CHARMAP name, base defines the
                     charmap `name`, each glyph as base + its number, and
                     leaves the active charmap as it was

    INCLUDE \"images.inc\"
        MAIN_FONT_CHARMAP text, 0      ; glyph numbers, for a renderer
        MAIN_FONT_CHARMAP hud, $80     ; tile numbers, glyphs loaded at $80
        PUSHC hud
    Score: db \"SCORE <heart>\", MAIN_FONT_END   ; a named glyph is <name>
        POPC

Glyphs are never merged: two drawn alike keep their own numbers. A
character the font lacks stays rgbasm's warning (Unmapped character). Keep
base + the last glyph number below $FE.

Past 254 glyphs (an imported Japanese font, say), list the characters the
program's strings use in assets/fonts/NAME.charlist, as text (a named glyph
as <name>; line breaks are ignored): the build emits only those, numbered
in the font's order. Without one it refuses: font_glyphs. A listed
character the font lacks is font_missing, and a font that doesn't read is
font_text, with line= and column=.

`gbcli assets` returns each font as a FontAsset (`gbcli schema`): its glyphs
in glyph-number order, each with key, width, 8 row bytes (by rule 3) and
origin (drawn or composed, below), and fixed. Its request's `glyphs` names
the characters wanted from each font (by its path, fonts/NAME.yaff), with
no cap.

Composed letters. A letter asked for (by a request's `glyphs`, or a
character list) that the font doesn't draw is made from its base and its
combining mark when the font draws both: é from 'e' and u+0301, for
gbcli's mark table (Latin-1, Latin Extended-A, and kana voiced by u+3099
or u+309A, so が from か and u+3099). The mark's pixels are laid over the
base's at the base's width; a drawn é always wins. Composed glyphs follow
the drawn ones, origin composed. A mark landing on its base's ink is a
warning naming the glyph (`overlap`, and font_overlaps in --json): draw
that letter by hand. Nothing is composed unless asked for.

Authoring. The .yaff is the font's one source, committed; a PNG or BDF is
a working copy for an editing tool, never committed:
  gbcli font edit FONT.yaff [--as png|bdf]
                       write a copy to the project's build/font-edit/
                       (gitignored) and print its path
  gbcli font save FONT.yaff
                       convert the copy back (its path works too), keep
                       the font's comments and properties, remove the copy
  gbcli font new NAME --chars ascii|latin1|latin-ext-a|FILE [--guide-sheet PNG]
                       an empty, guided copy for assets/fonts/NAME.yaff
  gbcli font import FILE.bdf [--chars SET|FILE] [--out FONT.yaff]
  gbcli font export FONT.yaff [--png PATH [--guides]] [--bdf PATH]
  gbcli font diff A.yaff B.yaff [--png PATH]
                       each changed glyph; the PNG draws it before and after
A build refuses while a copy is newer than its font, naming `gbcli font
save`. One copy at a time. Beside the copy, NAME.base records the font as
it was; if the font changed since, save refuses, naming both (remove
NAME.base to save the copy over it anyway). Export, a guide sheet and a
diff's PNG never write over a .yaff or into build/font-edit/.

The PNG copy is a grid of 8x8 cells, 16 to a row (128 px wide), glyphs in
order, each at its cell's left edge. An opaque pixel is ink (the darker of
two colors) or paper; a glyph's width runs to its cell's last opaque
column, so spacing is drawn as paper, and every pixel left of that is
opaque. Other pixels are clear: alpha 0, or alpha 1-254, guides, which
saving drops (every other cell and the rows below the baseline are
tinted). Beside it, NAME.chars names the cells, one line per row of
cells: each character as itself, a named glyph as <name>, any character
as <u+XXXX>, and <> for a cell left empty. Every disagreement with the
art is a problem naming its cell: a drawn cell with no character, a
character over an empty cell, a clear pixel inside a glyph, a third
color, a line past 16 cells or the image's rows, a character listed
twice.

BDF import places each glyph by its bounding box and the font's ascent
(FONT_ASCENT), so trimmed glyphs land where they draw, and advances it by
DWIDTH (1-8). The BDF must be Unicode (ISO10646): convert a JIS-encoded
one, such as Misaki's, first (FontForge, or monobit-convert). Its
properties become the font's, FONT_ASCENT as ascent unless it is the
default (7, with FONT_DESCENT 1).
";

/// The starter's renderer (leaf RDlaLo). Its cost is measured, and the CLI
/// test `starter_text` holds TextDraw to the bound stated here.
const STARTER: &str = "
The starter's renderer: `gbcli new` writes src/text.asm, a font drawn for
gbcli (assets/fonts/main.yaff: the 95 printable ASCII characters, 2-6 px wide,
glyph n is ASCII 32 + n), and one line drawn with them in src/main.asm.
  TextDraw       in: hl a string (MAIN_FONT_CHARMAP text, 0), de the line's
                 first tile, b ink and c paper (shades 0-3). Draws by the rules
                 above up to MAIN_FONT_END or MAIN_FONT_NEWLINE, papering each
                 tile as the line reaches it. Out: a the tiles touched,
                 (width + 7) / 8; hl past the end (after a NEWLINE, the next
                 line's string). Tiles past the line keep what they held.
  TextLoadTiles  in: de glyph rows (MainFontGlyphs + 8 * n), hl the first tile,
                 a how many, b ink, c paper. Each glyph becomes a tile, at its
                 left edge: a fixed-width font for text written straight into
                 the tilemap (MAIN_FONT_CHARMAP hud, $80 for tiles from $80).
Both write tiles: VRAM only with the LCD off, else a WRAM buffer copied in
VBlank (`gbcli help traps`). The font is in ROMX: with an MBC, switch to
BANK(MainFontGlyphs) first. The starter keeps 20 tiles, 236-255 (a line the
screen's width), as its line buffer, at map row 11. The image checks don't
count them: keep a background's tiles below 236, or move the buffer.

Cost: TextDraw takes 517 to at most 954 M-cycles a glyph, about 5% of a frame
(every glyph of main.yaff at all 8 pen offsets: gbcli measure
TextDraw.glyph..TextDraw.glyph --iterations <glyphs>). The width hardly
matters; the most is a glyph that crosses into a tile the line hasn't reached.
The starter's 16-glyph line takes 11,112 (63% of a frame), once, with the LCD
off.

The typewriter (a recipe: one character a step, as a dialogue box prints):
  1. Draw the whole line once into a WRAM buffer (TextDraw with de in WRAM, so
     the LCD stays on), and paper the line's tiles in VRAM.
  2. Keep the revealed width in WRAM, from 0. Each step (a frame, or every
     few), read the string's next byte k (stop at MAIN_FONT_END) and add the
     glyph's width, [MainFontWidths + k].
  3. In VBlank, copy the tiles from the one holding the glyph's first pixel
     to the one holding its last: whole if the tile ends at or before the
     revealed width, else each byte ANDed with $FF << (8 - width % 8), which
     leaves later glyphs unshown (for paper shade 0; another paper also ORs
     the mask's complement into the planes paper sets). At most 2 tiles, 32
     bytes, well inside VBlank's 1,140 M-cycles.
";

pub fn text() -> &'static str {
    static TEXT: LazyLock<String> =
        LazyLock::new(|| format!("{HEAD}{}{TAIL}{STARTER}", gbcli_font::RULES));
    &TEXT
}
