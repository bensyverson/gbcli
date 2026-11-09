//! `font`: a `.yaff` font read, a sample set by the layout rules, a
//! preview, and `--fit`.

use super::flags::{JSON, switch, value};
use super::{Arity, Class, FlagSpec, Verb, VerbSpec};

const FONT: &[FlagSpec] = &[
    value(
        "sample",
        "TEXT",
        "set TEXT by the layout rules (`gbcli help fonts`) and print its width and tiles; a \
         named glyph is written <name>",
    ),
    value(
        "preview",
        "PATH",
        "write a 4x PNG with tile boundaries marked: the sample, or without one every glyph \
         (for looking only; never in assets/)",
    ),
    value(
        "fit",
        "PX",
        "fail (exit 1) when the sample is wider than PX pixels",
    ),
];

/// `--json` alone: `font` needs no session, so no `--dir` or `--as`.
const JSON_ONLY: &[FlagSpec] = &[JSON];

pub const FONT_VERB: VerbSpec = VerbSpec {
    verb: Verb::Font,
    name: "font",
    class: Class::Build,
    summary: "read a .yaff font; set a sample, preview it, check it fits",
    usage: "FILE [--sample TEXT] [--preview PATH] [--fit PX] [--json]",
    arity: Arity {
        min: 1,
        max: Some(1),
        name: "FILE",
    },
    flags: FONT,
    groups: &[JSON_ONLY],
    about: "Reads FILE, a font in gbcli's YAFF subset (`gbcli help fonts`), and prints a \
            `font` row: its glyphs, whether every glyph is 8 wide (fixed), and its narrowest \
            and widest glyph. --sample sets TEXT on the host by the layout rules, the \
            reference a renderer matches pixel for pixel, and prints a `sample` row: its width \
            in pixels, the tiles it touches and any character the font lacks (missing, which \
            takes no room); --json adds its tiles as 1bpp hex. --preview writes a 4x PNG. \
            --fit PX fails with predicate_false (exit 1) when the sample is wider. A font that \
            doesn't read is font_unreadable (exit 4) with its line and column in a `problem` \
            row. Needs no session and no RGBDS.",
    example: (
        "gbcli font assets/fonts/main.yaff --sample \"Press START\" --fit 144",
        "prints the sample's width and tiles, and exits 1 if it is wider than 144 px",
    ),
};

/// `--chars`, one meaning on `import` and `new`.
const CHARS_HELP: &str = "a named set (ascii, latin1, latin-ext-a) or a file of the characters \
                          (a named glyph as <name>; line breaks ignored)";

const EDIT: &[FlagSpec] = &[value(
    "as",
    "NAME",
    "the working copy's form: png (a grid of 8x8 cells with a .chars sidecar and guides; the \
     default) or bdf (for bitmap font editors)",
)];

pub const FONT_EDIT_VERB: VerbSpec = VerbSpec {
    verb: Verb::FontEdit,
    name: "font edit",
    class: Class::Build,
    summary: "write a font's working copy (PNG or BDF) to edit in another tool",
    usage: "FONT.yaff [--as png|bdf] [--json]",
    arity: Arity {
        min: 1,
        max: Some(1),
        name: "FONT.yaff",
    },
    flags: EDIT,
    groups: &[JSON_ONLY],
    about: "Writes a working copy of FONT.yaff to the project's build/font-edit/ (gitignored) \
            and prints its path in a `wrote` row. --as png (the default) writes NAME.png, a \
            grid of 8x8 cells 16 to a row with each glyph at its cell's left, and NAME.chars, \
            one line per row of cells naming them; half-transparent pixels (alpha 1-254) are \
            guides that saving drops. --as bdf writes NAME.bdf for bited, Bits'N'Picas or \
            FontForge. NAME.base beside it records the font as it was, for save to check. A \
            build refuses while the copy is newer than the font; `gbcli font save` brings it \
            back. One copy at a time.",
    example: (
        "gbcli font edit assets/fonts/talk.yaff --as png",
        "writes build/font-edit/talk.png and talk.chars to draw in",
    ),
};

pub const FONT_SAVE_VERB: VerbSpec = VerbSpec {
    verb: Verb::FontSave,
    name: "font save",
    class: Class::Build,
    summary: "convert a font's working copy back into its .yaff and remove it",
    usage: "FONT.yaff|COPY [--json]",
    arity: Arity {
        min: 1,
        max: Some(1),
        name: "FONT.yaff",
    },
    flags: &[],
    groups: &[JSON_ONLY],
    about: "Reads the working copy of FONT.yaff (or the copy named by its own path), writes \
            its glyphs into FONT.yaff, keeping the font's comments and properties, and removes \
            the copy. Every disagreement between the PNG and its .chars (a drawn cell with no \
            character, a character over an empty cell, a clear pixel inside a glyph, a third \
            color, a character listed twice) is a `problem` row naming its cell; then nothing \
            is written (font_unreadable, exit 4). An empty cell listed in .chars is written <> \
            to leave it empty. If FONT.yaff changed after the copy was made (its NAME.base no \
            longer matches), the save is refused naming both, so neither's edits are lost; \
            removing NAME.base saves the copy over the font anyway.",
    example: (
        "gbcli font save assets/fonts/talk.yaff",
        "writes the edited glyphs into talk.yaff and removes build/font-edit/talk.png",
    ),
};

