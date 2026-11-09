//! Every parse error, pinned to its line, column, message and remedy.

use gbcli_pixels::{FORMAT_LINE, Palettes, ParseError, parse_text, read};

/// An 8x2 image whose legend is `0`, on lines 1-3.
const H: &str = "format gbcli-pixels 1\nsize 8 2\ncolor 0 #ffffff\n";

const VALUE: &str = "write `color C VALUE`: VALUE is #rrggbb, #rgb, clear, keep, or a palette and \
                     entry (`dmg 2`, `forest 1`)";
const FORMAT: &str = "make the first line `format gbcli-pixels 1`";
const SIZE: &str = "write `size W H`, e.g. `size 16 16`";
const AT: &str = "write `rows X Y` or `map X Y` (pixels from the top left), or leave the \
                  position out for 0 0";
const LEGEND_X: &str = "add `color x VALUE`, or use a legend character";
const TILE_REMEDY: &str = "a tile is exactly 8 rows of 8, then a blank line";
const KEY: &str = "write `tile KEY`: KEY is letters, digits and _";
const SAME_WIDTH: &str = "make every row in a block the same width";
const BLANK: &str = "put a blank line before it: a block ends at a blank line";

struct Case {
    name: &'static str,
    text: String,
    line: u32,
    column: u32,
    message: &'static str,
    remedy: &'static str,
}

fn case(
    name: &'static str,
    text: impl Into<String>,
    (line, column): (u32, u32),
    message: &'static str,
    remedy: &'static str,
) -> Case {
    Case {
        name,
        text: text.into(),
        line,
        column,
        message,
        remedy,
    }
}

fn with(rest: &str) -> String {
    format!("{H}{rest}")
}

fn check(c: &Case, got: Result<impl std::fmt::Debug, ParseError>) {
    let e = got.expect_err(c.name);
    let want = ParseError {
        line: c.line,
        column: c.column,
        message: c.message.into(),
        remedy: c.remedy.into(),
    };
    assert_eq!(e, want, "{}:\n{}", c.name, c.text);
}

