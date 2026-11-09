//! The read verbs for pictures, VRAM and the session's listings.

use super::flags::{CHANGE, EQUIV, MEASURE, OUTPUT, SESSION, SHOT, SHOT_OPTIONS, VIEW};
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
            --live takes the pixels drawn so far instead. The PNG is palette-indexed at \
            the smallest bit depth that holds its colors (2 bits for a four-gray screen), \
            with indices in order of first appearance, not shade order: a decoder reads \
            the PLTE chunk, not the raw index.",
    example: (
        "gbcli shot title.png --session dx",
        "saves the head's last frame at 4x as title.png and prints its frame number",
    ),
};

pub const VIEW_VERB: VerbSpec = VerbSpec {
    verb: Verb::View,
    name: "view",
    class: Class::Read,
    summary: "VRAM tiles, a tile map, OAM, the palettes or a block of memory, as text",
    usage: "tiles|map|oam|palettes [--map WHICH] [--png PATH [--scale N]] | memory ADDR LEN \
            [--out FILE] [--allow-uninit] [flags]",
    arity: Arity {
        min: 1,
        max: Some(3),
        name: "tiles|map|oam|palettes|memory",
    },
    flags: VIEW,
    groups: &[SESSION, OUTPUT],
    about: "tiles: every non-blank tile as eight rows of color indexes 0-3. map: the \
            32x32 tile ids of the background or window map (--map), with CGB attributes. \
            oam: the 40 objects with screen positions and which ones the 10-per-line \
            limit drops (run --oam-log gives it for every frame of a run). palettes: \
            BGP, OBP0 and OBP1 with the shade each color index gets, and on a GBC the 8 \
            background and 8 object palettes from palette RAM as RGB555 and #RRGGBB. --png \
            saves the same view as an image (not for palettes). memory ADDR LEN: LEN bytes \
            from ADDR as a hexdump, lowercase hex, 16 bytes a row led by the row's address; ADDR is any \
            address read takes (wMap, $C000, wMap+$10, a banked symbol or 03:4000 for that \
            bank, wram:1A0), and in a switchable area otherwise the bank mapped at the head. \
            Each byte is read as read reads it, without side effects; a byte the program \
            never wrote is an error naming the first, unless --allow-uninit. A block from a \
            CPU address reads on through the banks mapped at the head; one from a bank or \
            region must end by its area's end, or is refused naming the length that fits. A block over FEA0-FEFF (no backing store) is refused whole, as read \
            refuses a byte there. --out FILE also writes the raw bytes.",
    example: (
        "gbcli view memory wMap 64 --session dx",
        "prints the 64 bytes from wMap, 16 a row; --out map.bin also saves them",
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

pub const MEASURE_VERB: VerbSpec = VerbSpec {
    verb: Verb::Measure,
    name: "measure",
    class: Class::Read,
    summary: "count the cycles from one address to another",
    usage: "FROM..TO [--iterations N] [--max T] [--at NAME|~N] [--keys TOKENS] [flags]",
    arity: Arity {
        min: 1,
        max: Some(1),
        name: "FROM..TO",
    },
    flags: MEASURE,
    groups: &[SESSION, OUTPUT],
    about: "Runs a copy of the head (or --at) and counts the emulated cycles from the first \
            arrival at FROM (the current PC counts) to the next arrival at TO, --iterations \
            times. Prints n, the CPU speed, T- and M-cycles min/max/mean, and the share of a \
            70,224 T frame. The head never moves and breakpoints are ignored. --keys holds \
            buttons on the copy as `run --keys` would, to measure the held-input path. With \
            --max T it is an assertion: exit 1 if any iteration took more than T T-cycles. A \
            span across a CPU speed switch is refused (exit 1).",
    example: (
        "gbcli measure MainLoop+1..MainLoop --session dx --iterations 60 --max 40000",
        "measures 60 passes of the main loop's work, and exits 1 if any took over 40,000 T",
    ),
};

pub const EQUIV_VERB: VerbSpec = VerbSpec {
    verb: Verb::Equiv,
    name: "equiv",
    class: Class::Read,
    summary: "check that another build (or a patch) behaves frame for frame as this one",
    usage: "[ROM_B] --frames N [--patch TARGET=BYTES]… [--ram ADDR[,LEN]]… [--keys TOKENS] \
            [flags]",
    arity: Arity {
        min: 0,
        max: Some(1),
        name: "ROM_B",
    },
    flags: EQUIV,
    groups: &[SESSION, CHANGE, OUTPUT],
    about: "Runs the session's ROM (side A) and ROM_B, or the session's ROM with --patch \
            (side B), from equivalent starts (each side's boot handoff, or --at one \
            checkpoint) under one key timeline, and compares the screen, the --ram bytes \
            and the sound writes at the end of every frame; the verdict row names them \
            (screen=1 ram=… audio=…). Exit 0 when every frame \
            matches; exit 1 (predicate_false) at the first frame that differs, with every \
            difference, and both sides saved as checkpoints equiv-a and equiv-b (equiv-b \
            restores only when side B ran the session's ROM, as with --patch). A \
            frame_offset row says when that frame ended at a different time on each side \
            (a start-up of another length): then the screens can't match, and --screen \
            off with --ram compares timing-free state (`gbcli help optimize`). The head \
            never moves.",
    example: (
        "gbcli equiv fast.gb --session dx --frames 600 --keys 'RIGHT@10' --ram wX",
        "checks 600 frames of fast.gb against the session's ROM with RIGHT held from frame \
         10, comparing wX each frame",
    ),
};
