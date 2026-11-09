//! Flags shared by many verbs. A flag means one thing on every verb that
//! takes it (cli-design ▸ Grammar), so each is defined once, here.

use super::FlagSpec;

pub(super) const fn value(long: &'static str, value: &'static str, help: &'static str) -> FlagSpec {
    FlagSpec {
        long,
        short: None,
        value: Some(value),
        repeat: false,
        help,
    }
}

pub(super) const fn switch(long: &'static str, help: &'static str) -> FlagSpec {
    FlagSpec {
        long,
        short: None,
        value: None,
        repeat: false,
        help,
    }
}

pub(super) const fn repeated(
    long: &'static str,
    value: &'static str,
    help: &'static str,
) -> FlagSpec {
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
    "run as if from PATH: find .gbcli/ from it and read relative paths (the ROM, files \
     read and written) against it (beats GBCLI_DIR)",
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
    "the .sym for a new session (default: the ROM's name with .sym, if it exists); an \
     existing session refuses another",
);
pub const MODEL: FlagSpec = value(
    "model",
    "MODEL",
    "the model for a new session: dmg-b (default, also dmg), gbc (cgb-e, a GBC in CGB mode \
     for a $80 or $C0 ROM), cgb0, …; an existing session refuses another",
);
pub const SEED: FlagSpec = value(
    "seed",
    "N",
    "the RAM seed for a new session (default 0); an existing session refuses another",
);

pub const FLASH: FlagSpec = value(
    "flash",
    "BOARD",
    "a new session's cart is a flash-save board (`gbcli help flash` lists them; default: mask \
     ROM); an existing session refuses another",
);

pub const ALLOW_ROM_CHANGE: FlagSpec = switch(
    "allow-rom-change",
    "load a stored state even though the ROM was rebuilt and the check refuses it",
);

/// Taken by every verb that works on one session.
pub const SESSION: &[FlagSpec] = &[SESSION_FLAG, ROM, SYM, MODEL, SEED, FLASH, ALLOW_ROM_CHANGE];

/// Taken by every verb whose emulation the caller sizes: `run`, `step`,
/// `measure` and `equiv`.
pub const GUARD: &[FlagSpec] = &[value(
    "wall-guard",
    "SECONDS",
    "end the request after SECONDS of host time instead of the default 60, 1 to 3600; \
     a stop it causes is wall_guard (exit 3), not the emulated budget",
)];

/// Taken by every verb that changes the session: its head, checkpoints,
/// breakpoints or watches. A verb that changes nothing has no `--rev`.
pub const CHANGE: &[FlagSpec] = &[value(
    "rev",
    "sN",
    "refuse the change if the session is no longer at this revision",
)];

/// `batch`: a default session, created from a ROM if new.
pub const CREATE: &[FlagSpec] = &[SESSION_FLAG, ROM, SYM, MODEL, SEED, FLASH];

