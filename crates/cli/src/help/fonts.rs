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
that gbcli's mark table knows (Latin-1 and Latin Extended-A) is refused,
naming its precomposed letter. Refused too: codepoint labels (0x41:),
several characters in one key, unquoted labels (A:), a second label for a
glyph, control characters, YAFF's empty glyph (-), per-glyph properties,
kerning and bearings. `gbcli font` names the line and column of the first.

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
origin (drawn), and fixed. Its request's `glyphs` names the characters
wanted from each font (by its path, fonts/NAME.yaff), with no cap.
";

pub fn text() -> &'static str {
    static TEXT: LazyLock<String> = LazyLock::new(|| format!("{HEAD}{}{TAIL}", gbcli_font::RULES));
    &TEXT
}
