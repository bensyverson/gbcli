//! The RGBDS loop: `new`, `build` and `assets`, and `doctor` (a help verb).

use super::flags::{ALLOW_ROM_CHANGE, CHANGE, JSON, OUTPUT, SESSION_FLAG, repeated, switch, value};
use super::{Arity, Class, FlagSpec, Verb, VerbSpec};

/// `build`'s own flags: the layout it assumes, and the checkpoint to reload.
const BUILD: &[FlagSpec] = &[
    repeated(
        "src",
        "DIR",
        "assemble every .asm under DIR instead of src/; a relative DIR is read against the \
         project; repeat for more",
    ),
    repeated(
        "include",
        "DIR",
        "put DIR on rgbasm's include path instead of include/; a relative DIR is read \
         against the project; repeat for more",
    ),
    value(
        "out",
        "FILE",
        "the ROM to write, with its .sym and .map beside it (default build/<project>.gb \
         in the project; with --gbc, build/gbc/<project>.gbc); a relative FILE is read \
         against the working directory (--dir's PATH when given), not the project",
    ),
    switch(
        "gbc",
        "build for the GBC in dual mode: rgbfix -c sets $0143 to $80, and the ROM still \
         runs on the DMG",
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

/// `--json` alone: `new` and `doctor` need no session or bundle, so they
/// have no `--dir` or `--as`.
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
    usage: "[DIR] [--src DIR]… [--include DIR]… [--out FILE] [--gbc] [--session NAME \
            --restore NAME] [flags]",
    arity: Arity {
        min: 0,
        max: Some(1),
        name: "DIR",
    },
    flags: BUILD,
    groups: &[RELOAD, CHANGE, OUTPUT],
    about: "Builds the project in DIR (default: here) by convention: rgbasm assembles every \
            .asm under src/ with include/ on the include path, rgblink links them into \
            build/<DIR's name>.gb with a .sym and a .map (--gbc: build/gbc/<DIR's name>.gbc, \
            so both builds' outputs stay), and rgbfix fixes the header. Each \
            step's outputs must exist and be non-empty. Every diagnostic is a `diagnostic` \
            row with its file and line (a message repeated on lines in a row is one row, \
            with lines= and count=); any error fails with build_failed (exit 4) and leaves \
            the last good ROM in place. DIR names the project only: a relative --src or \
            --include is read against it, a relative --out against the working directory \
            (from a parent folder, `gbcli build game --out game/fast.gb`). With --session \
            and --restore, a good build is \
            reloaded: the checkpoint is restored into the rebuilt ROM, which loads only if \
            nothing it was running moved (rom_changed, exit 3, says what did; `gbcli run \
            --from boot` then starts the session over on the new ROM).",
    example: (
        "gbcli build --session game --restore start",
        "rebuilds the project here, then restores checkpoint start of session game into the \
         new ROM",
    ),
};

/// `assets`' own flags: what to convert, for which machine, and where to
/// keep it.
const ASSETS: &[FlagSpec] = &[
    value(
        "manifest",
        "FILE",
        "convert the images a JSON file names (`gbcli schema`, manifest) instead of a project's \
         assets/; its relative `file` and `dmg` paths are read from the manifest's folder (a \
         `serve` client's, from the server's working directory)",
    ),
    switch(
        "gbc",
        "convert for the GBC in dual mode, as `build --gbc` does: each image's GBC version too",
    ),
    value(
        "keep",
        "DIR",
        "keep the data and previews in DIR, laid out as build/images/ (default: nothing is kept)",
    ),
];

pub const ASSETS_VERB: VerbSpec = VerbSpec {
    verb: Verb::Assets,
    name: "assets",
    class: Class::Build,
    summary: "convert a project's or a manifest's images without building: tile maps, palettes",
    usage: "[DIR] [--manifest FILE] [--gbc] [--keep DIR] [--json]",
    arity: Arity {
        min: 0,
        max: Some(1),
        name: "DIR",
    },
    flags: ASSETS,
    groups: &[JSON_ONLY],
    about: "Converts the PNGs under DIR's assets/ (default: here), or the images a manifest \
            names, exactly as `gbcli build` would, encoded by gbcli itself: no RGBDS, no \
            session. Prints the `image`, `problem` and `images` rows a build prints; --json \
            gives each image's tile_map (tile, flip and GBC palette per source tile) and \
            palettes. Nothing is written unless --keep names a directory; images.asm and \
            images.inc are a build's. A problem fails with build_failed (exit 4), its rows \
            printed. A manifest names each image as its path under assets/ in a project \
            (`tiles/font.png`), the file to read, and the palettes palettes.txt would hold.",
    example: (
        "gbcli assets --manifest font.json --gbc --json",
        "converts the images font.json names for the GBC and prints each one's tile map and \
         palettes, writing nothing",
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