#[test]
fn every_parse_error_is_located_and_says_what_to_do() {
    let row8 = "00000000\n";
    let tile8 = row8.repeat(8);
    let cases = [
        // The format line.
        case(
            "no format line",
            "size 8 8\n",
            (1, 1),
            "pixel text starts with its format line",
            FORMAT,
        ),
        case(
            "another version",
            "format gbcli-pixels 2\n",
            (1, 1),
            "`gbcli-pixels 2` is not a format this gbcli reads",
            FORMAT,
        ),
        case(
            "a format line again",
            format!("{H}{FORMAT_LINE}\n"),
            (4, 1),
            "the format line is given again (first on line 1)",
            "give the format line once, first",
        ),
        case(
            "an unknown word",
            with("colour 1 #000000\n"),
            (4, 1),
            "`colour` doesn't start a line: a line starts with size, color, fill, rows, tile or map",
            "start the line with one of those, or make it a comment with `# `",
        ),
        // size.
        case(
            "no size",
            "format gbcli-pixels 1\ncolor 0 #ffffff\n",
            (1, 1),
            "the file has no size line",
            "add `size W H` after the format line",
        ),
        case(
            "size without a height",
            "format gbcli-pixels 1\nsize 8\n",
            (2, 1),
            "size takes a width and a height in pixels",
            SIZE,
        ),
        case(
            "size not a number",
            "format gbcli-pixels 1\nsize 8 x\n",
            (2, 8),
            "`x` is not a number of pixels",
            SIZE,
        ),
        case(
            "size zero",
            "format gbcli-pixels 1\nsize 0 8\n",
            (2, 6),
            "an image is 1 to 1024 pixels a side",
            SIZE,
        ),
        case(
            "size too large",
            "format gbcli-pixels 1\nsize 8 1025\n",
            (2, 8),
            "an image is 1 to 1024 pixels a side",
            SIZE,
        ),
        case(
            "size again",
            with("size 8 8\n"),
            (4, 1),
            "size is given again (first on line 2)",
            "give the size once",
        ),
        case(
            "a block before size",
            "format gbcli-pixels 1\ncolor 0 #fff\nrows\n0\n",
            (3, 1),
            "rows comes before size, so the image has no size yet",
            "put `size W H` before the first fill, rows, tile or map",
        ),
        // color.
        case(
            "color alone",
            with("color\n"),
            (4, 1),
            "color takes a legend character and a value",
            VALUE,
        ),
        case(
            "color without a value",
            with("color 1   # dark\n"),
            (4, 1),
            "color takes a legend character and a value",
            VALUE,
        ),
        case(
            "two characters",
            with("color ab #000\n"),
            (4, 7),
            "`ab` is not one legend character",
            "a legend character is one printable ASCII character other than # and space",
        ),
        case(
            "not ASCII",
            with("color é #000\n"),
            (4, 7),
            "`é` is not one legend character",
            "a legend character is one printable ASCII character other than # and space",
        ),
        case(
            "a character again",
            with("color 0 #000000\n"),
            (4, 7),
            "0 is in the legend already (line 3)",
            "give each legend character one value; pick another character",
        ),
        case(
            "not a value",
            with("color 1 red\n"),
            (4, 9),
            "`red` is not a value",
            VALUE,
        ),
        case(
            "a short color",
            with("color 1 #12\n"),
            (4, 9),
            "`#12` is no #rrggbb or #rgb color",
            VALUE,
        ),
        case(
            "a fifth entry",
            with("color 1 dmg 4\n"),
            (4, 13),
            "`4` is no palette entry: entries are 0 to 3",
            VALUE,
        ),
        case(
            "a palette not named by a word",
            with("color 1 Sea 2\n"),
            (4, 9),
            "`Sea` is no palette name, so `Sea 2` is not a value",
            VALUE,
        ),
        case(
            "a word after the value",
            with("color 1 #000000 x\n"),
            (4, 17),
            "`x` follows the value; color takes one character and one value",
            "make the rest a comment with `# `",
        ),
        // fill.
        case(
            "fill alone",
            with("fill\n"),
            (4, 1),
            "fill takes one legend character",
            "write `fill C` with C in the legend",
        ),
        case(
            "fill with two",
            with("fill 0 0\n"),
            (4, 1),
            "fill takes one legend character",
            "write `fill C` with C in the legend",
        ),
        case(
            "fill with an unknown character",
            with("fill x\n"),
            (4, 6),
            "x isn't in the legend: it has 0",
            LEGEND_X,
        ),
        case(
            "fill twice",
            with("fill 0\nfill 0\n"),
            (5, 1),
            "fill is given again (first on line 4)",
            "fill once, before the blocks",
        ),
        case(
            "fill after a block",
            with(&format!("rows\n{row8}{row8}\nfill 0\n")),
            (8, 1),
            "fill comes after a block, but it paints before every block",
            "move fill above the first rows, tile or map",
        ),
        case(
            "fill keep",
            with("color k keep\nfill k\n"),
            (5, 6),
            "fill can't keep: it paints first, so there is nothing to keep",
            "fill with a color or clear",
        ),
        // Block headers.
        case(
            "rows with one coordinate",
            with("rows 1\n"),
            (4, 1),
            "rows takes no position, or an X and a Y",
            AT,
        ),
        case(
            "rows with a word for x",
            with("rows a 0\n"),
            (4, 6),
            "`a` is not a number of pixels",
            AT,
        ),
        case(
            "rows outside the image",
            with("rows 8 0\n"),
            (4, 6),
            "x 8 is outside the image, which is 8 wide",
            "start the block inside the image: x is 0 to 7",
        ),
        case(
            "map below the image",
            with("map 0 2\n"),
            (4, 7),
            "y 2 is outside the image, which is 2 high",
            "start the block inside the image: y is 0 to 1",
        ),
        case("tile alone", with("tile\n"), (4, 1), "tile takes one key", KEY),
        case(
            "a bad tile key",
            with("tile a-b\n"),
            (4, 6),
            "`a-b` is not a tile key",
            KEY,
        ),
        case(
            "a tile again",
            with(&format!("tile a\n{tile8}\ntile a\n{tile8}")),
            (14, 6),
            "tile a is defined already (line 4)",
            "give each tile its own key",
        ),
        case(
            "an empty rows block",
            with("rows 0 0\n\n"),
            (4, 1),
            "this rows block has no rows",
            "draw its rows under it, before a blank line",
        ),
        case(
            "an empty map at the end",
            with("map"),
            (4, 1),
            "this map block has no rows",
            "draw its rows under it, before a blank line",
        ),
        // Pixel rows.
        case(
            "a row of two words",
            with("rows\n0000 0000\n"),
            (5, 6),
            "a row is one word, but this line has another",
            "draw a row without spaces; a block ends at a blank line",
        ),
        case(
            "a header inside a block",
            with(&format!("rows 0 0\n{row8}rows 0 1\n{row8}")),
            (6, 1),
            "`rows` starts a block, but the block above hasn't ended",
            BLANK,
        ),
        case(
            "an unknown character",
            with("rows\n00x00000\n"),
            (5, 3),
            "x isn't in the legend: it has 0",
            LEGEND_X,
        ),
        case(
            "a hash in a row",
            with("rows\n000#0000\n"),
            (5, 4),
            "# isn't a legend character: it starts comments and colors",
            "draw with a legend character; # and space are never one",
        ),
        case(
            "a narrower row",
            with("rows\n00000000\n0000000\n"),
            (6, 1),
            "this row is 7 pixels wide, and the block's first is 8",
            SAME_WIDTH,
        ),
        case(
            "past the right edge",
            with("rows 4 0\n00000\n"),
            (5, 5),
            "this row runs past the image's right edge: the image is 8 wide",
            "shorten the row, or move the block left",
        ),
        case(
            "past the bottom edge",
            with(&format!("rows\n{row8}{row8}{row8}")),
            (7, 1),
            "this row is past the image's bottom edge: the image is 2 high",
            "end the block sooner, or make the image taller with size",
        ),
        // Tiles.
        case(
            "a short tile row",
            with("tile a\n0000000\n"),
            (5, 1),
            "a tile row is 8 pixels; this one is 7",
            TILE_REMEDY,
        ),
        case(
            "a ninth tile row",
            with(&format!("tile a\n{tile8}{row8}")),
            (13, 1),
            "tile a has a ninth row; a tile is 8 rows",
            TILE_REMEDY,
        ),
        case(
            "a short tile",
            with(&format!("tile a\n{row8}\n")),
            (4, 6),
            "tile a has 1 row; a tile is 8",
            TILE_REMEDY,
        ),
        case(
            "an unknown character in a tile",
            with("tile a\n0000000x\n"),
            (5, 8),
            "x isn't in the legend: it has 0",
            LEGEND_X,
        ),
        // Maps.
        case(
            "a map before any tile",
            with("map\ngx\n"),
            (5, 1),
            "no tile gx: no tile is defined yet",
            "name a tile defined with `tile KEY`, or define gx",
        ),
        case(
            "a map naming an unknown tile",
            with(&format!(
                "size 64 24\ntile sk\n{tile8}\ntile cl\n{tile8}\ntile gt\n{tile8}\ntile di\n{tile8}\nmap\nsk gx\n"
            ))
            .replace("size 8 2\n", ""),
            (45, 4),
            "no tile gx: the tiles are sk, cl, gt and di",
            "name a tile defined with `tile KEY`, or define gx",
        ),
        case(
            "a narrower map row",
            with(&format!("tile a\n{tile8}\nmap\na a\na\n")).replace("size 8 2", "size 16 16"),
            (16, 1),
            "this row names 1 tile, and the block's first names 2",
            SAME_WIDTH,
        ),
        case(
            "a map past the right edge",
            with(&format!("tile a\n{tile8}\nmap\na a\n")).replace("size 8 2", "size 8 8"),
            (15, 3),
            "this tile runs past the image's right edge: the image is 8 wide",
            "map fewer tiles, or move the block left",
        ),
        case(
            "a map past the bottom edge",
            with(&format!("tile a\n{tile8}\nmap\na\n")),
            (15, 1),
            "this row of tiles runs past the image's bottom edge: the image is 2 high",
            "map fewer rows, or move the block up",
        ),
        case(
            "a header inside a map",
            with(&format!("tile a\n{tile8}\nmap\na\ntile b\n")).replace("size 8 2", "size 8 8"),
            (16, 1),
            "`tile` starts a block, but the block above hasn't ended",
            BLANK,
        ),
        // The picture as a whole.
        case(
            "an unpainted pixel",
            with(&format!("rows\n{row8}")),
            (2, 1),
            "pixel 0,1 is never painted: no fill or block covers it",
            "draw it in a rows or map block, or `fill C` first",
        ),
        case(
            "a kept pixel nothing painted",
            with(&format!("color k keep\nrows\n{row8}0000000k\n")),
            (2, 1),
            "pixel 7,1 is never painted: no fill or block covers it",
            "draw it in a rows or map block, or `fill C` first",
        ),
    ];
    for c in &cases {
        check(c, parse_text(&c.text));
    }
}

