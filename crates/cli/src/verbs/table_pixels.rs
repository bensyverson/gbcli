//! `pixels`: an image as pixel text, and pixel text as a PNG.

use super::flags::{JSON, switch, value};
use super::{Arity, Class, FlagSpec, Verb, VerbSpec};

const PIXELS: &[FlagSpec] = &[
    value(
        "text",
        "PATH",
        "write the image as canonical pixel text (rows: a block per frame or 8-row band)",
    ),
    switch(
        "tiles",
        "with --text, write the distinct 8x8 tiles and a map instead of rows",
    ),
    value(
        "png",
        "PATH",
        "write the image at 1:1 as a PNG, the one the build would read",
    ),
    value(
        "preview",
        "PATH",
        "write it four times larger with an 8x8 grid, for looking only (never in assets/)",
    ),
    value(
        "palettes",
        "FILE",
        "resolve `PALETTE N` and `dmg N` through this palette file (default: the \
         assets/palettes.txt of the project FILE sits in)",
    ),
];

/// `--json` alone: `pixels` needs no session, so no `--dir` or `--as`.
const JSON_ONLY: &[FlagSpec] = &[JSON];

pub const PIXELS_VERB: VerbSpec = VerbSpec {
    verb: Verb::Pixels,
    name: "pixels",
    class: Class::Build,
    summary: "read a PNG or pixel text; write the other, or a preview",
    usage: "FILE [--text PATH] [--tiles] [--png PATH] [--preview PATH] [--palettes FILE] [--json]",
    arity: Arity {
        min: 1,
        max: Some(1),
        name: "FILE",
    },
    flags: PIXELS,
    groups: &[JSON_ONLY],
    about: "Reads FILE, a PNG or pixel text (`gbcli help pixels`), and prints a `pixels` row: \
            its form, size, colors, whether it has clear pixels, frames (when its name gives \
            a frame size, hero.16x16.pixels) and distinct 8x8 tiles. --text writes canonical \
            pixel text (from a PNG, a legend made lightest first: . clear, then 0-9, a-z, \
            A-Z), --tiles lays it out as tiles and a map, --png writes the image the build \
            reads, and --preview a 4x copy with a grid. PNG to text to PNG is pixel for \
            pixel. Nothing is written unless the whole file reads; a problem is \
            pixels_unreadable (exit 4) with its line and column (or, in a PNG, its pixel) \
            in a `problem` row. Needs no session and no RGBDS.",
    example: (
        "gbcli pixels assets/bg/town.png --text town.pixels --tiles",
        "writes town.pixels, the background's tiles and map as text an agent can read; \
         `gbcli pixels town.pixels --preview town.big.png` draws it back",
    ),
};
