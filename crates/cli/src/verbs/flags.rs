//! Flags shared by many verbs. A flag means one thing on every verb that
//! takes it (cli-design ▸ Grammar), so each is defined once, here.

use super::FlagSpec;

const fn value(long: &'static str, value: &'static str, help: &'static str) -> FlagSpec {
    FlagSpec {
        long,
        short: None,
        value: Some(value),
        repeat: false,
        help,
    }
}

const fn switch(long: &'static str, help: &'static str) -> FlagSpec {
    FlagSpec {
        long,
        short: None,
        value: None,
        repeat: false,
        help,
    }
}

const fn repeated(long: &'static str, value: &'static str, help: &'static str) -> FlagSpec {
    FlagSpec {
        long,
        short: None,
        value: Some(value),
        repeat: true,
        help,
    }
}

pub const JSON: FlagSpec = switch("json", "print the protocol's result object instead of rows");
pub const DIR: FlagSpec = value(
    "dir",
    "PATH",
    "find .gbcli/ from PATH instead of the working directory (GBCLI_DIR overrides both)",
);
pub const AS: FlagSpec = value(
    "as",
    "NAME",
    "who is asking, for the event log (default: GBCLI_AS)",
);

/// Taken by every verb that sends a request.
pub const OUTPUT: &[FlagSpec] = &[JSON, DIR, AS];

pub const SESSION_FLAG: FlagSpec = FlagSpec {
    long: "session",
    short: Some('s'),
    value: Some("NAME"),
    repeat: false,
    help: "the session to use (default: GBCLI_SESSION); naming a new one with a ROM creates it",
};

pub const ROM: FlagSpec = value(
    "rom",
    "FILE",
    "the ROM a new session is created from; an existing session refuses another ROM",
);
pub const SYM: FlagSpec = value(
    "sym",
    "FILE",
    "the .sym for a new session (default: the ROM's name with .sym, if it exists)",
);
pub const MODEL: FlagSpec = value(
    "model",
    "MODEL",
    "the model for a new session: dmg-b (default), cgb-e, …",
);
pub const SEED: FlagSpec = value("seed", "N", "the RAM seed for a new session (default 0)");

/// Taken by every verb that works on one session.
pub const SESSION: &[FlagSpec] = &[
    SESSION_FLAG,
    ROM,
    SYM,
    MODEL,
    SEED,
    value(
        "rev",
        "sN",
        "refuse a change if the session is no longer at this revision",
    ),
    switch(
        "allow-rom-change",
        "load a stored state even though the ROM was rebuilt and the check refuses it",
    ),
];

/// `batch`: a default session, created from a ROM if new.
pub const CREATE: &[FlagSpec] = &[SESSION_FLAG, ROM, SYM, MODEL, SEED];

/// `serve`: a default session, created from a ROM if new, and who asks.
pub const SERVE: &[FlagSpec] = &[SESSION_FLAG, ROM, SYM, MODEL, SEED, DIR, AS];

pub const SINCE: FlagSpec = value(
    "since",
    "BASE",
    "what frame and t count from: boot (default), power-on or a checkpoint name",
);
pub const ALLOW_UNINIT: FlagSpec = switch(
    "allow-uninit",
    "read never-written bytes as their power-on values, with a warning",
);
pub const IF: FlagSpec = value("if", "EXPR", "stop only when this condition holds");

pub const SCALE: FlagSpec = value(
    "scale",
    "N",
    "pixels per Game Boy pixel in the PNG, 1 to 16 (default 4)",
);

/// How a screenshot is taken: by `run --shot` and by `shot`.
pub const SHOT_OPTIONS: &[FlagSpec] = &[
    SCALE,
    value(
        "crop",
        "X,Y,W,H",
        "only this rectangle of the 160x144 screen, in Game Boy pixels",
    ),
    switch(
        "live",
        "the pixels drawn so far this frame, not the last complete frame",
    ),
    switch(
        "any-frame",
        "take the last frame even if it isn't a normal one (LCD off, blank)",
    ),
];

pub const RUN: &[FlagSpec] = &[
    value(
        "at",
        "NAME|~N",
        "start from a named checkpoint or the head N changes back",
    ),
    value(
        "from",
        "boot|power-on",
        "with no head, start at the boot handoff (default) or at power-on",
    ),
    SINCE,
    value("until", "EXPR", "stop when this condition holds"),
    value(
        "to",
        "ADDR",
        "stop before the instruction at this address executes",
    ),
    value(
        "frames",
        "N",
        "frame budget (default 600 with --until or --to)",
    ),
    value("instructions", "N", "instruction budget"),
    value("ticks", "N", "tick budget"),
    repeated(
        "write",
        "TARGET=BYTES",
        "bus write before the run, as the CPU would",
    ),
    repeated(
        "poke",
        "TARGET=BYTES",
        "write backing memory before the run, no side effects",
    ),
    repeated(
        "patch",
        "TARGET=BYTES",
        "patch the ROM before the run (recorded in checkpoints)",
    ),
    repeated("read", "EXPR", "evaluate at the stop; repeat for more"),
    value("checkpoint", "NAME", "save the stop state under this name"),
    ALLOW_UNINIT,
    value(
        "shot",
        "PATH",
        "save a PNG of the screen at the stop (see the shot flags)",
    ),
    value(
        "keys",
        "TOKENS",
        "press and release buttons: 'A@0 -A@2 START@10' (frame offsets)",
    ),
    value(
        "keys-file",
        "FILE",
        "key tokens from a file, after --keys; # starts a comment",
    ),
];

pub const SHOT: &[FlagSpec] = &[SINCE];

pub const VIEW: &[FlagSpec] = &[
    value(
        "map",
        "WHICH",
        "for view map: background (default), window, 9800 or 9c00",
    ),
    value("png", "PATH", "also save the view as a PNG"),
    SCALE,
];

pub const READ: &[FlagSpec] = &[SINCE, ALLOW_UNINIT];
pub const BREAK: &[FlagSpec] = &[IF];
pub const WATCH: &[FlagSpec] = &[IF, switch("once", "delete the watch after it fires once")];
pub const PRUNE: &[FlagSpec] = &[value(
    "unused-for",
    "DURATION",
    "remove sessions idle this long: 30m, 12h, 7d (required)",
)];
pub const RM: &[FlagSpec] = &[switch(
    "with-checkpoints",
    "remove the session even though it holds named checkpoints",
)];
pub const EVENTS: &[FlagSpec] = &[value("limit", "N", "only the last N lines")];
pub const BATCH: &[FlagSpec] = &[FlagSpec {
    long: "file",
    short: Some('F'),
    value: Some("FILE"),
    repeat: false,
    help: "protocol request lines, one per line; - reads stdin (required)",
}];