/// `serve`: a default session, created from a ROM if new, and who asks.
pub const SERVE: &[FlagSpec] = &[SESSION_FLAG, ROM, SYM, MODEL, SEED, FLASH, DIR, AS];

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
    "pixels per Game Boy pixel in the PNG, 1 to 16 (default 4); on run, also pixels per \
     image pixel in --filmstrip, --piano and --spectrogram (default 2 for --piano, 1 for \
     the others: try --scale 4 for a short clip's spectrogram)",
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
        "start at the boot handoff (the default with no head) or at power-on; given, either \
         skips the head, so --from boot restarts the session on a rebuilt ROM, keeping its \
         breakpoints and watches (moved with their labels); the old head stays in history; \
         not with --at",
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
        "frame budget; any run without a budget gets 600 frames, and using them all up \
         without another stop is budget_exhausted; it stops as VBlank begins, before the \
         handler runs (gbcli help time)",
    ),
    value("instructions", "N", "instruction budget"),
    value("ticks", "N", "tick budget"),
    repeated(
        "write",
        "TARGET=BYTES",
        "bus write before the run, as the CPU would; repeat for more",
    ),
    repeated(
        "poke",
        "TARGET=BYTES",
        "write backing memory before the run, no side effects; repeat for more",
    ),
    repeated(
        "patch",
        "TARGET=BYTES",
        "patch the ROM before the run (recorded in checkpoints); repeat for more",
    ),
    repeated(
        "read",
        "EXPR",
        "evaluate one expression at the stop; repeat --read for more (a read that fails says \
         why in its own row)",
    ),
    repeated(
        "expect",
        "EXPR",
        "a condition that must hold at the stop, evaluated beside --read (same grammar, same \
         never-written rule); prints an expect row (expr, value, ok) for each, and if any is \
         false the run fails with exit 1 (predicate_false), still saved as the head, without \
         --checkpoint; one that can't be evaluated fails with its own error; a run that fails \
         on its own first (--until not reached, the wall guard) reports that error, without \
         expect rows; repeat for more",
    ),
    value("checkpoint", "NAME", "save the stop state under this name"),
    ALLOW_UNINIT,
    value(
        "shot",
        "PATH",
        "save a PNG of the screen at the stop (see the shot flags)",
    ),
    value(
        "filmstrip",
        "PATH",
        "save a PNG of the frames the run passed, in a row, each labeled (LCD-off frames \
         kept); at most 16, more are counted and warned about",
    ),
    value(
        "every",
        "N",
        "with --filmstrip: keep a frame every N frames of the run (default 1)",
    ),
    switch(
        "audio-log",
        "print every write the run made to the sound registers, decoded into notes and \
         settings, at elapsed 2^23 Hz ticks since --since, with the frame (counted as the \
         stop's frame=) and the line (ly) it fell on: the cheap sibling of --wav. Only \
         this run's: writes made before the run (a sound engine's setup) aren't listed. \
         The rows stop at 20 with a more row; --json has every write",
    ),
    switch(
        "audio-summary",
        "print the run's sound-register writes counted by register (wave RAM as WAVE) and \
         by writer PC (and its symbol, Label+$n), most first, with the registers each \
         wrote: the log in a few rows",
    ),
    repeated(
        "audio-channel",
        "N",
        "with --audio-log or --audio-summary: only channel N's writes, 1-4 or square1, \
         square2, wave, noise (its NRxy registers; wave RAM is the wave channel's; NR50-NR52 \
         are no channel's and are left out); repeat for more",
    ),
    value(
        "wav",
        "PATH",
        "save what the run played as a WAV (16-bit stereo, 32,768 Hz)",
    ),
    value(
        "channels",
        "mixed|each",
        "with --wav: each also saves every channel alone beside it (PATH's stem with \
         -square1, -square2, -wave, -noise) by replaying the run muted",
    ),
    value(
        "piano",
        "PATH",
        "save a piano roll PNG of the notes the run's sound writes played",
    ),
    value(
        "spectrogram",
        "PATH",
        "save a spectrogram PNG of what the run played",
    ),
    value(
        "measure",
        "FROM..TO",
        "after the stop, on a copy of it: cycles from the first arrival at FROM to the next \
         arrival at TO (one iteration; `gbcli measure` for more)",
    ),
    switch(
        "profile",
        "cycles per routine over the run, by .sym, most exclusive cycles first; idle_t is \
         the HALT share of a routine's exclusive cycles, a HALT reached by `jp` included",
    ),
    value(
        "top",
        "N",
        "with --profile: only the N routines with the most exclusive cycles",
    ),
    switch(
        "frame-budget",
        "each frame's active and idle (HALT) T-cycles over the run, and the lag frames: \
         frames in which the CPU never halted with the LCD on (LCD-off frames, as at \
         startup or a screen change, count as lcd_off instead); the first row sums up the \
         whole LCD-on frames' active T-cycles (min, max, mean) and the busiest one (a row \
         index), then a frame_lag row per run of consecutive lag frames: from= and to= \
         (counted as the stop's frame=), count=, active_max= and index= (its first row in \
         --json), at most 20 rows (--json has every frame, each with its frame; gbcli help \
         budgets)",
    ),
    switch(
        "oam-log",
        "OAM at every vblank the run passes: a first row naming the objects the 10-per-line \
         limit dropped in any frame (dropped=) and in every frame they were on screen \
         (never_drawn=), then a row per frame, at most 20, with the objects on screen and \
         the ones dropped (--json has every frame and object); each frame's OAM is the one \
         it was drawn from",
    ),
    switch(
        "raster",
        "each IO-register write the run made: first a raster_writer row per register and \
         writer PC counting its writes by the PPU mode they landed in (unknown= where the \
         emulator can't tell, lcd_off= with the LCD off), at most 20, those with writes in drawing, unknown or oam_scan \
         first, and a more row summing the rest; then by frame and line, at most 40 rows of \
         lines and writes (20 lines, 20 writes to a line): register, value, writer's PC, \
         ticks into the line (912 per line) and mode= (--json has every line and write)",
    ),
    value(
        "keys",
        "TOKENS",
        "press and release buttons: 'A@0 -A@2 START@10' (frame offsets); a key still \
         held when the run ends stays held into the next run; K@N lands as frame N's \
         VBlank begins (gbcli help time)",
    ),
    value(
        "keys-file",
        "FILE",
        "key tokens from a file, after --keys; # starts a comment",
    ),
    value(
        "keys-since",
        "BASE",
        "what the key offsets count from: boot, power-on or a checkpoint name, as --since \
         names them (default: the run's start). So a file recorded from boot replays from a \
         mid-run checkpoint: --at floor5 --keys-file win.keys --keys-since boot applies only \
         the events from floor5's frame on (the earlier ones are in floor5 already, and \
         aren't reported as unapplied). boot is refused on a run that starts inside the \
         boot ROM, before the handoff",
    ),
];