const IMPORT: &[FlagSpec] = &[
    value("chars", "CHARS", CHARS_HELP),
    value(
        "out",
        "FILE",
        "the .yaff to write (default: the BDF's name with .yaff, beside it); never written over",
    ),
];

pub const FONT_IMPORT_VERB: VerbSpec = VerbSpec {
    verb: Verb::FontImport,
    name: "font import",
    class: Class::Build,
    summary: "import a BDF font as a .yaff",
    usage: "FILE.bdf [--chars CHARS] [--out FILE] [--json]",
    arity: Arity {
        min: 1,
        max: Some(1),
        name: "FILE.bdf",
    },
    flags: IMPORT,
    groups: &[JSON_ONLY],
    about: "Reads a Unicode BDF font and writes it as a .yaff: each glyph placed in its 8x8 \
            cell by its bounding box and the font's ascent (FONT_ASCENT), so trimmed glyphs \
            land where they draw, advancing by DWIDTH; the BDF's properties become the font's. \
            The BDF must be Unicode (ISO10646): convert a JIS-encoded one, such as Misaki's, \
            first (FontForge, or monobit-convert). --chars keeps only those characters, and lists those the BDF lacks in \
            a `missing` row. A glyph wider than 8 or drawn outside its cell is a `problem` row \
            naming it (font_unreadable, exit 4); glyphs with no gbcli key are skipped and \
            listed.",
    example: (
        "gbcli font import pixel.bdf --chars latin1 --out assets/fonts/pixel.yaff",
        "writes the Latin-1 glyphs pixel.bdf has as assets/fonts/pixel.yaff",
    ),
};

const EXPORT: &[FlagSpec] = &[
    value(
        "png",
        "PATH",
        "write the font as a grid PNG, with its .chars beside it",
    ),
    value("bdf", "PATH", "write the font as BDF"),
    switch("guides", "with --png, draw guides in the clear pixels"),
];

pub const FONT_EXPORT_VERB: VerbSpec = VerbSpec {
    verb: Verb::FontExport,
    name: "font export",
    class: Class::Build,
    summary: "write a font as a grid PNG (with .chars) or BDF, where asked",
    usage: "FONT.yaff [--png PATH [--guides]] [--bdf PATH] [--json]",
    arity: Arity {
        min: 1,
        max: Some(1),
        name: "FONT.yaff",
    },
    flags: EXPORT,
    groups: &[JSON_ONLY],
    about: "Writes FONT.yaff in the working copies' forms to the paths given, outside the \
            edit/save cycle: to share it, start another font from it, or check a tool's view \
            of it. The build ignores these files. It writes over an earlier export, but never \
            over a .yaff or into build/font-edit/, where the working copies live.",
    example: (
        "gbcli font export assets/fonts/talk.yaff --png talk.png --guides",
        "writes talk.png and talk.chars with guides",
    ),
};

const NEW: &[FlagSpec] = &[
    value("chars", "CHARS", CHARS_HELP),
    value(
        "guide-sheet",
        "PATH",
        "also write a 4x sheet labeling every cell with its character (code point in hex, or \
         name), to keep open beside the editor",
    ),
];

pub const FONT_NEW_VERB: VerbSpec = VerbSpec {
    verb: Verb::FontNew,
    name: "font new",
    class: Class::Build,
    summary: "start a font: an empty, guided working copy for the characters asked",
    usage: "NAME --chars CHARS [--guide-sheet PATH] [--json]",
    arity: Arity {
        min: 1,
        max: Some(1),
        name: "NAME",
    },
    flags: NEW,
    groups: &[JSON_ONLY],
    about: "Writes a working copy for a font that doesn't exist yet, assets/fonts/NAME.yaff \
            (or NAME itself when it ends in .yaff): a grid PNG of empty cells, one per \
            character, with guides (every other cell tinted, the rows below the baseline \
            tinted), and its .chars. Draw in it, then `gbcli font save` writes the font. \
            --chars is required. The guide sheet, like an export, never goes over a .yaff or \
            into build/font-edit/.",
    example: (
        "gbcli font new talk --chars latin1 --guide-sheet sheet.png",
        "writes build/font-edit/talk.png and talk.chars for 191 characters, and sheet.png",
    ),
};

const DIFF: &[FlagSpec] = &[value(
    "png",
    "PATH",
    "draw each changed glyph before and after, at 4x",
)];

pub const FONT_DIFF_VERB: VerbSpec = VerbSpec {
    verb: Verb::FontDiff,
    name: "font diff",
    class: Class::Build,
    summary: "compare two fonts glyph by glyph; draw the changes",
    usage: "A.yaff B.yaff [--png PATH] [--json]",
    arity: Arity {
        min: 2,
        max: Some(2),
        name: "A.yaff B.yaff",
    },
    flags: DIFF,
    groups: &[JSON_ONLY],
    about: "Compares two fonts by key and prints a `glyph` row for each changed, added or \
            removed glyph, with its widths, and a `diff` row with the counts and whether the \
            glyphs both hold were reordered (which renumbers them). Comments are not \
            differences. Exits 0 either way. --png never writes over a .yaff or into \
            build/font-edit/.",
    example: (
        "gbcli font diff old.yaff assets/fonts/talk.yaff --png diff.png",
        "lists the glyphs that changed and draws them before and after",
    ),
};
