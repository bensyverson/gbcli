//! The RGBDS loop: `new` and `build`, and `doctor` (a help verb).

use super::flags::{ALLOW_ROM_CHANGE, CHANGE, JSON, OUTPUT, SESSION_FLAG, repeated, value};
use super::{Arity, Class, FlagSpec, Verb, VerbSpec};

/// `build`'s own flags: the layout it assumes, and the checkpoint to reload.
const BUILD: &[FlagSpec] = &[
    repeated(
        "src",
        "DIR",
        "assemble every .asm under DIR instead of src/ (relative to the project)",
    ),
    repeated(
        "include",
        "DIR",
        "put DIR on rgbasm's include path instead of include/",
    ),
    value(
        "out",
        "FILE",
        "the ROM to write, with its .sym and .map beside it (default build/<project>.gb)",
    ),
    value(
        "restore",
        "NAME",
        "after a good build, restore this checkpoint into --session, under the rebuilt-ROM \
         check",
    ),
];

/// The session to reload into: no ROM flags, since the session's ROM must
/// be the one the build wrote.
const RELOAD: &[FlagSpec] = &[SESSION_FLAG, ALLOW_ROM_CHANGE];

/// `--json` alone: a verb that sends no request has no `--dir` or `--as`.
const JSON_ONLY: &[FlagSpec] = &[JSON];

pub const NEW_VERB: VerbSpec = VerbSpec {
    verb: Verb::New,
    name: "new",
    class: Class::Build,
    summary: "scaffold a starter project that builds and runs",
    usage: "DIR [--json]",
    arity: Arity {
        min: 1,
        max: Some(1),
        name: "DIR",
    },
    flags: &[],
    groups: &[JSON_ONLY],
    about: "Writes a minimal RGBDS project into DIR (new, or empty): src/main.asm with the \
            header, a VBlank handler and a screen with a sprite on it, src/oam_dma.asm with \
            the OAM DMA routine copied to HRAM, the pinned gbdev hardware.inc in include/, a \
            README and a .gitignore. DIR's name names the ROM and the session the next \
            commands use. There is no build file: `gbcli build` assumes this layout.",
    example: (
        "gbcli new game",
        "writes game/ and prints the files, then the two commands that build it and take a \
         screenshot",
    ),
};

pub const BUILD_VERB: VerbSpec = VerbSpec {
    verb: Verb::Build,
    name: "build",
    class: Class::Build,
    summary: "assemble, link and fix a project with RGBDS; errors with file and line",
    usage: "[DIR] [--src DIR]… [--include DIR]… [--out FILE] [--session NAME --restore NAME] \
            [flags]",
    arity: Arity {
        min: 0,
        max: Some(1),
        name: "DIR",
    },
    flags: BUILD,
    groups: &[RELOAD, CHANGE, OUTPUT],
    about: "Builds the project in DIR (default: here) by convention: rgbasm assembles every \
            .asm under src/ with include/ on the include path, rgblink links them into \
            build/<DIR's name>.gb with a .sym and a .map, and rgbfix fixes the header. Each \
            step's outputs must exist and be non-empty. Every diagnostic is a `diagnostic` \
            row with its file and line; any error fails with build_failed (exit 4) and leaves \
            the last good ROM in place. With --session and --restore, a good build is \
            reloaded: the checkpoint is restored into the rebuilt ROM, which loads only if \
            nothing it was running moved (rom_changed, exit 3, says what did).",
    example: (
        "gbcli build --session game --restore start",
        "rebuilds the project here, then restores checkpoint start of session game into the \
         new ROM",
    ),
};

pub const DOCTOR_VERB: VerbSpec = VerbSpec {
    verb: Verb::Doctor,
    name: "doctor",
    class: Class::Help,
    summary: "check RGBDS against the pin, the boot ROMs and the project",
    usage: "[--json]",
    arity: Arity {
        min: 0,
        max: Some(0),
        name: "",
    },
    flags: &[],
    groups: &[JSON_ONLY],
    about: "One `check` row each: rgbasm, rgblink and rgbfix on PATH at the pinned RGBDS \
            version (another patch release warns), the built-in boot ROMs, whether a git \
            repository here ignores .gbcli/, and whether this directory is writable. A \
            failed check exits with its code (toolchain_missing names the install command); \
            warnings exit 0.",
    example: (
        "gbcli doctor",
        "prints one check row per item, each with its remedy when it isn't ok",
    ),
};