pub const SHOT: &[FlagSpec] = &[SINCE];

pub const EQUIV: &[FlagSpec] = &[
    value(
        "frames",
        "N",
        "how many frames to compare, each ended by a vblank (required)",
    ),
    value(
        "sym-b",
        "FILE",
        "side B's .sym (default: ROM_B's name with .sym, when that file exists, else none; \
         with no ROM_B, the session's)",
    ),
    repeated(
        "patch",
        "TARGET=BYTES",
        "patch side B's ROM after its start state loads; repeat for more",
    ),
    value(
        "at",
        "NAME|~N",
        "start both sides from this checkpoint (the rebuilt-ROM check applies to B)",
    ),
    value(
        "from",
        "boot|power-on",
        "start each side at its own boot handoff (default) or at power-on; not with --at",
    ),
    repeated(
        "ram",
        "ADDR[,LEN]",
        "compare these bytes after every frame, each side through its own .sym; repeat: \
         --ram '$C0A0,4' (4 bytes from $C0A0), --ram wPlayerX (1 byte), --ram wPos,2",
    ),
    value(
        "screen",
        "on|off",
        "compare each frame's pixels (on, the default) or not: off lets --ram and the \
         sound writes stand alone, for builds whose timing differs by design",
    ),
    value(
        "audio",
        "writes|timed|off",
        "compare each frame's sound writes in order (writes, the default), also their \
         times (timed), or not at all",
    ),
    value(
        "keys",
        "TOKENS",
        "the input both sides get: 'A@0 -A@2 START@10' (frame offsets)",
    ),
    value(
        "keys-file",
        "FILE",
        "key tokens from a file, after --keys; # starts a comment",
    ),
];

pub const MEASURE: &[FlagSpec] = &[
    value(
        "at",
        "NAME|~N",
        "measure from a named checkpoint or the head N changes back (default: the head)",
    ),
    value(
        "from",
        "boot|power-on",
        "measure from the boot handoff or from power-on instead of the head; not with --at",
    ),
    value(
        "iterations",
        "N",
        "how many FROM-to-TO spans to measure (default 1)",
    ),
    value(
        "max",
        "T",
        "fail with exit 1 (predicate_false) if any iteration takes more than T T-cycles; \
         an iteration ends at the next arrival at TO, so when TO isn't \
         reached on every pass the max spans several passes: read min, or pick a TO every pass \
         reaches",
    ),
    value(
        "frames",
        "N",
        "frame budget for the whole measurement (default 600)",
    ),
    value("instructions", "N", "instruction budget"),
    value("ticks", "N", "tick budget"),
    value(
        "keys",
        "TOKENS",
        "press and release buttons on the copy, as run --keys: 'RIGHT@0' holds RIGHT from the \
         start (frame offsets from the measurement's start)",
    ),
    value(
        "keys-file",
        "FILE",
        "key tokens from a file, after --keys; # starts a comment",
    ),
];

pub const VIEW: &[FlagSpec] = &[
    value(
        "map",
        "WHICH",
        "for view map: background (default), window, 9800 or 9c00",
    ),
    value("png", "PATH", "also save the view as a PNG"),
    SCALE,
    value(
        "out",
        "FILE",
        "for view memory: also write the raw bytes to FILE",
    ),
    ALLOW_UNINIT,
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
