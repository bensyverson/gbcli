//! `song`: hUGETracker songs to and from text, and their piano roll.

use super::flags::{JSON, value};
use super::{Arity, Class, FlagSpec, Verb, VerbSpec};

const SONG: &[FlagSpec] = &[
    value(
        "text",
        "PATH",
        "write the song as text (the canonical spelling: every row, every field)",
    ),
    value(
        "uge",
        "PATH",
        "write the song as a hUGETracker .uge (version 6)",
    ),
    value(
        "piano",
        "PATH",
        "save a piano roll PNG of the song's notes, drawn from the file without building it",
    ),
    value(
        "asm",
        "PATH",
        "write the song as RGBDS source for hUGEDriver, as build converts assets/music, \
         for a build of your own (the `song` op answers the same bytes)",
    ),
    value(
        "descriptor",
        "NAME",
        "with --asm: the label to pass hUGE_init, which scopes the source's others \
         (default song_ and FILE's stem)",
    ),
];

/// `--json` alone: `song` sends no request, so it has no `--dir` or `--as`.
const JSON_ONLY: &[FlagSpec] = &[JSON];

pub const SONG_VERB: VerbSpec = VerbSpec {
    verb: Verb::Song,
    name: "song",
    class: Class::Build,
    summary: "read a hUGETracker .uge or song text; write the other, or a piano roll",
    usage: "FILE [--text PATH] [--uge PATH] [--piano PATH] [--asm PATH [--descriptor NAME]] \
            [--json]",
    arity: Arity {
        min: 1,
        max: Some(1),
        name: "FILE",
    },
    flags: SONG,
    groups: &[JSON_ONLY],
    about: "Reads FILE, a hUGETracker .uge (versions 0-6, upgraded as hUGETracker upgrades \
            them) or song text (`gbcli help music`), and prints a `song` row: its form, name \
            and counts. --text and --uge write it in either form, and --piano draws its notes \
            as a PNG without building or playing it; --asm writes the RGBDS source a build \
            assembles with hUGEDriver, the `song` op's answer. Nothing is written unless the whole file \
            reads; a problem is song_unreadable (exit 4) with its line and column (or, in a \
            .uge, its byte) in a `problem` row. Needs no session and no RGBDS.",
    example: (
        "gbcli song theme.uge --text theme.song",
        "writes theme.song, a text grid of the song an agent can edit; `gbcli song \
         theme.song --uge theme.uge --piano theme.png` turns it back and draws it",
    ),
};
