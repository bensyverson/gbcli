//! The read verbs for pictures, VRAM and the session's listings.

use super::flags::{OUTPUT, SESSION, SHOT, SHOT_OPTIONS, VIEW};
use super::{Arity, Class, Verb, VerbSpec};

pub const SHOT_VERB: VerbSpec = VerbSpec {
    verb: Verb::Shot,
    name: "shot",
    class: Class::Read,
    summary: "save a PNG of the screen at the head",
    usage: "PATH [--scale N] [--crop X,Y,W,H] [--live | --any-frame] [flags]",
    arity: Arity {
        min: 1,
        max: Some(1),
        name: "PATH",
    },
    flags: SHOT,
    groups: &[SHOT_OPTIONS, SESSION, OUTPUT],
    about: "Writes the last complete frame the LCD showed, as the real screen's four \
            grays, and prints which frame it is. A frame that isn't normal (LCD off, the \
            blank frame after the LCD turns on) is refused with exit 1 unless --any-frame; \
            --live takes the pixels drawn so far instead.",
    example: (
        "gbcli shot title.png --session dx",
        "saves the head's last frame at 4x as title.png and prints its frame number",
    ),
};

pub const VIEW_VERB: VerbSpec = VerbSpec {
    verb: Verb::View,
    name: "view",
    class: Class::Read,
    summary: "VRAM tiles, a tile map or OAM, as text (and a PNG)",
    usage: "tiles|map|oam [--map WHICH] [--png PATH [--scale N]] [flags]",
    arity: Arity {
        min: 1,
        max: Some(1),
        name: "tiles|map|oam",
    },
    flags: VIEW,
    groups: &[SESSION, OUTPUT],
    about: "tiles: every non-blank tile as eight rows of color indexes 0-3. map: the \
            32x32 tile ids of the background or window map (--map), with CGB attributes. \
            oam: the 40 objects with screen positions and which ones the 10-per-line \
            limit drops. --png saves the same view as an image.",
    example: (
        "gbcli view oam --session dx",
        "lists the sprites: position, tile, flags and whether each is on screen",
    ),
};

pub const SYM_VERB: VerbSpec = VerbSpec {
    verb: Verb::Sym,
    name: "sym",
    class: Class::Read,
    summary: "the session's symbols matching a pattern",
    usage: "PATTERN [flags]",
    arity: Arity {
        min: 1,
        max: Some(1),
        name: "PATTERN",
    },
    flags: &[],
    groups: &[SESSION, OUTPUT],
    about: "Lists the symbols of the session's .sym whose names match PATTERN, where * \
            matches any run of characters and ? one character; names match as spelled. \
            Each row gives the bank, address and memory area.",
    example: (
        "gbcli sym 'w*' --session dx",
        "lists every symbol starting with w: the WRAM variables, by convention",
    ),
};

pub const CHECKPOINTS_VERB: VerbSpec = VerbSpec {
    verb: Verb::Checkpoints,
    name: "checkpoints",
    class: Class::Read,
    summary: "the session's named checkpoints and history",
    usage: "[flags]",
    arity: Arity {
        min: 0,
        max: Some(0),
        name: "",
    },
    flags: &[],
    groups: &[SESSION, OUTPUT],
    about: "One row per state `restore` and `run --at` accept: the named checkpoints, then \
            the history entries ~1, ~2 … with the frame since boot, the revision when \
            known, and whether it was saved over the ROM on disk now.",
    example: (
        "gbcli checkpoints --session dx",
        "lists what you can restore",
    ),
};
