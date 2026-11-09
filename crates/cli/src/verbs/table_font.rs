//! `font`: a `.yaff` font read, a sample set by the layout rules, a
//! preview, and `--fit`.

use super::flags::{JSON, value};
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