#[test]
fn a_file_that_isnt_utf8_is_located_to_its_line() {
    let mut bytes = with("rows\n").into_bytes();
    bytes.extend_from_slice(b"\xff\xfe\n");
    let e = read(&bytes).expect_err("not UTF-8");
    let gbcli_pixels::PixelsError::Text(e) = e else {
        panic!("{e:?}")
    };
    assert_eq!(
        e,
        ParseError {
            line: 5,
            column: 1,
            message: "this line is not UTF-8, so the file is neither a PNG nor pixel text".into(),
            remedy: "an image file is a PNG, or UTF-8 text starting with `format gbcli-pixels 1`"
                .into(),
        }
    );
}

#[test]
fn a_value_that_doesnt_resolve_is_located_to_it() {
    let text = with("color 1 dmg 2\ncolor 2 forest 1\nrows\n01010101\n22222222\n");
    let picture = parse_text(&text).expect("parses: palettes resolve later");
    let e = picture
        .raster(&Palettes::default())
        .expect_err("no dmg line");
    assert_eq!(
        e,
        ParseError {
            line: 4,
            column: 9,
            message: "`dmg 2` is shade 2, but the palettes have no dmg line to say its color"
                .into(),
            remedy: "add `dmg: #ffffff #aaaaaa #555555 #000000` (shade 0 first) to \
                     assets/palettes.txt"
                .into(),
        }
    );
    let palettes = Palettes {
        dmg: Some([[255; 3], [170; 3], [85; 3], [0; 3]]),
        named: vec![("sea".into(), [[0; 3]; 4])],
    };
    let e = picture.raster(&palettes).expect_err("no forest");
    assert_eq!(
        e,
        ParseError {
            line: 5,
            column: 9,
            message: "no palette forest: the palettes define sea".into(),
            remedy: "define `forest: c0 c1 c2 c3` in assets/palettes.txt, or write the color \
                     as #rrggbb"
                .into(),
        }
    );
    let none = picture.raster(&Palettes {
        named: vec![],
        ..palettes
    });
    assert_eq!(
        none.expect_err("no palettes").message,
        "no palette forest: the palettes define none"
    );
}
